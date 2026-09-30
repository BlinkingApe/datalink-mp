//! Wire protocol for DirectPlay messages over Iroh
//!
//! This module defines the message types sent between peers.
//!
//! ## Single Ticket Architecture
//!
//! The host shares ONE ticket. All players connect to host first,
//! then establish a full mesh by exchanging tickets in PlayerJoined messages.

use dp_types::{PlayerName, SessionDesc, DPID, GUID};
use serde::{Deserialize, Serialize};

/// Message types exchanged between peers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// Mesh peer handshake - sent when establishing direct P2P connection
    Hello {
        /// Our player ID
        player_id: DPID,
        /// Our ticket for reverse connection
        ticket: String,
    },

    /// Session announcement (host broadcasts this)
    SessionAnnounce {
        session: SessionInfo,
    },

    /// Query for available sessions
    SessionQuery {
        app_guid: GUID,
    },

    /// Response to session query
    SessionList {
        sessions: Vec<SessionInfo>,
    },

    /// Request to join a session
    JoinRequest {
        session_id: GUID,
        player_name: PlayerName,
        player_data: Vec<u8>,
        /// Joiner's ticket so others can connect for mesh
        my_ticket: String,
    },

    /// Response to join request
    JoinResponse {
        accepted: bool,
        player_id: Option<DPID>,
        reason: Option<String>,
        session_desc: Option<SessionDesc>,
        /// All current players with their tickets for mesh establishment
        players: Vec<PlayerInfo>,
    },

    /// Notification that a player joined (broadcast by host)
    PlayerJoined {
        /// Includes ticket for mesh establishment
        player: PlayerInfo,
    },

    /// Notification that a player left
    PlayerLeft {
        player_id: DPID,
        reason: LeaveReason,
    },

    /// Game data message (can be sent P2P after mesh established)
    GameMessage {
        from: DPID,
        to: DPID,
        flags: u32,
        data: Vec<u8>,
    },

    /// Player data update
    PlayerDataUpdate {
        player_id: DPID,
        data: Vec<u8>,
        flags: u32,
    },

    /// Player name update
    PlayerNameUpdate {
        player_id: DPID,
        name: PlayerName,
    },

    /// Session description update
    SessionDescUpdate {
        session: SessionDesc,
    },

    /// Ping for latency measurement
    Ping {
        timestamp: u64,
    },

    /// Pong response
    Pong {
        timestamp: u64,
    },

    /// Host migration notification
    HostMigration {
        new_host_id: DPID,
    },

    /// Session closed
    SessionClosed {
        reason: String,
    },

    /// Disconnect notification
    Disconnect,
}

impl Message {
    /// Get a human-readable name for the message type (for logging)
    pub fn type_name(&self) -> &'static str {
        match self {
            Message::Hello { .. } => "Hello",
            Message::SessionAnnounce { .. } => "SessionAnnounce",
            Message::SessionQuery { .. } => "SessionQuery",
            Message::SessionList { .. } => "SessionList",
            Message::JoinRequest { .. } => "JoinRequest",
            Message::JoinResponse { .. } => "JoinResponse",
            Message::PlayerJoined { .. } => "PlayerJoined",
            Message::PlayerLeft { .. } => "PlayerLeft",
            Message::GameMessage { .. } => "GameMessage",
            Message::PlayerDataUpdate { .. } => "PlayerDataUpdate",
            Message::PlayerNameUpdate { .. } => "PlayerNameUpdate",
            Message::SessionDescUpdate { .. } => "SessionDescUpdate",
            Message::Ping { .. } => "Ping",
            Message::Pong { .. } => "Pong",
            Message::HostMigration { .. } => "HostMigration",
            Message::SessionClosed { .. } => "SessionClosed",
            Message::Disconnect => "Disconnect",
        }
    }
}

/// Session information for discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub guid_instance: GUID,
    pub guid_application: GUID,
    pub session_name: String,
    pub max_players: u32,
    pub current_players: u32,
    pub flags: u32,
    pub host_node_id: [u8; 32],
    /// Host's ticket for connection
    pub host_ticket: String,
    pub password_required: bool,
    pub user1: u32,
    pub user2: u32,
    pub user3: u32,
    pub user4: u32,
}

impl SessionInfo {
    /// Create from SessionDesc with host info
    pub fn from_desc_with_host(desc: &SessionDesc, host_node_id: [u8; 32], host_ticket: String) -> Self {
        Self {
            guid_instance: desc.guid_instance,
            guid_application: desc.guid_application,
            session_name: desc.session_name.clone(),
            max_players: desc.max_players,
            current_players: desc.current_players,
            flags: desc.flags,
            host_node_id,
            host_ticket,
            password_required: desc.password.is_some(),
            user1: desc.user1,
            user2: desc.user2,
            user3: desc.user3,
            user4: desc.user4,
        }
    }
}

impl From<&SessionDesc> for SessionInfo {
    fn from(desc: &SessionDesc) -> Self {
        Self {
            guid_instance: desc.guid_instance,
            guid_application: desc.guid_application,
            session_name: desc.session_name.clone(),
            max_players: desc.max_players,
            current_players: desc.current_players,
            flags: desc.flags,
            host_node_id: [0; 32],
            host_ticket: String::new(),
            password_required: desc.password.is_some(),
            user1: desc.user1,
            user2: desc.user2,
            user3: desc.user3,
            user4: desc.user4,
        }
    }
}

impl From<SessionInfo> for SessionDesc {
    fn from(info: SessionInfo) -> Self {
        Self {
            flags: info.flags,
            guid_instance: info.guid_instance,
            guid_application: info.guid_application,
            max_players: info.max_players,
            current_players: info.current_players,
            session_name: info.session_name,
            password: None,
            user1: info.user1,
            user2: info.user2,
            user3: info.user3,
            user4: info.user4,
        }
    }
}

/// Player information with ticket for mesh networking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInfo {
    pub player_id: DPID,
    pub name: PlayerName,
    pub flags: u32,
    /// Node ID for identification
    pub node_id: [u8; 32],
    /// Full ticket for direct P2P connection (enables mesh)
    pub ticket: String,
    pub data: Vec<u8>,
}

impl PlayerInfo {
    /// Check if this is a local player (no ticket needed for self)
    pub fn is_local(&self) -> bool {
        self.ticket.is_empty()
    }
}

/// Reason for player leaving
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LeaveReason {
    /// Normal disconnect
    Disconnect,
    /// Connection lost
    ConnectionLost,
    /// Kicked by host
    Kicked,
    /// Session ended
    SessionEnded,
}

/// Encode a message to bytes
pub fn encode_message(msg: &Message) -> Result<Vec<u8>, postcard::Error> {
    postcard::to_stdvec(msg)
}

/// Decode a message from bytes
pub fn decode_message(data: &[u8]) -> Result<Message, postcard::Error> {
    postcard::from_bytes(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_roundtrip() {
        let msg = Message::Ping {
            timestamp: 12345,
        };
        let encoded = encode_message(&msg).unwrap();
        let decoded = decode_message(&encoded).unwrap();
        match decoded {
            Message::Ping { timestamp } => assert_eq!(timestamp, 12345),
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_player_info_with_ticket() {
        let info = PlayerInfo {
            player_id: 0x10000,
            name: PlayerName::default(),
            flags: 0,
            node_id: [1; 32],
            ticket: "iroh:test_ticket_abc123".to_string(),
            data: vec![],
        };

        let msg = Message::PlayerJoined { player: info };
        let encoded = encode_message(&msg).unwrap();
        let decoded = decode_message(&encoded).unwrap();

        match decoded {
            Message::PlayerJoined { player } => {
                assert_eq!(player.player_id, 0x10000);
                assert_eq!(player.ticket, "iroh:test_ticket_abc123");
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_join_request_with_ticket() {
        let msg = Message::JoinRequest {
            session_id: GUID::new_random(),
            player_name: PlayerName::default(),
            player_data: vec![1, 2, 3],
            my_ticket: "iroh:my_ticket_xyz".to_string(),
        };

        let encoded = encode_message(&msg).unwrap();
        let decoded = decode_message(&encoded).unwrap();

        match decoded {
            Message::JoinRequest { my_ticket, .. } => {
                assert_eq!(my_ticket, "iroh:my_ticket_xyz");
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_session_info_roundtrip() {
        let info = SessionInfo {
            guid_instance: GUID::new_random(),
            guid_application: GUID::new_random(),
            session_name: "Test Session".to_string(),
            max_players: 8,
            current_players: 1,
            flags: 0,
            host_node_id: [0; 32],
            host_ticket: "iroh:host_ticket".to_string(),
            password_required: false,
            user1: 0,
            user2: 0,
            user3: 0,
            user4: 0,
        };

        let msg = Message::SessionAnnounce {
            session: info.clone(),
        };
        let encoded = encode_message(&msg).unwrap();
        let decoded = decode_message(&encoded).unwrap();

        match decoded {
            Message::SessionAnnounce { session } => {
                assert_eq!(session.session_name, "Test Session");
                assert_eq!(session.host_ticket, "iroh:host_ticket");
            }
            _ => panic!("Wrong message type"),
        }
    }
}
