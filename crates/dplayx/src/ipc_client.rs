//! IPC Client for communicating with datalink-mp
//!
//! This module provides a synchronous TCP client that connects to the
//! native datalink-mp process and proxies all Transport operations.

use dp_types::{PlayerName, SessionDesc, DPID, GUID};
use datalink_ipc::{
    decode_response, encode_request, read_message, IpcError, IpcRequest, IpcResponse,
    PlayerListEntry, QueuedMessage, SessionListEntry, DEFAULT_PORT, PROTOCOL_VERSION,
};
use parking_lot::Mutex;
use std::io::Write;
use std::net::TcpStream;
use tracing::{debug, error, info, warn};

/// IPC Client that connects to the native datalink-mp process
pub struct IpcClient {
    /// TCP stream to the helper
    stream: Mutex<TcpStream>,
    /// Our endpoint ID (from handshake)
    endpoint_id: [u8; 32],
    /// Our ticket (from handshake)
    our_ticket: String,
}

impl IpcClient {
    /// Connect to the helper process
    pub fn connect() -> Result<Self, IpcError> {
        let port = std::env::var("SMAC_HELPER_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_PORT);

        let addr = format!("127.0.0.1:{}", port);
        info!("IpcClient: connecting to {}", addr);

        let stream = TcpStream::connect(&addr).map_err(|e| {
            if e.kind() == std::io::ErrorKind::ConnectionRefused {
                error!("IpcClient: connection refused - is datalink-mp running?");
                IpcError::ConnectionRefused
            } else {
                IpcError::Io(e)
            }
        })?;

        // Set socket timeouts to prevent hanging forever if helper becomes unresponsive
        let timeout = std::time::Duration::from_secs(5);
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;

        // Disable Nagle's algorithm for lower latency
        stream.set_nodelay(true)?;

        info!("IpcClient: connected, performing handshake");

        let mut client = Self {
            stream: Mutex::new(stream),
            endpoint_id: [0; 32],
            our_ticket: String::new(),
        };

        // Perform handshake
        let response = client.send_request(&IpcRequest::Handshake {
            protocol_version: PROTOCOL_VERSION,
        })?;

        match response {
            IpcResponse::HandshakeOk {
                endpoint_id,
                our_ticket,
            } => {
                info!("IpcClient: handshake successful, endpoint_id={:?}", &endpoint_id[..8]);
                client.endpoint_id = endpoint_id;
                client.our_ticket = our_ticket;
                Ok(client)
            }
            IpcResponse::Error { message } => {
                error!("IpcClient: handshake failed: {}", message);
                Err(IpcError::HelperError(message))
            }
            _ => {
                error!("IpcClient: unexpected handshake response");
                Err(IpcError::UnexpectedResponse)
            }
        }
    }

    /// Send a request and wait for response
    fn send_request(&self, request: &IpcRequest) -> Result<IpcResponse, IpcError> {
        debug!("send_request: acquiring stream lock for {}", request.type_name());
        let mut stream = self.stream.lock();
        debug!("send_request: got stream lock");

        // Encode and send
        let encoded = encode_request(request)?;
        debug!("send_request: encoded {} bytes", encoded.len());
        stream.write_all(&encoded)?;
        stream.flush()?;
        debug!("send_request: sent, waiting for response");

        // Read response
        let data = read_message(&mut *stream)?;
        debug!("send_request: received {} bytes", data.len());
        let response = decode_response(&data)?;
        debug!("send_request: decoded response");

        Ok(response)
    }

    /// Get our ticket for sharing with other players
    pub fn our_ticket(&self) -> &str {
        &self.our_ticket
    }

    /// Get our endpoint ID as bytes
    pub fn endpoint_id_bytes(&self) -> [u8; 32] {
        self.endpoint_id
    }

    /// Create a session (as host)
    pub fn create_session(&self, desc: SessionDesc) -> Option<GUID> {
        match self.send_request(&IpcRequest::CreateSession { desc }) {
            Ok(IpcResponse::SessionCreated { guid }) => Some(guid),
            Ok(IpcResponse::Error { message }) => {
                warn!("create_session failed: {}", message);
                None
            }
            Ok(_) => {
                warn!("create_session: unexpected response");
                None
            }
            Err(e) => {
                error!("create_session error: {:?}", e);
                None
            }
        }
    }

    /// Join a session using the host ticket
    pub fn join_session_by_ticket(&self, host_ticket: &str) -> Result<DPID, IpcError> {
        let response = self.send_request(&IpcRequest::JoinSessionByTicket {
            host_ticket: host_ticket.to_string(),
        })?;

        match response {
            IpcResponse::SessionJoined { player_id } => Ok(player_id),
            IpcResponse::Error { message } => Err(IpcError::HelperError(message)),
            _ => Err(IpcError::UnexpectedResponse),
        }
    }

    /// Close the current session
    pub fn close_session(&self) {
        if let Err(e) = self.send_request(&IpcRequest::CloseSession) {
            warn!("close_session error: {:?}", e);
        }
    }

    /// Enumerate sessions
    pub fn enum_sessions(&self, app_guid: &GUID, timeout_ms: u32) -> Vec<SessionListEntry> {
        match self.send_request(&IpcRequest::EnumSessions {
            app_guid: *app_guid,
            timeout_ms,
        }) {
            Ok(IpcResponse::SessionList { sessions }) => sessions,
            Ok(IpcResponse::Error { message }) => {
                warn!("enum_sessions failed: {}", message);
                Vec::new()
            }
            Ok(_) => {
                warn!("enum_sessions: unexpected response");
                Vec::new()
            }
            Err(e) => {
                error!("enum_sessions error: {:?}", e);
                Vec::new()
            }
        }
    }

    /// Create a local player
    pub fn create_player(&self, name: PlayerName, flags: u32, data: Vec<u8>) -> Option<DPID> {
        match self.send_request(&IpcRequest::CreatePlayer { name, flags, data }) {
            Ok(IpcResponse::PlayerCreated { player_id }) => Some(player_id),
            Ok(IpcResponse::Error { message }) => {
                warn!("create_player failed: {}", message);
                None
            }
            Ok(_) => {
                warn!("create_player: unexpected response");
                None
            }
            Err(e) => {
                error!("create_player error: {:?}", e);
                None
            }
        }
    }

    /// Destroy a player
    pub fn destroy_player(&self, player_id: DPID) -> bool {
        match self.send_request(&IpcRequest::DestroyPlayer { player_id }) {
            Ok(IpcResponse::Ok) => true,
            Ok(IpcResponse::Error { message }) => {
                warn!("destroy_player failed: {}", message);
                false
            }
            Ok(_) => false,
            Err(e) => {
                error!("destroy_player error: {:?}", e);
                false
            }
        }
    }

    /// Send a message
    pub fn send(&self, from: DPID, to: DPID, data: Vec<u8>, guaranteed: bool) -> Result<(), IpcError> {
        let response = self.send_request(&IpcRequest::Send {
            from,
            to,
            data,
            guaranteed,
        })?;

        match response {
            IpcResponse::Ok => Ok(()),
            IpcResponse::Error { message } => Err(IpcError::HelperError(message)),
            _ => Err(IpcError::UnexpectedResponse),
        }
    }

    /// Receive a message (non-blocking)
    pub fn receive(&self) -> Option<QueuedMessage> {
        match self.send_request(&IpcRequest::Receive) {
            Ok(IpcResponse::Message { message }) => message,
            Ok(IpcResponse::Error { message }) => {
                warn!("receive failed: {}", message);
                None
            }
            Ok(_) => None,
            Err(e) => {
                debug!("receive error: {:?}", e);
                None
            }
        }
    }

    /// Get message count
    pub fn message_count(&self) -> usize {
        match self.send_request(&IpcRequest::GetMessageCount) {
            Ok(IpcResponse::MessageCount { count }) => {
                debug!("message_count: got count={}", count);
                count
            }
            Ok(other) => {
                warn!("message_count: unexpected response type");
                debug!("message_count: response was {:?}", other);
                0
            }
            Err(e) => {
                warn!("message_count: error: {:?}", e);
                0
            }
        }
    }

    /// Check if we're the session host
    pub fn is_host(&self) -> bool {
        match self.send_request(&IpcRequest::IsHost) {
            Ok(IpcResponse::Bool { value }) => value,
            _ => false,
        }
    }

    /// Check if we're in a session
    pub fn in_session(&self) -> bool {
        match self.send_request(&IpcRequest::InSession) {
            Ok(IpcResponse::Bool { value }) => value,
            _ => false,
        }
    }

    /// Get list of players
    pub fn get_players(&self) -> Vec<PlayerListEntry> {
        match self.send_request(&IpcRequest::GetPlayers) {
            Ok(IpcResponse::PlayerList { players }) => players,
            _ => Vec::new(),
        }
    }

    /// Get session description
    pub fn get_session_desc(&self) -> Option<SessionDesc> {
        match self.send_request(&IpcRequest::GetSessionDesc) {
            Ok(IpcResponse::SessionDescValue { desc }) => desc,
            _ => None,
        }
    }

    /// Set player data
    pub fn set_player_data(&self, player_id: DPID, data: Vec<u8>, local: bool) {
        if let Err(e) = self.send_request(&IpcRequest::SetPlayerData {
            player_id,
            data,
            local,
        }) {
            warn!("set_player_data error: {:?}", e);
        }
    }

    /// Get player data
    pub fn get_player_data(&self, player_id: DPID, local: bool) -> Option<Vec<u8>> {
        match self.send_request(&IpcRequest::GetPlayerData { player_id, local }) {
            Ok(IpcResponse::PlayerData { data }) => data,
            _ => None,
        }
    }

    /// Set player name
    pub fn set_player_name(&self, player_id: DPID, name: PlayerName) -> bool {
        match self.send_request(&IpcRequest::SetPlayerName { player_id, name }) {
            Ok(IpcResponse::Ok) => true,
            Ok(IpcResponse::Error { message }) => {
                warn!("set_player_name error: {}", message);
                false
            }
            _ => false,
        }
    }

    /// Set session description (host only)
    pub fn set_session_desc(&self, desc: SessionDesc) -> bool {
        match self.send_request(&IpcRequest::SetSessionDesc { desc }) {
            Ok(IpcResponse::Ok) => true,
            Ok(IpcResponse::Error { message }) => {
                warn!("set_session_desc error: {}", message);
                false
            }
            _ => false,
        }
    }
}
