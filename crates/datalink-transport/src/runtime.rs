//! Runtime management for Iroh transport
//!
//! Provides the main entry point and manages the async runtime.
//!
//! ## Ticket Architecture
//!
//! - Each endpoint has a ticket (JSON-serialized EndpointAddr)
//! - Host shares ticket with joiners (via CLI or file)
//! - Joiners connect using `--host-ticket` to establish peer connection
//! - `enum_sessions()` queries all connected peers for their sessions

use crate::connection::{
    peer_protocol_alpn, ConnectionManager, DEFAULT_DIAL_TIMEOUT,
    STREAM_PROTO_VERSION,
};
use crate::inbox::{GameInbox, QueuedMessage, SystemEvent};
use crate::protocol::{Message, PlayerInfo, SessionInfo};
use crate::session::SessionManager;
use crate::{TransportError, TransportResult};
use dp_types::{dpid, PlayerName, SessionDesc, DPID, GUID};
use iroh::{Endpoint, EndpointId, SecretKey};
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::{Builder, Runtime};
use tracing::{debug, debug_span, error, info, warn};

/// Environment switch (testing aid, not documented for players): when set to a
/// truthy value the endpoint drops its IP transports so every connection is a
/// Relayed connection.
pub const RELAY_ONLY_ENV: &str = "DATALINK_RELAY_ONLY";

/// Whether a `DATALINK_RELAY_ONLY` value turns the switch on. Unset, empty, `0`
/// and `false` are off; anything else is on.
fn relay_only_enabled(value: Option<&str>) -> bool {
    match value.map(str::trim) {
        None | Some("") | Some("0") => false,
        Some(v) => !v.eq_ignore_ascii_case("false"),
    }
}

/// Network topology mode for message routing
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NetworkMode {
    /// Route messages based on destination only (TCP/IP point-to-point semantics)
    PointToPoint,
    /// All messages visible to all peers (IPX broadcast domain semantics)
    /// When a message arrives from any peer, the host relays it to all other peers
    #[default]
    BroadcastDomain,
}

/// Main transport instance
pub struct Transport {
    /// Tokio runtime
    runtime: Runtime,
    /// Iroh endpoint
    endpoint: Arc<Endpoint>,
    /// Connection manager
    connection_manager: Arc<ConnectionManager>,
    /// Session manager
    session_manager: Arc<SessionManager>,
    /// Our ticket string (for sharing)
    our_ticket: String,
    /// Everything the game will be handed, in order
    inbox: GameInbox,
}

/// Options for constructing a [`Transport`].
///
/// Production code uses the defaults. Tests use the options to stand in for a
/// Helper from a different build (another Peer protocol version) and to keep
/// dial-timeout tests short.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransportOptions {
    /// Peer protocol version this Transport speaks
    pub peer_protocol_version: u16,
    /// How long a dial may take before it fails with `TransportError::CantReach`
    pub dial_timeout: Duration,
}

impl Default for TransportOptions {
    fn default() -> Self {
        Self {
            peer_protocol_version: STREAM_PROTO_VERSION,
            dial_timeout: DEFAULT_DIAL_TIMEOUT,
        }
    }
}

impl Transport {
    /// Create a new transport instance
    pub fn new() -> TransportResult<Self> {
        Self::with_options(TransportOptions::default())
    }

    /// Create a new transport instance with non-default options
    pub fn with_options(options: TransportOptions) -> TransportResult<Self> {
        crate::capture::init();

        let runtime = Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| TransportError::Io(e))?;

        let inbox = GameInbox::new();
        let (endpoint, connection_manager, session_manager, our_ticket) =
            runtime.block_on(async { Self::init_async(options, inbox.clone()).await })?;

        Ok(Self {
            runtime,
            endpoint: Arc::new(endpoint),
            connection_manager: Arc::new(connection_manager),
            session_manager,
            our_ticket,
            inbox,
        })
    }

    async fn init_async(options: TransportOptions, inbox: GameInbox) -> TransportResult<(
        Endpoint,
        ConnectionManager,
        Arc<SessionManager>,
        String,
    )> {
        // Generate a new secret key for this instance
        let secret_key = SecretKey::generate();

        info!("Initializing Iroh endpoint...");

        // The N0 preset provides relay + address-lookup defaults (n0 dns/pkarr);
        // the builder pre-binds IPv4 0.0.0.0 with an OS-assigned port.
        let mut builder = Endpoint::builder(iroh::endpoint::presets::N0)
            .secret_key(secret_key)
            .alpns(vec![peer_protocol_alpn(options.peer_protocol_version)]);
        if relay_only_enabled(std::env::var(RELAY_ONLY_ENV).ok().as_deref()) {
            info!("{RELAY_ONLY_ENV} is on: IP transports cleared, all connections will be Relayed");
            builder = builder.clear_ip_transports();
        }
        let endpoint = match builder
            .bind()
            .await
        {
            Ok(ep) => ep,
            Err(e) => {
                // Log the full error chain for debugging
                error!("Endpoint bind failed: {}", e);
                error!("Error details: {:?}", e);
                return Err(TransportError::Io(std::io::Error::other(format!("{:?}", e))));
            }
        };

        info!("Endpoint ID: {}", endpoint.id());

        // Generate our ticket (EndpointAddr as base32-encoded postcard)
        let endpoint_addr = endpoint.addr();
        let our_ticket = crate::ticket::Ticket::new(endpoint_addr).serialize();

        info!("Our ticket: {}", our_ticket);

        // Set up session manager with our identity
        let session_manager = Arc::new(SessionManager::new());
        session_manager.set_identity(*endpoint.id(), our_ticket.clone());

        let connection_manager =
            ConnectionManager::new(endpoint.clone(), session_manager.clone(), inbox, our_ticket.clone(), NetworkMode::default(), options);

        // Spawn connection acceptor
        let endpoint_clone = endpoint.clone();
        let cm = connection_manager.clone();
        tokio::spawn(async move {
            while let Some(incoming) = endpoint_clone.accept().await {
                match incoming.await {
                    Ok(connection) => {
                        // Get the remote endpoint_id from the connection after it's established
                        let endpoint_id = connection.remote_id();
                        if let Err(e) = cm.handle_incoming(connection, endpoint_id).await {
                            error!("Failed to handle incoming connection: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Failed to accept connection: {}", e);
                    }
                }
            }
        });

        Ok((endpoint, connection_manager, session_manager, our_ticket))
    }

    /// Get our ticket (for sharing with other players)
    pub fn our_ticket(&self) -> &str {
        &self.our_ticket
    }

    /// Get our endpoint ID
    pub fn endpoint_id(&self) -> EndpointId {
        self.endpoint.id()
    }

    /// Get our endpoint ID as bytes
    pub fn endpoint_id_bytes(&self) -> [u8; 32] {
        *self.endpoint.id()
    }

    /// Get session manager
    pub fn session_manager(&self) -> &Arc<SessionManager> {
        &self.session_manager
    }

    /// Create a session (as host)
    pub fn create_session(&self, desc: SessionDesc) -> Option<GUID> {
        let result = self.session_manager.create_session(desc);
        if result.is_some() {
            self.discard_earlier_sessions_messages();
            info!("Created session as host");
        }
        result
    }

    /// Empty the game's inbox as a Game session begins.
    ///
    /// The inbox outlives sessions: a peer's message about a session that has
    /// since closed (a Joiner leaving after the host's game quit) waits there,
    /// and the next game to host or join would be handed it. Nothing addressed
    /// to the new session can be waiting yet, so all of it is stale.
    fn discard_earlier_sessions_messages(&self) {
        self.inbox.session_began();
        self.connection_manager.game_session_began();
    }

    /// Join a session using the host ticket
    pub fn join_session(&self, session_info: &SessionInfo, _password: Option<&str>) -> TransportResult<DPID> {
        let host_ticket = &session_info.host_ticket;
        if host_ticket.is_empty() {
            return Err(TransportError::InvalidMessage);
        }

        self.discard_earlier_sessions_messages();

        let player_id = self.runtime.block_on(async {
            // Connect to host via ticket
            let peer = self.connection_manager.connect_by_ticket(host_ticket).await?;
            let host_endpoint_id = peer.endpoint_id;

            // Request to join (include our ticket so host can share it with others)
            let join_request = Message::JoinRequest {
                session_id: session_info.guid_instance,
                player_name: PlayerName::default(),
                player_data: Vec::new(),
                my_ticket: self.our_ticket.clone(),
            };

            // Send request and wait for response
            let response = self
                .connection_manager
                .send_and_receive(&host_endpoint_id, &join_request)
                .await?;

            match response {
                Message::JoinResponse {
                    accepted: true,
                    player_id: Some(id),
                    session_desc,
                    players,
                    ..
                } => {
                    // Set up session with host tracking for disconnect detection
                    if let Some(desc) = session_desc {
                        // Generate SETSESSIONDESC so the game knows the session settings
                        let sys_msg = dp_types::serialize_dpmsg_setsessiondesc(&desc);
                        self.inbox.add_system(SystemEvent::Message(sys_msg));
                        info!("Queued SETSESSIONDESC for game on join");

                        self.session_manager.join_session_with_host(desc, id, *host_endpoint_id);
                    }

                    // Add existing players and establish mesh connections
                    // Also queue DPSYS_CREATEPLAYERORGROUP for each so the game knows about them
                    for player in players {
                        // Try to connect directly to each existing player
                        if let Err(e) = self.connection_manager.establish_mesh_connection(&player).await {
                            warn!("Failed to establish mesh to {}: {}", player.player_id, e);
                        }
                        self.session_manager.add_remote_player(player.clone());

                        // Queue system message so game discovers this existing player
                        let current_players = self.session_manager.get_players().len() as u32;
                        self.inbox.add_system(SystemEvent::PlayerJoined { info: player, current_players });
                    }

                    // Queue CREATEPLAYERORGROUP for ourselves too. In real
                    // DirectPlay the joiner receives SUPERENUMPLAYERSREPLY, which
                    // includes their own player, and DirectPlay then generates
                    // CREATEPLAYERORGROUP for ALL players including self; the game
                    // does not consider itself joined until it sees this message.
                    // See DirectPlay spec MC-DPL4CS section 3.1.5.7.
                    let my_info = PlayerInfo {
                        player_id: id,
                        name: PlayerName::default(), // Will be updated when game calls CreatePlayer()
                        flags: 0,
                        node_id: self.endpoint_id_bytes(),
                        ticket: self.our_ticket.clone(),
                        data: vec![],
                    };
                    // Note: joiner is already in session_manager (added by join_session_with_host)
                    // so get_players().len() already includes us - don't add +1
                    let current_players = self.session_manager.get_players().len() as u32;
                    if self.inbox.add_system(SystemEvent::PlayerJoined { info: my_info, current_players }) {
                        info!("Queued CREATEPLAYERORGROUP for self (player_id={})", id);
                    }

                    Ok(id)
                }
                Message::JoinResponse {
                    accepted: false,
                    reason,
                    ..
                } => {
                    warn!("Join rejected: {:?}", reason);
                    Err(TransportError::SessionNotFound)
                }
                _ => Err(TransportError::InvalidMessage),
            }
        })?;

        Ok(player_id)
    }

    /// Join a session by host ticket (convenience method)
    pub fn join_session_by_ticket(&self, host_ticket: &str) -> TransportResult<DPID> {
        // First, query the host for session info
        let session_info = self.runtime.block_on(async {
            let peer = self.connection_manager.connect_by_ticket(host_ticket).await?;

            // Query for sessions
            let query = Message::SessionQuery {
                app_guid: GUID::zeroed(), // Match any app
            };

            let response = self.connection_manager.send_and_receive(&peer.endpoint_id, &query).await?;

            match response {
                Message::SessionList { sessions } if !sessions.is_empty() => {
                    Ok(sessions.into_iter().next().unwrap())
                }
                _ => Err(TransportError::SessionNotFound),
            }
        })?;

        self.join_session(&session_info, None)
    }

    /// Close the current session
    ///
    /// Only the host closing ends the session for everyone. A Joiner closing
    /// leaves it, as real DirectPlay does: its peers hear its player was
    /// destroyed, and the host keeps hosting.
    pub fn close_session(&self) {
        // Notify peers
        let msg = if self.session_manager.is_host() {
            Some(Message::SessionClosed {
                reason: "Host closed session".to_string(),
            })
        } else {
            // Nothing is queued for our own game: it is the one leaving.
            self.session_manager.local_player_id().map(|player_id| Message::PlayerLeft {
                player_id,
                reason: crate::protocol::LeaveReason::Disconnect,
            })
        };

        if let Some(msg) = msg {
            if let Err(e) = self.connection_manager.broadcast(&msg) {
                warn!("{} broadcast failed: {}", msg.type_name(), e);
            }
        }
        // Do NOT disconnect_all here. Session lifetime is decoupled from the
        // endpoint/connection lifetime: tearing down warm peer connections on
        // every Close breaks re-hosting and rediscovery within the same helper
        // (a peer that closes and re-hosts would become invisible because the
        // helper-to-helper connection was severed and nothing re-dials it).
        // Connections are dropped only on real transport loss (handle_incoming),
        // where the reconnect driver re-establishes them.

        self.session_manager.close_session();
    }

    /// Create a local player
    pub fn create_player(&self, name: PlayerName, flags: u32, data: Vec<u8>) -> Option<DPID> {
        // Client path: If we're not the host and already have a local player,
        // the player was created during join with a default name.
        // Just update the name and return the existing player ID.
        if !self.session_manager.is_host() {
            if let Some(player_id) = self.session_manager.local_player_id() {
                // Update the player's name
                self.session_manager.update_player_name(player_id, name.clone());

                // If data was provided, store it (false = remote/shared data, not local-only)
                if !data.is_empty() {
                    self.session_manager.update_player_data(player_id, data.clone(), false);
                }

                // Broadcast the name update to other players
                let name_msg = Message::PlayerNameUpdate {
                    player_id,
                    name,
                };

                // The player-data blob must be broadcast, not just stored.
                //
                // The game's CreatePlayer hands DirectPlay a 1-byte player-data value
                // (bit0 = is-host, bit1 = "eligible for reliable broadcast delivery";
                // a joiner's byte is 0x02). The blob is not known at Open/JoinRequest
                // time, so if it is not propagated after CreatePlayer, the host's
                // registration of this player keeps a flags byte of 0. With bit1
                // clear, the host's JACKAL send wrapper (DirectPlaySendWrapper
                // @0x64a210) skips the player in every reliable fan-out
                // (`test byte [slot+8], 2; je skip`) — game-state sync and the
                // game-start signal never arrive and the joiner hangs in the lobby.
                //
                // Broadcasting it makes the host synthesize
                // DPSYS_SETPLAYERORGROUPDATA (0x102) toward its own game, whose
                // handler writes the byte into this player's table slot (+0x168),
                // setting bit1 before the host starts broadcasting game state.
                let data_msg = (!data.is_empty()).then(|| Message::PlayerDataUpdate {
                    player_id,
                    data: data.clone(),
                    flags: 0, // shared (not DPSET_LOCAL)
                });

                // Synchronous enqueue: name-then-data order onto each peer's
                // ordered stream is guaranteed by construction.
                if let Err(e) = self.connection_manager.broadcast(&name_msg) {
                    warn!("PlayerNameUpdate broadcast failed: {}", e);
                }
                if let Some(data_msg) = data_msg {
                    if let Err(e) = self.connection_manager.broadcast(&data_msg) {
                        warn!("PlayerDataUpdate broadcast failed: {}", e);
                    }
                }

                return Some(player_id);
            }
        }

        // Host path: create new player
        let player_id = self.session_manager.create_local_player(name.clone(), flags)?;

        // Store initial player data if provided (false = remote/shared data)
        if !data.is_empty() {
            self.session_manager.update_player_data(player_id, data.clone(), false);
        }

        // Queue DPSYS_CREATEPLAYERORGROUP for the local game. Real DirectPlay
        // sends this message even for local player creation; the game receives
        // it via Receive() and processes it through NetworkPlayerStateHandler,
        // which computes the host flag (+0x1b30). Without it the host flag is
        // never set and multiplayer breaks.

        // The host's own player-data byte is 0x03 (bit0 is-host | bit1 reliable-
        // eligible). The game self-registers the host with this byte, and it must
        // also ride the roster to every joiner. Bake it deterministically rather
        // than depend on the game's CreatePlayer lpData (which our stack may drop).
        let host_data = if data.is_empty() {
            vec![dp_types::player_data_byte(true)]
        } else {
            data.clone()
        };
        let player_info = PlayerInfo {
            player_id,
            name: name.clone(),
            flags,
            node_id: self.endpoint_id_bytes(),
            ticket: self.our_ticket.clone(),
            data: host_data,
        };
        let current_players = self.session_manager.get_players().len() as u32;
        self.inbox.add_system(SystemEvent::PlayerJoined { info: player_info.clone(), current_players });
        info!("Queued CREATEPLAYERORGROUP for host's local player (player_id={})", player_id);

        // Broadcast to other players
        let msg = Message::PlayerJoined {
            player: player_info,
        };

        if let Err(e) = self.connection_manager.broadcast(&msg) {
            warn!("broadcast failed: {}", e);
        }

        Some(player_id)
    }

    /// Destroy a player
    pub fn destroy_player(&self, player_id: DPID) -> bool {
        // Permission check: only host can destroy other players,
        // or a player can destroy themselves
        let local_id = self.session_manager.local_player_id();
        let is_host = self.session_manager.is_host();

        if !is_host && local_id != Some(player_id) {
            // Not host and not destroying our own player
            return false;
        }

        if let Some(player) = self.session_manager.remove_player(player_id) {
            // Broadcast PlayerLeft to remote peers
            let msg = Message::PlayerLeft {
                player_id,
                reason: crate::protocol::LeaveReason::Disconnect,
            };

            if let Err(e) = self.connection_manager.broadcast(&msg) {
                warn!("broadcast failed: {}", e);
            }

            // Queue DPSYS_DESTROYPLAYERORGROUP for local game
            // This ensures the game receives the system message about player destruction
            self.inbox.add_system(SystemEvent::PlayerLeft {
                player_id,
                name: player.name.clone(),
                flags: player.flags,
            });

            true
        } else {
            false
        }
    }

    /// Set a player's name
    pub fn set_player_name(&self, player_id: DPID, name: PlayerName) -> bool {
        // Permission check: only owner can modify their name, or host can modify any
        let local_id = self.session_manager.local_player_id();
        let is_host = self.session_manager.is_host();

        if !is_host && local_id != Some(player_id) {
            // Not host and not modifying our own name
            return false;
        }

        if self.session_manager.update_player_name(player_id, name.clone()) {
            // Broadcast the name change
            let msg = Message::PlayerNameUpdate { player_id, name };
            if let Err(e) = self.connection_manager.broadcast(&msg) {
                warn!("broadcast failed: {}", e);
            }
            true
        } else {
            false
        }
    }

    /// Set session description (host only)
    pub fn set_session_desc(&self, desc: SessionDesc) -> bool {
        crate::capture::session_desc_set(&desc, self.session_manager.is_host());

        // Only host can modify session
        if !self.session_manager.is_host() {
            return false;
        }

        self.session_manager.update_session_desc(|existing| {
            *existing = desc.clone();
        });

        // Broadcast the update
        let msg = Message::SessionDescUpdate { session: desc };
        if let Err(e) = self.connection_manager.broadcast(&msg) {
            warn!("broadcast failed: {}", e);
        }

        true
    }

    /// Send a message
    pub fn send(
        &self,
        from: DPID,
        to: DPID,
        data: Vec<u8>,
        guaranteed: bool,
    ) -> TransportResult<()> {
        let size = data.len();
        let span = debug_span!("transport_send", from, to, size, guaranteed);
        let _guard = span.enter();

        let target = if to == dpid::DPID_ALLPLAYERS {
            "broadcast"
        } else if to == dpid::DPID_SERVERPLAYER {
            "server"
        } else {
            "direct"
        };
        debug!(target, "sending GameMessage");

        let flags = if guaranteed {
            dp_types::DPSEND_GUARANTEED
        } else {
            0
        };

        // Clone data for local loopback before it's moved into msg.
        // Loopback is needed for:
        // - DPID_SERVERPLAYER when we ARE the host: host receives its own server message
        // NOTE: DPID_ALLPLAYERS does NOT loopback to sender per Wine's behavior
        // (Wine uses excludeId=from in DP_QueueMessage to exclude sender)
        let needs_loopback = to == dpid::DPID_SERVERPLAYER && self.session_manager.is_host();
        let loopback_data = if needs_loopback {
            Some(data.clone())
        } else {
            None
        };

        let msg = Message::GameMessage {
            from,
            to,
            flags,
            data,
        };

        let cm = &self.connection_manager;
        let session_manager = &self.session_manager;

        // Synchronous enqueue onto per-peer ordered streams: successive Send
        // calls from the game reach each peer in exactly this order.
        let result = if to == dpid::DPID_ALLPLAYERS {
            // Broadcast to all remote peers
            // NOTE: No loopback to sender - Wine excludes sender from DPID_ALLPLAYERS
            cm.broadcast(&msg)
        } else if to == dpid::DPID_SERVERPLAYER {
            // Send to host/server only
            if session_manager.is_host() {
                // LOCAL LOOPBACK: Host sending to server (itself).
                // Game uses fire-and-receive pattern, expects to get message back.
                if let Some(data) = loopback_data {
                    let capture_id = crate::capture::loopback(from, to, guaranteed, &data);
                    self.inbox.add_loopback(from, to, data, guaranteed, capture_id);
                    debug!("queued local loopback for host->SERVERPLAYER");
                }
                Ok(())
            } else {
                // Send to the host
                if let Some(host_node_id) = session_manager.get_host_node_id() {
                    if let Some(endpoint_id) = iroh::EndpointId::try_from(host_node_id.as_slice()).ok() {
                        cm.send_to_peer(&endpoint_id, &msg)
                    } else {
                        Err(TransportError::NotConnected)
                    }
                } else {
                    Err(TransportError::NotConnected)
                }
            }
        } else {
            // Direct send to specific player
            if let Some(peer) = cm.get_peer_by_player(to) {
                cm.send_to_peer(&peer.endpoint_id, &msg)
            } else {
                Err(TransportError::PlayerNotFound)
            }
        };

        match &result {
            Ok(()) => debug!("send completed successfully"),
            Err(e) => {
                debug!(error = %e, "send failed");
                if let Message::GameMessage { data, .. } = &msg {
                    crate::capture::send_failed(from, to, guaranteed, data, &e.to_string());
                }
            }
        }

        result
    }

    /// Receive a message (non-blocking)
    pub fn receive(&self) -> Option<QueuedMessage> {
        let span = debug_span!("transport_receive");
        let _guard = span.enter();

        let msg = self.inbox.take();
        if msg.is_none() {
            debug!("no messages available");
        }
        msg
    }

    /// Get message count
    pub fn message_count(&self) -> usize {
        self.inbox.count()
    }

    /// Enumerate sessions (blocking)
    ///
    /// Queries all connected peers for their sessions.
    pub fn enum_sessions(&self, app_guid: &GUID, timeout_ms: u32) -> Vec<SessionInfo> {
        let connected_peers = self.connection_manager.get_connected_peers();

        if connected_peers.is_empty() {
            debug!("No connected peers, returning cached sessions");
            return self.session_manager.get_discovered_sessions();
        }

        info!("Querying {} connected peers for sessions (timeout={}ms)", connected_peers.len(), timeout_ms);

        // Query each connected peer with timeout.
        // Dedup the RETURNED list by guid_instance: the game never dedups
        // (SessionUIUpdate appends one UI row per EnumSessionsCallback), and in a
        // mesh the same hosted session can be reported by more than one peer. With
        // Item 1 (host-only answers) only the host should reply, but keep this as
        // defense-in-depth so the game callback fires at most once per instance.
        let mut all_sessions = Vec::new();
        let mut seen: std::collections::HashSet<GUID> = std::collections::HashSet::new();
        let timeout_duration = std::time::Duration::from_millis(timeout_ms.max(1000) as u64); // Minimum 1s timeout

        self.runtime.block_on(async {
            for endpoint_id in connected_peers {
                let query = Message::SessionQuery {
                    app_guid: *app_guid,
                };

                // Apply timeout to each peer query
                let query_future = self.connection_manager.send_and_receive(&endpoint_id, &query);
                match tokio::time::timeout(timeout_duration, query_future).await {
                    Ok(Ok(Message::SessionList { sessions })) => {
                        info!("Peer {} returned {} sessions", endpoint_id, sessions.len());
                        for session in sessions {
                            self.session_manager.add_discovered_session(session.clone());
                            if seen.insert(session.guid_instance) {
                                all_sessions.push(session);
                            } else {
                                debug!("enum_sessions: skipping duplicate session guid_instance");
                            }
                        }
                    }
                    Ok(Ok(_)) => {
                        warn!("Peer {} returned unexpected response", endpoint_id);
                    }
                    Ok(Err(e)) => {
                        warn!("Failed to query peer {}: {}", endpoint_id, e);
                    }
                    Err(_) => {
                        warn!("Timeout querying peer {} after {}ms", endpoint_id, timeout_ms);
                    }
                }
            }
        });

        all_sessions
    }

    /// Connect to a specific endpoint for session discovery
    pub fn connect_for_discovery(&self, endpoint_id_bytes: [u8; 32]) -> TransportResult<()> {
        let endpoint_id = endpoint_id_from_bytes(&endpoint_id_bytes)
            .ok_or(TransportError::InvalidMessage)?;

        self.runtime.block_on(async {
            self.connection_manager.connect(endpoint_id).await?;
            Ok(())
        })
    }

    /// Connect to a peer by their ticket (for establishing connection before session operations)
    pub fn connect_to_peer(&self, host_ticket: &str) -> TransportResult<()> {
        self.runtime.block_on(async {
            self.connection_manager.connect_by_ticket(host_ticket).await?;
            Ok(())
        })
    }

    /// Update player data
    pub fn set_player_data(&self, player_id: DPID, data: Vec<u8>, local: bool) {
        self.session_manager
            .update_player_data(player_id, data.clone(), local);

        if !local {
            // Broadcast to other players (only for non-local data)
            let msg = Message::PlayerDataUpdate {
                player_id,
                data,
                flags: 0, // Not local, so no DPSET_LOCAL flag
            };

            if let Err(e) = self.connection_manager.broadcast(&msg) {
                warn!("broadcast failed: {}", e);
            }
        }
    }

    /// Get player data
    pub fn get_player_data(&self, player_id: DPID, local: bool) -> Option<Vec<u8>> {
        self.session_manager.get_player_data(player_id, local)
    }

    /// Endpoint IDs of the Helpers currently connected to this one.
    ///
    /// A plain lock read: it never enters the Transport's runtime, so unlike the
    /// blocking calls above it is safe to call from async code (status requests).
    /// These are connected Helpers, not players: a peer connected only for
    /// session discovery is listed too.
    pub fn connected_peers(&self) -> Vec<EndpointId> {
        self.connection_manager.get_connected_peers()
    }

    /// Close the connection to one Helper while keeping the endpoint up, as a
    /// dropped network path would. For tests of connection loss.
    #[doc(hidden)]
    pub fn drop_connection_to(&self, endpoint_id: EndpointId) {
        self.connection_manager.disconnect(&endpoint_id);
    }

    /// How each connected Helper's connection goes, Direct or Relayed, once its
    /// path is selected. A plain lock read, like `connected_peers`.
    pub fn peer_paths(&self) -> Vec<(EndpointId, crate::path::PathClass)> {
        self.connection_manager.peer_paths()
    }

    /// Upper bound on how long `shutdown` blocks.
    pub const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);

    /// Close every peer connection and the endpoint gracefully (blocking).
    ///
    /// "Gracefully" means the connected Helpers are told: each gets a QUIC close
    /// and sees the connection end at once, instead of waiting out the 30 s idle
    /// timeout. Messages still queued for a peer are not flushed first.
    ///
    /// Blocks for at most `SHUTDOWN_TIMEOUT`; with a peer connected, expect about
    /// a second while the closed connections drain, and the whole bound if a
    /// peer vanished without closing. It waits on the Transport's
    /// runtime, so it must NOT be called on an async worker thread (tokio panics,
    /// and release builds abort on panic): from async code, call it inside
    /// `spawn_blocking`. The same goes for dropping the last reference to a
    /// Transport, before or after shutdown.
    ///
    /// Returns nothing because there is nothing for a caller to act on: the local
    /// side is closed either way, and running out of time only means a close may
    /// not have reached some peer (logged). Calling it again is a no-op.
    ///
    /// Afterwards this Transport is finished: `connected_peers()` is empty, dials
    /// fail and nothing is accepted, though the plain reads (`our_ticket`,
    /// `endpoint_id`, `session_manager`) still answer. A Transport cannot be
    /// reopened; Stop creates a new one, which has a new Ticket.
    pub fn shutdown(&self) {
        // Close the connections ourselves first so peers get our close code and
        // reason; the endpoint would otherwise close them with code 0 and no reason.
        self.connection_manager.disconnect_all();

        // Endpoint::close sends the close frames and waits until every connection
        // has drained; without the wait they would be lost with the socket.
        // It also ends the accept loop and fails any dial still in flight, so the
        // per-connection tasks wind down on their own. iroh makes only the first
        // close do anything, which is what makes a repeated shutdown safe.
        // The close frames go out at once, so the timeout only cuts the drain
        // short; left alone it can run slightly past 3 s for a vanished peer.
        let closed = self.runtime.block_on(async {
            tokio::time::timeout(Self::SHUTDOWN_TIMEOUT, self.endpoint.close()).await
        });
        if closed.is_err() {
            warn!(
                "Shutdown hit its {:?} bound before every connection drained",
                Self::SHUTDOWN_TIMEOUT
            );
        }
    }
}

impl Default for Transport {
    fn default() -> Self {
        Self::new().expect("Failed to create transport")
    }
}

/// Convert bytes to EndpointId
fn endpoint_id_from_bytes(bytes: &[u8; 32]) -> Option<EndpointId> {
    EndpointId::try_from(bytes.as_slice()).ok()
}

#[cfg(test)]
mod tests {
    #[test]
    fn relay_only_switch_reads_env_value() {
        use super::relay_only_enabled;
        assert!(!relay_only_enabled(None));
        assert!(!relay_only_enabled(Some("")));
        assert!(!relay_only_enabled(Some("0")));
        assert!(!relay_only_enabled(Some("false")));
        assert!(relay_only_enabled(Some("1")));
        assert!(relay_only_enabled(Some("true")));
    }

    use super::*;

    #[test]
    fn test_transport_creation() {
        // This test requires network access, so it may fail in restricted environments
        let transport = Transport::new();
        // Don't assert success - just check that we don't panic during creation attempt
        // Network binding may fail in containerized/sandboxed environments
        if transport.is_err() {
            eprintln!("Transport creation failed (expected in sandboxed environments): {:?}",
                     transport.err());
        }
    }
}
