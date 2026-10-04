//! Session management for DirectPlay over Iroh
//!
//! Handles session creation, joining, player management, and state tracking.
//!
//! ## Architecture
//!
//! - Host creates session and allocates player IDs (0x10000, 0x20000, ...)
//! - Each player has a ticket for mesh establishment
//! - All players maintain full player list with tickets

use crate::protocol::{PlayerInfo, SessionInfo};
use dp_types::{PlayerName, SessionDesc, DPID, GUID};
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};

/// Session state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Not in a session
    None,
    /// Hosting a session
    Hosting,
    /// Joined as a client
    Joined,
    /// Session closed/lost
    Closed,
}

/// A DirectPlay session
#[derive(Debug)]
pub struct Session {
    /// Session descriptor
    pub desc: SessionDesc,
    /// Our player ID
    pub local_player_id: Option<DPID>,
    /// All players in the session
    pub players: HashMap<DPID, Player>,
    /// Session state
    pub state: SessionState,
    /// Are we the host?
    pub is_host: bool,
    /// Joiners (host only) whose game hasn't called CreatePlayer yet, so the
    /// host's game hasn't been told about them. See [`SessionManager::add_joiner`].
    awaiting_create_player: HashSet<DPID>,
    /// Next available player ID (for host) - increments by 0x10000
    next_player_id: AtomicU32,
}

impl Session {
    /// Create a new session as host
    pub fn new_host(desc: SessionDesc) -> Self {
        Self {
            desc,
            local_player_id: None,
            players: HashMap::new(),
            state: SessionState::Hosting,
            is_host: true,
            awaiting_create_player: HashSet::new(),
            // Start at 0x10000, increment by 0x10000 for each player
            next_player_id: AtomicU32::new(0x10000),
        }
    }

    /// Create a session as a joining client
    pub fn new_client(desc: SessionDesc) -> Self {
        Self {
            desc,
            local_player_id: None,
            players: HashMap::new(),
            state: SessionState::Joined,
            is_host: false,
            awaiting_create_player: HashSet::new(),
            next_player_id: AtomicU32::new(0),
        }
    }

    /// Allocate a new player ID (host only)
    /// Returns IDs: 0x10000, 0x20000, 0x30000, ...
    pub fn allocate_player_id(&self) -> DPID {
        self.next_player_id.fetch_add(0x10000, Ordering::SeqCst)
    }

    /// Add a player to the session
    pub fn add_player(&mut self, player: Player) {
        self.players.insert(player.id, player);
        self.desc.current_players = self.players.len() as u32;
    }

    /// Remove a player from the session
    pub fn remove_player(&mut self, player_id: DPID) -> Option<Player> {
        self.awaiting_create_player.remove(&player_id);
        let player = self.players.remove(&player_id);
        self.desc.current_players = self.players.len() as u32;
        player
    }

    /// Get a player by ID
    pub fn get_player(&self, player_id: DPID) -> Option<&Player> {
        self.players.get(&player_id)
    }

    /// Get a mutable player by ID
    pub fn get_player_mut(&mut self, player_id: DPID) -> Option<&mut Player> {
        self.players.get_mut(&player_id)
    }

    /// Check if session is full
    pub fn is_full(&self) -> bool {
        self.players.len() as u32 >= self.desc.max_players
    }

    /// Get session info for broadcasting (includes host ticket)
    pub fn to_session_info(&self, host_node_id: [u8; 32], host_ticket: String) -> SessionInfo {
        SessionInfo::from_desc_with_host(&self.desc, host_node_id, host_ticket)
    }

    /// Get all players as PlayerInfo list (for JoinResponse)
    pub fn get_player_infos(&self) -> Vec<PlayerInfo> {
        self.players.values().map(|p| p.to_info()).collect()
    }
}

/// A player in a session
#[derive(Debug, Clone)]
pub struct Player {
    /// Player ID
    pub id: DPID,
    /// Player name
    pub name: PlayerName,
    /// Player flags
    pub flags: u32,
    /// Is this the local player?
    pub is_local: bool,
    /// Is this the server/host player?
    pub is_host: bool,
    /// Node ID (for identification)
    pub node_id: Option<[u8; 32]>,
    /// Ticket for direct P2P connection (mesh)
    pub ticket: String,
    /// Player data (application-defined)
    pub data: Vec<u8>,
    /// Local-only data
    pub local_data: Vec<u8>,
}

impl Player {
    /// Create a new local player with ticket
    pub fn new_local(id: DPID, name: PlayerName, flags: u32, node_id: [u8; 32], ticket: String) -> Self {
        Self {
            id,
            name,
            flags,
            is_local: true,
            is_host: false,
            node_id: Some(node_id),
            ticket,
            data: Vec::new(),
            local_data: Vec::new(),
        }
    }

    /// Create from PlayerInfo (remote player)
    pub fn from_info(info: PlayerInfo) -> Self {
        Self {
            id: info.player_id,
            name: info.name,
            flags: info.flags,
            is_local: false,
            is_host: false,
            node_id: Some(info.node_id),
            ticket: info.ticket,
            data: info.data,
            local_data: Vec::new(),
        }
    }

    /// Convert to PlayerInfo for network transmission
    pub fn to_info(&self) -> PlayerInfo {
        PlayerInfo {
            player_id: self.id,
            name: self.name.clone(),
            flags: self.flags,
            node_id: self.node_id.unwrap_or([0; 32]),
            ticket: self.ticket.clone(),
            data: self.data.clone(),
        }
    }
}

/// Thread-safe session manager
#[derive(Debug, Default)]
pub struct SessionManager {
    /// Current session (if any)
    session: RwLock<Option<Session>>,
    /// Known sessions from discovery
    discovered_sessions: RwLock<Vec<SessionInfo>>,
    /// Our ticket (set on init)
    our_ticket: RwLock<String>,
    /// Our node ID
    our_node_id: RwLock<Option<[u8; 32]>>,
    /// Host's node ID (if we're not the host)
    host_node_id: RwLock<Option<[u8; 32]>>,
    /// Shared player-data updates that arrived BEFORE the player was registered.
    /// Merged into the player at registration instead of being silently dropped.
    /// Cross-peer arrival order is unordered in a mesh, so a data update can
    /// legitimately beat the registration that announces its player; the game's
    /// own 0x102 handler silently skips unknown DPIDs with no retry, so the
    /// helper's session state must not share that hazard.
    pending_player_data: RwLock<HashMap<DPID, Vec<u8>>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set our identity info
    pub fn set_identity(&self, node_id: [u8; 32], ticket: String) {
        *self.our_node_id.write() = Some(node_id);
        *self.our_ticket.write() = ticket;
    }

    /// Get our ticket
    pub fn our_ticket(&self) -> String {
        self.our_ticket.read().clone()
    }

    /// Get our node ID
    pub fn our_node_id(&self) -> Option<[u8; 32]> {
        *self.our_node_id.read()
    }

    /// Get current session state
    pub fn state(&self) -> SessionState {
        self.session
            .read()
            .as_ref()
            .map(|s| s.state)
            .unwrap_or(SessionState::None)
    }

    /// Check if we're in a session
    pub fn in_session(&self) -> bool {
        matches!(self.state(), SessionState::Hosting | SessionState::Joined)
    }

    /// Check if we're the host
    pub fn is_host(&self) -> bool {
        self.session
            .read()
            .as_ref()
            .map(|s| s.is_host)
            .unwrap_or(false)
    }

    /// Create a new session as host
    pub fn create_session(&self, desc: SessionDesc) -> Option<GUID> {
        let mut session = self.session.write();
        if session.is_some() {
            return None;
        }

        let instance_guid = desc.guid_instance;
        *session = Some(Session::new_host(desc));
        Some(instance_guid)
    }

    /// Join a session as client
    pub fn join_session(&self, desc: SessionDesc, player_id: DPID) {
        let mut session = self.session.write();
        let mut new_session = Session::new_client(desc);
        new_session.local_player_id = Some(player_id);

        // The local player needs a real entry in the players map, not just
        // local_player_id: roster serialization and data lookups read the map.
        let node_id = self.our_node_id().unwrap_or([0; 32]);
        let ticket = self.our_ticket();
        let player = Player::new_local(player_id, PlayerName::default(), 0, node_id, ticket);
        new_session.add_player(player);

        *session = Some(new_session);
    }

    /// Join a session as client, storing the host's node ID for disconnect detection
    pub fn join_session_with_host(&self, desc: SessionDesc, player_id: DPID, host_node_id: [u8; 32]) {
        self.join_session(desc, player_id);
        *self.host_node_id.write() = Some(host_node_id);
    }

    /// Get the host's node ID (if we're not the host)
    pub fn get_host_node_id(&self) -> Option<[u8; 32]> {
        *self.host_node_id.read()
    }

    /// Check if a node ID is the host
    pub fn is_node_host(&self, node_id: &[u8; 32]) -> bool {
        if self.is_host() {
            // We are the host, no one else is
            false
        } else {
            self.host_node_id.read().as_ref() == Some(node_id)
        }
    }

    /// Find a still-in-session remote player's reconnect ticket by node id.
    /// Used by the reconnect driver to re-dial a peer after a transport loss so
    /// the full mesh heals. Returns None if the node isn't a known remote player
    /// or has no ticket (e.g. our own local player).
    pub fn get_player_ticket_by_node(&self, node_id: &[u8; 32]) -> Option<String> {
        let session = self.session.read();
        let session = session.as_ref()?;
        session
            .players
            .values()
            .find(|p| !p.is_local && p.node_id.as_ref() == Some(node_id) && !p.ticket.is_empty())
            .map(|p| p.ticket.clone())
    }

    /// Find an already-registered remote player's DPID by node id.
    /// Used to make JoinRequest idempotent: a reconnect/retry/double-Open from the
    /// same physical node must reuse its existing DPID rather than mint a second
    /// one (allocate_player_id fetch_adds unconditionally). Distinct duplicate DPIDs
    /// for one node are the genuine roster-corruption vector — the game dedups
    /// identical-DPID repeats but not two DPIDs for one peer.
    pub fn get_player_id_by_node(&self, node_id: &[u8; 32]) -> Option<DPID> {
        let session = self.session.read();
        let session = session.as_ref()?;
        session
            .players
            .values()
            .find(|p| !p.is_local && p.node_id.as_ref() == Some(node_id))
            .map(|p| p.id)
    }

    /// Close the current session
    pub fn close_session(&self) {
        let mut session = self.session.write();
        if let Some(ref mut s) = *session {
            s.state = SessionState::Closed;
        }
        *session = None;
        self.pending_player_data.write().clear();
    }

    /// Add discovered session
    pub fn add_discovered_session(&self, info: SessionInfo) {
        let mut sessions = self.discovered_sessions.write();
        // Update or add
        if let Some(existing) = sessions
            .iter_mut()
            .find(|s| s.guid_instance == info.guid_instance)
        {
            *existing = info;
        } else {
            sessions.push(info);
        }
    }

    /// Clear discovered sessions
    pub fn clear_discovered_sessions(&self) {
        self.discovered_sessions.write().clear();
    }

    /// Get discovered sessions
    pub fn get_discovered_sessions(&self) -> Vec<SessionInfo> {
        self.discovered_sessions.read().clone()
    }

    /// Get session descriptor
    pub fn get_session_desc(&self) -> Option<SessionDesc> {
        self.session.read().as_ref().map(|s| s.desc.clone())
    }

    /// Update session descriptor
    pub fn update_session_desc<F>(&self, f: F)
    where
        F: FnOnce(&mut SessionDesc),
    {
        if let Some(ref mut session) = *self.session.write() {
            f(&mut session.desc);
        }
    }

    /// Get session info for broadcasting
    pub fn get_session_info(&self) -> Option<SessionInfo> {
        let session = self.session.read();
        let session = session.as_ref()?;
        let node_id = self.our_node_id()?.clone();
        let ticket = self.our_ticket();
        Some(session.to_session_info(node_id, ticket))
    }

    /// Create a local player (host assigns ID, client uses provided ID)
    pub fn create_local_player(&self, name: PlayerName, flags: u32) -> Option<DPID> {
        let mut session = self.session.write();
        let session = session.as_mut()?;

        if session.is_full() {
            return None;
        }

        let player_id = if session.is_host {
            session.allocate_player_id()
        } else {
            // Client should use create_local_player_with_id
            return None;
        };

        let node_id = self.our_node_id().unwrap_or([0; 32]);
        let ticket = self.our_ticket();
        let player = Player::new_local(player_id, name, flags, node_id, ticket);
        session.add_player(player);
        session.local_player_id = Some(player_id);

        Some(player_id)
    }

    /// Create local player with specific ID (for clients after host assigns ID)
    pub fn create_local_player_with_id(&self, player_id: DPID, name: PlayerName, flags: u32) -> bool {
        let mut session = self.session.write();
        let session = match session.as_mut() {
            Some(s) => s,
            None => return false,
        };

        if session.is_full() {
            return false;
        }

        let node_id = self.our_node_id().unwrap_or([0; 32]);
        let ticket = self.our_ticket();
        let player = Player::new_local(player_id, name, flags, node_id, ticket);
        session.add_player(player);
        session.local_player_id = Some(player_id);

        true
    }

    /// Add a remote player (called when player joins)
    pub fn add_remote_player(&self, info: PlayerInfo) -> bool {
        let player_id = info.player_id;
        let mut session = self.session.write();
        if let Some(ref mut session) = *session {
            if session.is_full() {
                return false;
            }
            let mut player = Player::from_info(info);
            // Merge any player-data update that raced ahead of this registration.
            // The pending value is an explicit later SetPlayerData, so it wins
            // over whatever data the registration itself carried.
            if let Some(pending) = self.pending_player_data.write().remove(&player_id) {
                tracing::info!(player_id, data_len = pending.len(),
                    "applying player-data update that arrived before registration");
                player.data = pending;
            }
            session.add_player(player);
            true
        } else {
            false
        }
    }

    /// Add a joiner at its JoinRequest (host only), held back from the host's
    /// game until [`rename_remote_player`](Self::rename_remote_player) sees its
    /// CreatePlayer.
    ///
    /// The JoinRequest comes at the joiner's Open, before its game calls
    /// CreatePlayer with the player's name. The host's game (SMAC) registers a
    /// player once, at its CREATEPLAYERORGROUP, under the long name
    /// GetPlayerName returns right then, and ignores SETPLAYERORGROUPNAME (its
    /// handler is a no-op). Announced at the JoinRequest, the joiner is named ""
    /// for the whole game, on every machine, since joiners copy the host's names.
    pub fn add_joiner(&self, info: PlayerInfo) -> bool {
        let player_id = info.player_id;
        if !self.add_remote_player(info) {
            return false;
        }
        self.with_session_mut(|s| s.awaiting_create_player.insert(player_id))
            .is_some()
    }

    /// Record a remote player's new name. Returns the player if this is a
    /// joiner's CreatePlayer — its first name since [`add_joiner`](Self::add_joiner)
    /// — so the caller announces it to the local game now, already named.
    pub fn rename_remote_player(&self, player_id: DPID, name: PlayerName) -> Option<PlayerInfo> {
        let mut session = self.session.write();
        let session = session.as_mut()?;
        let player = session.players.get_mut(&player_id)?;
        player.name = name;
        session
            .awaiting_create_player
            .remove(&player_id)
            .then(|| player.to_info())
    }

    /// Remove a player
    pub fn remove_player(&self, player_id: DPID) -> Option<Player> {
        self.pending_player_data.write().remove(&player_id);
        let mut session = self.session.write();
        session.as_mut()?.remove_player(player_id)
    }

    /// Get local player ID
    pub fn local_player_id(&self) -> Option<DPID> {
        self.session.read().as_ref()?.local_player_id
    }

    /// Update a player's name (used when client calls CreatePlayer after joining)
    pub fn update_player_name(&self, player_id: DPID, name: PlayerName) -> bool {
        let mut session = self.session.write();
        if let Some(ref mut session) = *session {
            if let Some(player) = session.players.get_mut(&player_id) {
                player.name = name;
                return true;
            }
        }
        false
    }

    /// Get all players
    pub fn get_players(&self) -> Vec<Player> {
        self.session
            .read()
            .as_ref()
            .map(|s| s.players.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Get all players as PlayerInfo (for network transmission)
    pub fn get_player_infos(&self) -> Vec<PlayerInfo> {
        self.session
            .read()
            .as_ref()
            .map(|s| s.get_player_infos())
            .unwrap_or_default()
    }

    /// Get player by ID
    pub fn get_player(&self, player_id: DPID) -> Option<Player> {
        self.session
            .read()
            .as_ref()?
            .get_player(player_id)
            .cloned()
    }

    /// Update player data.
    ///
    /// A SHARED (non-local) update for a player we don't know yet is buffered
    /// and merged at registration — never silently dropped. Local data is only
    /// ever set by the local game for an existing player, so it isn't buffered.
    pub fn update_player_data(&self, player_id: DPID, data: Vec<u8>, local: bool) {
        let mut session = self.session.write();
        if let Some(ref mut session) = *session {
            if let Some(player) = session.get_player_mut(player_id) {
                if local {
                    player.local_data = data;
                } else {
                    player.data = data;
                }
                return;
            }
        }
        drop(session);
        if !local {
            tracing::warn!(player_id, data_len = data.len(),
                "player-data update for unregistered player — buffering until registration");
            self.pending_player_data.write().insert(player_id, data);
        }
    }

    /// Get player data
    pub fn get_player_data(&self, player_id: DPID, local: bool) -> Option<Vec<u8>> {
        let session = self.session.read();
        let session = session.as_ref()?;
        let player = session.get_player(player_id)?;
        Some(if local {
            player.local_data.clone()
        } else {
            player.data.clone()
        })
    }

    /// Allocate player ID (host only)
    pub fn allocate_player_id(&self) -> Option<DPID> {
        let session = self.session.read();
        let session = session.as_ref()?;
        if session.is_host {
            Some(session.allocate_player_id())
        } else {
            None
        }
    }

    /// With session read access
    pub fn with_session<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&Session) -> R,
    {
        self.session.read().as_ref().map(f)
    }

    /// With session write access
    pub fn with_session_mut<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut Session) -> R,
    {
        self.session.write().as_mut().map(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let manager = SessionManager::new();
        manager.set_identity([1; 32], "test_ticket".to_string());
        assert_eq!(manager.state(), SessionState::None);

        let desc = SessionDesc {
            guid_instance: GUID::new_random(),
            guid_application: GUID::new_random(),
            session_name: "Test".to_string(),
            max_players: 8,
            ..Default::default()
        };

        manager.create_session(desc);
        assert_eq!(manager.state(), SessionState::Hosting);
        assert!(manager.is_host());
    }

    #[test]
    fn test_player_management() {
        let manager = SessionManager::new();
        manager.set_identity([1; 32], "test_ticket".to_string());

        let desc = SessionDesc {
            guid_instance: GUID::new_random(),
            guid_application: GUID::new_random(),
            session_name: "Test".to_string(),
            max_players: 8,
            ..Default::default()
        };

        manager.create_session(desc);

        let name = PlayerName {
            short_name: "Player1".to_string(),
            long_name: "Player One".to_string(),
        };

        let player_id = manager.create_local_player(name, 0).unwrap();
        // First player gets 0x10000
        assert_eq!(player_id, 0x10000);

        let players = manager.get_players();
        assert_eq!(players.len(), 1);
        assert_eq!(players[0].id, player_id);
        assert_eq!(players[0].ticket, "test_ticket");
    }

    #[test]
    fn test_player_id_allocation() {
        let manager = SessionManager::new();
        manager.set_identity([1; 32], "test_ticket".to_string());

        let desc = SessionDesc {
            guid_instance: GUID::new_random(),
            guid_application: GUID::new_random(),
            session_name: "Test".to_string(),
            max_players: 8,
            ..Default::default()
        };

        manager.create_session(desc);

        // Allocate several IDs
        let id1 = manager.allocate_player_id().unwrap();
        let id2 = manager.allocate_player_id().unwrap();
        let id3 = manager.allocate_player_id().unwrap();

        assert_eq!(id1, 0x10000);
        assert_eq!(id2, 0x20000);
        assert_eq!(id3, 0x30000);
    }

    #[test]
    fn test_player_data_update_before_registration_is_buffered() {
        let manager = SessionManager::new();
        manager.set_identity([1; 32], "test_ticket".to_string());

        let desc = SessionDesc {
            guid_instance: GUID::new_random(),
            guid_application: GUID::new_random(),
            session_name: "Test".to_string(),
            max_players: 8,
            ..Default::default()
        };
        manager.create_session(desc);

        let player_id: DPID = 0x20000;

        // Data update arrives BEFORE the player is registered (the create-vs-
        // data race inherent to mesh arrival order). Must be buffered, not dropped.
        manager.update_player_data(player_id, vec![0x02], false);
        assert_eq!(manager.get_player_data(player_id, false), None);

        // Registration carries stale/empty data; the buffered update must win.
        let info = PlayerInfo {
            player_id,
            name: PlayerName::default(),
            flags: 0,
            node_id: [2; 32],
            ticket: "joiner_ticket".to_string(),
            data: vec![],
        };
        assert!(manager.add_remote_player(info));
        assert_eq!(manager.get_player_data(player_id, false), Some(vec![0x02]));

        // Buffer entry is consumed: a re-registration must not resurrect old data.
        manager.remove_player(player_id);
        let info2 = PlayerInfo {
            player_id,
            name: PlayerName::default(),
            flags: 0,
            node_id: [2; 32],
            ticket: "joiner_ticket".to_string(),
            data: vec![0x07],
        };
        assert!(manager.add_remote_player(info2));
        assert_eq!(manager.get_player_data(player_id, false), Some(vec![0x07]));
    }

    #[test]
    fn test_player_data_update_after_registration_applies_directly() {
        let manager = SessionManager::new();
        manager.set_identity([1; 32], "test_ticket".to_string());

        let desc = SessionDesc {
            guid_instance: GUID::new_random(),
            guid_application: GUID::new_random(),
            session_name: "Test".to_string(),
            max_players: 8,
            ..Default::default()
        };
        manager.create_session(desc);

        let player_id: DPID = 0x20000;
        let info = PlayerInfo {
            player_id,
            name: PlayerName::default(),
            flags: 0,
            node_id: [2; 32],
            ticket: "joiner_ticket".to_string(),
            data: vec![],
        };
        assert!(manager.add_remote_player(info));

        manager.update_player_data(player_id, vec![0x02], false);
        assert_eq!(manager.get_player_data(player_id, false), Some(vec![0x02]));
    }
}
