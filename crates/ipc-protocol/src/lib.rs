//! IPC Protocol for DLL ↔ Helper communication
//!
//! This crate defines the message types exchanged between the dplayx DLL
//! (running under Wine) and the smac-helper (running natively on macOS).
//!
//! ## Wire Format
//!
//! Messages are length-prefixed:
//! ```text
//! [4-byte little-endian length][postcard payload]
//! ```

use dp_types::{PlayerName, SessionDesc, DPID, GUID};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Protocol version - increment when making breaking changes
pub const PROTOCOL_VERSION: u32 = 3;

/// Default port for the helper
pub const DEFAULT_PORT: u16 = 47624;

/// Errors from IPC operations
#[derive(Debug, Error)]
pub enum IpcError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Codec error: {0}")]
    Codec(#[from] postcard::Error),

    #[error("Protocol version mismatch: expected {expected}, got {got}")]
    VersionMismatch { expected: u32, got: u32 },

    #[error("Connection refused - is smac-helper running?")]
    ConnectionRefused,

    #[error("Helper returned error: {0}")]
    HelperError(String),

    #[error("Unexpected response type")]
    UnexpectedResponse,
}

/// Requests from DLL to Helper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcRequest {
    /// Initial handshake - must be first message
    Handshake { protocol_version: u32 },

    /// Create a new session as host
    CreateSession { desc: SessionDesc },

    /// Join a session using the host's ticket
    JoinSessionByTicket { host_ticket: String },

    /// Close the current session
    CloseSession,

    /// Enumerate available sessions
    EnumSessions { app_guid: GUID, timeout_ms: u32 },

    /// Create a local player
    CreatePlayer {
        name: PlayerName,
        flags: u32,
        /// Initial player data (from DirectPlay lpData parameter)
        data: Vec<u8>,
    },

    /// Destroy a player
    DestroyPlayer { player_id: DPID },

    /// Send a message to another player
    Send {
        from: DPID,
        to: DPID,
        data: Vec<u8>,
        guaranteed: bool,
    },

    /// Receive a message (non-blocking)
    Receive,

    /// Get number of pending messages
    GetMessageCount,

    /// Get our ticket for sharing with other players
    GetOurTicket,

    /// Check if we're the session host
    IsHost,

    /// Check if we're in a session
    InSession,

    /// Get list of all players in session
    GetPlayers,

    /// Get session description
    GetSessionDesc,

    /// Set player data
    SetPlayerData {
        player_id: DPID,
        data: Vec<u8>,
        local: bool,
    },

    /// Get player data
    GetPlayerData { player_id: DPID, local: bool },

    /// Set player name
    SetPlayerName { player_id: DPID, name: PlayerName },

    /// Set session description (host only)
    SetSessionDesc { desc: SessionDesc },
}

impl IpcRequest {
    /// Get a human-readable name for the request type (for logging)
    pub fn type_name(&self) -> &'static str {
        match self {
            IpcRequest::Handshake { .. } => "Handshake",
            IpcRequest::CreateSession { .. } => "CreateSession",
            IpcRequest::JoinSessionByTicket { .. } => "JoinSessionByTicket",
            IpcRequest::CloseSession => "CloseSession",
            IpcRequest::EnumSessions { .. } => "EnumSessions",
            IpcRequest::CreatePlayer { .. } => "CreatePlayer",
            IpcRequest::DestroyPlayer { .. } => "DestroyPlayer",
            IpcRequest::Send { .. } => "Send",
            IpcRequest::Receive => "Receive",
            IpcRequest::GetMessageCount => "GetMessageCount",
            IpcRequest::GetOurTicket => "GetOurTicket",
            IpcRequest::IsHost => "IsHost",
            IpcRequest::InSession => "InSession",
            IpcRequest::GetPlayers => "GetPlayers",
            IpcRequest::GetSessionDesc => "GetSessionDesc",
            IpcRequest::SetPlayerData { .. } => "SetPlayerData",
            IpcRequest::GetPlayerData { .. } => "GetPlayerData",
            IpcRequest::SetPlayerName { .. } => "SetPlayerName",
            IpcRequest::SetSessionDesc { .. } => "SetSessionDesc",
        }
    }
}

/// Responses from Helper to DLL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcResponse {
    /// Handshake successful
    HandshakeOk {
        /// Our endpoint ID (32 bytes)
        endpoint_id: [u8; 32],
        /// Our ticket for sharing
        our_ticket: String,
    },

    /// Generic success
    Ok,

    /// Error occurred
    Error { message: String },

    /// Session was created
    SessionCreated { guid: GUID },

    /// Session was joined
    SessionJoined { player_id: DPID },

    /// List of available sessions
    SessionList { sessions: Vec<SessionListEntry> },

    /// Player was created
    PlayerCreated { player_id: DPID },

    /// A received message (or None if queue empty)
    Message { message: Option<QueuedMessage> },

    /// Number of pending messages
    MessageCount { count: usize },

    /// String value (for ticket, etc.)
    StringValue { value: String },

    /// Boolean value
    Bool { value: bool },

    /// List of players
    PlayerList { players: Vec<PlayerListEntry> },

    /// Session description
    SessionDescValue { desc: Option<SessionDesc> },

    /// Player data
    PlayerData { data: Option<Vec<u8>> },
}

/// A session entry for enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionListEntry {
    pub guid_instance: GUID,
    pub guid_application: GUID,
    pub session_name: String,
    pub max_players: u32,
    pub current_players: u32,
    pub flags: u32,
    pub host_ticket: String,
}

/// A player entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerListEntry {
    pub player_id: DPID,
    pub name: PlayerName,
    pub flags: u32,
    pub is_local: bool,
}

/// A queued message for DirectPlay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedMessage {
    pub from: DPID,
    pub to: DPID,
    pub data: Vec<u8>,
    pub guaranteed: bool,
}

/// Encode a request to bytes with length prefix
pub fn encode_request(req: &IpcRequest) -> Result<Vec<u8>, IpcError> {
    encode_with_length_prefix(req)
}

/// Encode a response to bytes with length prefix
pub fn encode_response(resp: &IpcResponse) -> Result<Vec<u8>, IpcError> {
    encode_with_length_prefix(resp)
}

fn encode_with_length_prefix<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, IpcError> {
    let payload = postcard::to_stdvec(value)?;
    let mut result = Vec::with_capacity(4 + payload.len());
    result.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    result.extend_from_slice(&payload);
    Ok(result)
}

/// Decode a request from bytes (without length prefix - caller handles framing)
pub fn decode_request(data: &[u8]) -> Result<IpcRequest, IpcError> {
    Ok(postcard::from_bytes(data)?)
}

/// Decode a response from bytes (without length prefix - caller handles framing)
pub fn decode_response(data: &[u8]) -> Result<IpcResponse, IpcError> {
    Ok(postcard::from_bytes(data)?)
}

/// Read a length-prefixed message from a reader
pub fn read_message<R: std::io::Read>(reader: &mut R) -> Result<Vec<u8>, IpcError> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;

    // Sanity check - don't allocate more than 16MB
    if len > 16 * 1024 * 1024 {
        return Err(IpcError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Message too large",
        )));
    }

    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;
    Ok(buf)
}

/// Write a length-prefixed message to a writer
pub fn write_message<W: std::io::Write>(writer: &mut W, data: &[u8]) -> Result<(), IpcError> {
    let len = data.len() as u32;
    writer.write_all(&len.to_le_bytes())?;
    writer.write_all(data)?;
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_roundtrip() {
        let req = IpcRequest::Handshake {
            protocol_version: PROTOCOL_VERSION,
        };

        let encoded = encode_request(&req).unwrap();
        // Skip the 4-byte length prefix
        let decoded = decode_request(&encoded[4..]).unwrap();

        match decoded {
            IpcRequest::Handshake { protocol_version } => {
                assert_eq!(protocol_version, PROTOCOL_VERSION);
            }
            _ => panic!("Wrong request type"),
        }
    }

    #[test]
    fn test_response_roundtrip() {
        let resp = IpcResponse::HandshakeOk {
            endpoint_id: [42; 32],
            our_ticket: "test_ticket".to_string(),
        };

        let encoded = encode_response(&resp).unwrap();
        let decoded = decode_response(&encoded[4..]).unwrap();

        match decoded {
            IpcResponse::HandshakeOk {
                endpoint_id,
                our_ticket,
            } => {
                assert_eq!(endpoint_id, [42; 32]);
                assert_eq!(our_ticket, "test_ticket");
            }
            _ => panic!("Wrong response type"),
        }
    }

    #[test]
    fn test_message_framing() {
        use std::io::Cursor;

        let data = b"hello world";
        let mut buffer = Vec::new();
        write_message(&mut buffer, data).unwrap();

        let mut cursor = Cursor::new(buffer);
        let read_data = read_message(&mut cursor).unwrap();

        assert_eq!(read_data, data);
    }

    #[test]
    fn test_queued_message() {
        let msg = QueuedMessage {
            from: 0x10000,
            to: 0x20000,
            data: vec![1, 2, 3, 4],
            guaranteed: true,
        };

        let resp = IpcResponse::Message {
            message: Some(msg.clone()),
        };

        let encoded = encode_response(&resp).unwrap();
        let decoded = decode_response(&encoded[4..]).unwrap();

        match decoded {
            IpcResponse::Message { message: Some(m) } => {
                assert_eq!(m.from, 0x10000);
                assert_eq!(m.to, 0x20000);
                assert_eq!(m.data, vec![1, 2, 3, 4]);
                assert!(m.guaranteed);
            }
            _ => panic!("Wrong response type"),
        }
    }
}
