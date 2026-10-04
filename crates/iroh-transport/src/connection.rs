//! Connection management for Iroh peers
//!
//! Handles establishing and maintaining connections to other players.
//!
//! ## Mesh Networking
//!
//! When a player joins, they connect to the host. The host broadcasts PlayerJoined
//! with the new player's ticket. All existing players then connect to the new player
//! directly, forming a full mesh.

use crate::protocol::{decode_message, encode_message, Message, PlayerInfo, SessionInfo};
use crate::runtime::{NetworkMode, TransportOptions};
use crate::session::SessionManager;
use crate::{TransportError, TransportResult};
use dp_types::structs::{serialize_dpmsg_setplayerorgroupdata, serialize_dpmsg_setplayerorgroupname, serialize_dpmsg_setsessiondesc};
use dp_types::{SessionDesc, DPID, GUID};

/// DPPLAYERTYPE_PLAYER constant (player, not group)
const DPPLAYERTYPE_PLAYER: u32 = 1;
use iroh::endpoint::{
    ConnectError, ConnectingError, Connection, ConnectionError, RecvStream, SendStream,
    TransportErrorCode,
};
use iroh::{Endpoint, EndpointAddr, EndpointId};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, debug_span, error, info, warn};

/// Peer protocol version: the version of the Helper-to-Helper wire protocol.
/// Bumped whenever the wire format changes so two Helpers from different builds
/// refuse each other LOUDLY instead of desyncing silently. It is carried twice:
/// in the ALPN (`datalink/<version>`, see `peer_protocol_alpn`), so a mismatch
/// is rejected in the QUIC handshake, and in the ordered-stream preamble as a
/// second line of defence.
/// v1: first datalink-mp release (postcard-encoded message payloads).
pub const STREAM_PROTO_VERSION: u16 = 1;

/// ALPN for a Peer protocol version: `datalink/<version>`. Two Helpers only
/// complete the QUIC handshake if their ALPNs, and so their versions, are equal.
pub fn peer_protocol_alpn(peer_protocol_version: u16) -> Vec<u8> {
    format!("datalink/{}", peer_protocol_version).into_bytes()
}

/// TLS alert `no_application_protocol` (RFC 7301): the accepting side supports
/// none of the ALPNs the dialler offered.
const TLS_ALERT_NO_APPLICATION_PROTOCOL: u8 = 120;

/// Classify a failed dial for the caller. The other Helper rejecting our ALPN
/// reaches us as a connection close carrying the TLS `no_application_protocol`
/// alert as a QUIC crypto error code: a Peer protocol version mismatch. Every
/// other failure means the other Helper could not be reached.
fn classify_dial_error(e: &ConnectError) -> TransportError {
    let connection_error = match e {
        ConnectError::Connecting {
            source: ConnectingError::ConnectionError { source, .. },
            ..
        } => Some(source),
        ConnectError::Connection { source, .. } => Some(source),
        _ => None,
    };
    match connection_error {
        Some(ConnectionError::ConnectionClosed(close))
            if close.error_code
                == TransportErrorCode::crypto(TLS_ALERT_NO_APPLICATION_PROTOCOL) =>
        {
            TransportError::PeerProtocolMismatch
        }
        _ => TransportError::CantReach,
    }
}

/// Default bound on a single dial (see `ConnectionManager::connect_by_ticket`).
pub const DEFAULT_DIAL_TIMEOUT: Duration = Duration::from_secs(15);

/// Magic prefix opening every ordered message stream: b"SMAC" + version (u16 LE).
const STREAM_MAGIC: &[u8; 4] = b"SMAC";

/// Upper bound on a single framed message (defensive against corrupt frames).
const MAX_FRAME_LEN: u32 = 1 << 20; // 1 MiB

/// A peer connection
#[derive(Debug)]
pub struct PeerConnection {
    /// The Iroh connection
    pub connection: Connection,
    /// Endpoint ID
    pub endpoint_id: EndpointId,
    /// Their ticket (for reconnection)
    pub ticket: String,
    /// Player IDs on this peer (uses Mutex for interior mutability since Arc::get_mut fails with multiple refs)
    pub player_ids: parking_lot::Mutex<Vec<DPID>>,
    /// Ordered outbound message queue. All sends to this peer are enqueued here
    /// (synchronously, so enqueue order == caller order) and written to ONE
    /// long-lived framed QUIC stream by the writer task. Per-peer FIFO delivery
    /// depends on this single-queue/single-stream design: separate streams or
    /// tasks per message would leave ordering to the scheduler.
    outbox: mpsc::UnboundedSender<Vec<u8>>,
}

impl PeerConnection {
    /// Create a peer and spawn its ordered-stream writer task.
    /// Must be called from within the tokio runtime context.
    fn spawn(
        connection: Connection,
        endpoint_id: EndpointId,
        ticket: String,
        peer_protocol_version: u16,
    ) -> Arc<Self> {
        let (outbox, outbox_rx) = mpsc::unbounded_channel();
        let peer = Arc::new(PeerConnection {
            connection: connection.clone(),
            endpoint_id,
            ticket,
            player_ids: parking_lot::Mutex::new(Vec::new()),
            outbox,
        });
        tokio::spawn(ordered_writer_task(
            connection,
            outbox_rx,
            endpoint_short(&endpoint_id),
            peer_protocol_version,
        ));
        peer
    }

    /// Enqueue an already-encoded message for ordered delivery to this peer.
    fn enqueue(&self, encoded: Vec<u8>) -> TransportResult<()> {
        self.outbox
            .send(encoded)
            .map_err(|_| TransportError::NotConnected)
    }
}

/// Short hex form of an endpoint id for log lines.
fn endpoint_short(endpoint_id: &EndpointId) -> String {
    let b = endpoint_id.as_bytes();
    format!("{:08x}", u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

/// Writer task: owns the single ordered outbound stream to one peer.
///
/// Opens the stream lazily on the first message (preamble: magic + version),
/// then writes 4-byte-LE length-prefixed frames in queue order. On a write
/// error it reopens the stream ONCE and retries the failed message; if that
/// also fails the connection is closed so the accept-loop cleanup fires
/// ConnectionLost and callers see NotConnected on later sends. Errors are
/// logged loudly — control-plane messages must never disappear silently.
async fn ordered_writer_task(
    connection: Connection,
    mut outbox_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    peer_label: String,
    peer_protocol_version: u16,
) {
    let mut stream: Option<SendStream> = None;

    async fn open_stream(
        connection: &Connection,
        peer_protocol_version: u16,
    ) -> TransportResult<SendStream> {
        let mut s = connection.open_uni().await?;
        let mut preamble = [0u8; 6];
        preamble[..4].copy_from_slice(STREAM_MAGIC);
        preamble[4..].copy_from_slice(&peer_protocol_version.to_le_bytes());
        s.write_all(&preamble).await?;
        Ok(s)
    }

    async fn write_frame(s: &mut SendStream, data: &[u8]) -> TransportResult<()> {
        s.write_all(&(data.len() as u32).to_le_bytes()).await?;
        s.write_all(data).await?;
        Ok(())
    }

    while let Some(data) = outbox_rx.recv().await {
        // Ensure we have a stream
        if stream.is_none() {
            match open_stream(&connection, peer_protocol_version).await {
                Ok(s) => stream = Some(s),
                Err(e) => {
                    error!(peer = %peer_label, error = %e, "failed to open ordered stream; dropping connection");
                    connection.close(1u32.into(), b"stream open failed");
                    return;
                }
            }
        }

        let s = stream.as_mut().expect("stream set above");
        if let Err(e) = write_frame(s, &data).await {
            warn!(peer = %peer_label, error = %e, "ordered stream write failed; reopening once");
            match open_stream(&connection, peer_protocol_version).await {
                Ok(mut s2) => {
                    if let Err(e2) = write_frame(&mut s2, &data).await {
                        error!(peer = %peer_label, error = %e2, "ordered stream retry failed; dropping connection");
                        connection.close(1u32.into(), b"stream write failed");
                        return;
                    }
                    stream = Some(s2);
                }
                Err(e2) => {
                    error!(peer = %peer_label, error = %e2, "ordered stream reopen failed; dropping connection");
                    connection.close(1u32.into(), b"stream reopen failed");
                    return;
                }
            }
        }
    }

    // Outbox closed (peer removed) — finish the stream gracefully.
    if let Some(mut s) = stream {
        let _ = s.finish();
    }
    debug!(peer = %peer_label, "ordered writer task exiting");
}

/// Messages received from the network
#[derive(Debug, Clone)]
pub enum ReceivedMessage {
    /// Session discovered
    SessionDiscovered(SessionInfo),
    /// Session list received
    SessionList(Vec<SessionInfo>),
    /// Player joined our session
    PlayerJoined(PlayerInfo),
    /// Player left
    PlayerLeft(DPID),
    /// Game message
    GameMessage {
        from: DPID,
        to: DPID,
        data: Vec<u8>,
        guaranteed: bool,
    },
    /// Join response received
    JoinResponse {
        accepted: bool,
        player_id: Option<DPID>,
        reason: Option<String>,
        session_desc: Option<SessionDesc>,
        players: Vec<PlayerInfo>,
    },
    /// Session closed
    SessionClosed,
    /// Connection lost to a peer
    ConnectionLost(EndpointId),
    /// DirectPlay system message to deliver to game (already serialized)
    SystemMessage(Vec<u8>),
}

/// Connection manager for mesh networking
pub struct ConnectionManager {
    /// Iroh endpoint
    endpoint: Endpoint,
    /// Connected peers by EndpointId bytes
    peers: Arc<RwLock<HashMap<[u8; 32], Arc<PeerConnection>>>>,
    /// Map from DPID to EndpointId for message routing
    player_routes: Arc<RwLock<HashMap<DPID, [u8; 32]>>>,
    /// Session manager
    session_manager: Arc<SessionManager>,
    /// Received messages queue
    message_tx: mpsc::UnboundedSender<ReceivedMessage>,
    /// Our endpoint ID
    endpoint_id: EndpointId,
    /// Our ticket
    our_ticket: String,
    /// Network mode for message routing (IPX broadcast vs TCP/IP point-to-point)
    network_mode: NetworkMode,
    /// Peer protocol version and dial timeout
    options: TransportOptions,
}

impl Clone for ConnectionManager {
    fn clone(&self) -> Self {
        Self {
            endpoint: self.endpoint.clone(),
            peers: self.peers.clone(),
            player_routes: self.player_routes.clone(),
            session_manager: self.session_manager.clone(),
            message_tx: self.message_tx.clone(),
            endpoint_id: self.endpoint_id,
            our_ticket: self.our_ticket.clone(),
            network_mode: self.network_mode,
            options: self.options,
        }
    }
}

impl ConnectionManager {
    /// Create a new connection manager. The endpoint must advertise
    /// `peer_protocol_alpn(options.peer_protocol_version)`.
    pub fn new(
        endpoint: Endpoint,
        session_manager: Arc<SessionManager>,
        message_tx: mpsc::UnboundedSender<ReceivedMessage>,
        our_ticket: String,
        network_mode: NetworkMode,
        options: TransportOptions,
    ) -> Self {
        let endpoint_id = endpoint.id();
        Self {
            endpoint,
            peers: Arc::new(RwLock::new(HashMap::new())),
            player_routes: Arc::new(RwLock::new(HashMap::new())),
            session_manager,
            message_tx,
            endpoint_id,
            our_ticket,
            network_mode,
            options,
        }
    }

    /// Get the current network mode
    pub fn network_mode(&self) -> NetworkMode {
        self.network_mode
    }

    /// Get our endpoint ID
    pub fn endpoint_id(&self) -> EndpointId {
        self.endpoint_id
    }

    /// Get our endpoint ID as bytes
    pub fn endpoint_id_bytes(&self) -> [u8; 32] {
        *self.endpoint_id
    }

    /// Get our ticket
    pub fn our_ticket(&self) -> &str {
        &self.our_ticket
    }

    /// Parse a ticket string to EndpointAddr
    fn parse_ticket(ticket: &str) -> TransportResult<EndpointAddr> {
        crate::ticket::Ticket::parse(ticket)
            .map(|t| t.into_addr())
            .map_err(TransportError::from)
    }

    /// Connect to a peer by ticket
    pub async fn connect_by_ticket(&self, ticket: &str) -> TransportResult<Arc<PeerConnection>> {
        let endpoint_addr = Self::parse_ticket(ticket)?;

        let key: [u8; 32] = *endpoint_addr.id;

        // Check if already connected
        if let Some(peer) = self.peers.read().get(&key) {
            return Ok(peer.clone());
        }

        info!("Connecting to peer via ticket: {}", endpoint_addr.id);

        let alpn = peer_protocol_alpn(self.options.peer_protocol_version);
        // The dial timeout lives here, in the one place that dials, so every
        // caller gets it: join, mesh connections and the reconnect driver.
        let dial = self.endpoint.connect(endpoint_addr.clone(), &alpn);
        let connection = match tokio::time::timeout(self.options.dial_timeout, dial).await {
            Ok(Ok(connection)) => connection,
            Ok(Err(e)) => {
                let classified = classify_dial_error(&e);
                warn!(peer = %endpoint_addr.id, error = %e, "dial failed: {}", classified);
                return Err(classified);
            }
            Err(_) => {
                warn!(peer = %endpoint_addr.id, timeout = ?self.options.dial_timeout, "dial timed out");
                return Err(TransportError::CantReach);
            }
        };

        let peer = PeerConnection::spawn(
            connection.clone(),
            endpoint_addr.id,
            ticket.to_string(),
            self.options.peer_protocol_version,
        );

        self.peers.write().insert(key, peer.clone());

        // Outgoing connections need a receiver loop just like incoming ones:
        // without it we could send to the peer but never receive from them.
        self.spawn_connection_handler(connection, endpoint_addr.id);

        Ok(peer)
    }

    /// Connect to a peer by EndpointId (if we already have a connection)
    pub async fn connect(&self, endpoint_id: EndpointId) -> TransportResult<Arc<PeerConnection>> {
        let key: [u8; 32] = *endpoint_id;

        // Check if already connected
        if let Some(peer) = self.peers.read().get(&key) {
            return Ok(peer.clone());
        }

        Err(TransportError::NotConnected)
    }

    /// Add player route (DPID -> NodeId mapping)
    pub fn add_player_route(&self, player_id: DPID, node_id: [u8; 32]) {
        self.player_routes.write().insert(player_id, node_id);

        // Also update the peer's player list using interior mutability
        if let Some(peer) = self.peers.read().get(&node_id) {
            let mut player_ids = peer.player_ids.lock();
            if !player_ids.contains(&player_id) {
                player_ids.push(player_id);
            }
        }
    }

    /// Remove player route
    pub fn remove_player_route(&self, player_id: DPID) {
        self.player_routes.write().remove(&player_id);
    }

    /// Get peer by player ID
    pub fn get_peer_by_player(&self, player_id: DPID) -> Option<Arc<PeerConnection>> {
        let node_id = *self.player_routes.read().get(&player_id)?;
        self.peers.read().get(&node_id).cloned()
    }

    /// Disconnect from a peer
    pub fn disconnect(&self, endpoint_id: &EndpointId) {
        let key: [u8; 32] = **endpoint_id;
        if let Some(peer) = self.peers.write().remove(&key) {
            // Remove all player routes for this peer
            let mut routes = self.player_routes.write();
            routes.retain(|_, v| v != &key);

            peer.connection.close(0u32.into(), b"disconnect");
        }
    }

    /// Disconnect all peers
    pub fn disconnect_all(&self) {
        let mut peers = self.peers.write();
        for (_, peer) in peers.drain() {
            peer.connection.close(0u32.into(), b"session closed");
        }
        self.player_routes.write().clear();
    }

    /// Get connected peer count
    pub fn peer_count(&self) -> usize {
        self.peers.read().len()
    }

    /// Get all connected peer endpoint IDs
    pub fn get_connected_peers(&self) -> Vec<EndpointId> {
        self.peers.read()
            .values()
            .map(|p| p.endpoint_id)
            .collect()
    }

    /// Send a message to a specific peer by EndpointId.
    ///
    /// Synchronous: encodes and enqueues to the peer's ordered stream. Enqueue
    /// order across all send/broadcast calls IS the wire order for that peer,
    /// so callers get deterministic per-peer FIFO by calling these in order.
    pub fn send_to_peer(&self, endpoint_id: &EndpointId, msg: &Message) -> TransportResult<()> {
        let key: [u8; 32] = **endpoint_id;
        let peer = self
            .peers
            .read()
            .get(&key)
            .cloned()
            .ok_or(TransportError::NotConnected)?;

        peer.enqueue(encode_message(msg)?)
    }

    /// Send a message to a specific player (synchronous enqueue, see send_to_peer)
    pub fn send_to_player(&self, player_id: DPID, msg: &Message) -> TransportResult<()> {
        let node_id = *self.player_routes.read().get(&player_id)
            .ok_or(TransportError::PlayerNotFound)?;

        let peer = self.peers.read().get(&node_id)
            .cloned()
            .ok_or(TransportError::NotConnected)?;

        peer.enqueue(encode_message(msg)?)
    }

    /// Send a message to all peers (synchronous enqueue, see send_to_peer).
    ///
    /// Returns an error if ANY enqueue fails, preserving the last error.
    /// All peers are attempted regardless of individual failures.
    pub fn broadcast(&self, msg: &Message) -> TransportResult<()> {
        self.broadcast_filtered(None, msg)
    }

    /// Send a message to all peers except the specified one.
    ///
    /// Used when relaying: the excluded peer is the original sender.
    pub fn broadcast_except(&self, exclude: &EndpointId, msg: &Message) -> TransportResult<()> {
        self.broadcast_filtered(Some(**exclude), msg)
    }

    fn broadcast_filtered(&self, exclude: Option<[u8; 32]>, msg: &Message) -> TransportResult<()> {
        let data = encode_message(msg)?;
        let peers: Vec<_> = self.peers.read()
            .iter()
            .filter(|(id, _)| Some(**id) != exclude)
            .map(|(_, p)| p.clone())
            .collect();
        let peer_count = peers.len();
        let msg_type = msg.type_name();

        let span = debug_span!("broadcast", peer_count, msg_type);
        let _guard = span.enter();

        if peer_count == 0 {
            debug!("no peers to send to");
            return Ok(());
        }

        let mut last_error: Option<TransportError> = None;
        let mut success_count = 0;

        for peer in peers {
            match peer.enqueue(data.clone()) {
                Ok(()) => {
                    debug!(endpoint = %endpoint_short(&peer.endpoint_id), "enqueued");
                    success_count += 1;
                }
                Err(e) => {
                    warn!(endpoint = %endpoint_short(&peer.endpoint_id), error = %e, "enqueue failed (peer dead)");
                    last_error = Some(e);
                }
            }
        }

        debug!(success_count, peer_count, "broadcast enqueued");

        match last_error {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// Send and wait for response (bidirectional)
    pub async fn send_and_receive(&self, endpoint_id: &EndpointId, msg: &Message) -> TransportResult<Message> {
        let key: [u8; 32] = **endpoint_id;
        let peer = self
            .peers
            .read()
            .get(&key)
            .cloned()
            .ok_or(TransportError::NotConnected)?;

        let data = encode_message(msg)?;

        let (mut send, mut recv) = peer.connection.open_bi().await?;
        send.write_all(&data).await?;
        send.finish()?;

        let response_data = recv.read_to_end(65536).await?;
        if response_data.is_empty() {
            return Err(TransportError::InvalidMessage);
        }

        decode_message(&response_data).map_err(TransportError::Serialization)
    }

    /// Spawn a task to handle incoming streams on a connection.
    ///
    /// This MUST be called for BOTH incoming AND outgoing connections to ensure
    /// bidirectional message flow. Without this, outgoing connections can send
    /// but never receive messages.
    fn spawn_connection_handler(&self, connection: Connection, endpoint_id: EndpointId) {
        let endpoint_id_bytes: [u8; 32] = *endpoint_id;
        let session_manager = self.session_manager.clone();
        let message_tx = self.message_tx.clone();
        let peers = self.peers.clone();
        let player_routes = self.player_routes.clone();
        let cm = self.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    // Handle unidirectional streams: each is a long-lived ORDERED
                    // message stream (magic+version preamble, length-prefixed
                    // frames). One reader task per stream decodes and dispatches
                    // messages SEQUENTIALLY — dispatching concurrently would let
                    // in-order arrivals apply out of order. Normally a peer opens
                    // exactly one stream; a second appears only if the peer's
                    // writer had to reopen after an error.
                    result = connection.accept_uni() => {
                        match result {
                            Ok(recv) => {
                                let session_manager = session_manager.clone();
                                let message_tx = message_tx.clone();
                                let player_routes = player_routes.clone();
                                let cm = cm.clone();

                                tokio::spawn(async move {
                                    if let Err(e) = read_ordered_stream(
                                        recv,
                                        &session_manager,
                                        &message_tx,
                                        &player_routes,
                                        endpoint_id_bytes,
                                        &cm,
                                    ).await {
                                        warn!("Ordered stream reader ended with error: {}", e);
                                    }
                                });
                            }
                            Err(e) => {
                                debug!("Connection closed (uni): {}", e);
                                break;
                            }
                        }
                    }
                    // Handle bidirectional streams (request/response)
                    result = connection.accept_bi() => {
                        match result {
                            Ok((send, recv)) => {
                                let session_manager = session_manager.clone();
                                let cm = cm.clone();

                                tokio::spawn(async move {
                                    if let Err(e) = handle_bi_stream(
                                        send,
                                        recv,
                                        &session_manager,
                                        endpoint_id_bytes,
                                        &cm,
                                    ).await {
                                        warn!("Error handling bi stream: {}", e);
                                    }
                                });
                            }
                            Err(e) => {
                                debug!("Connection closed (bi): {}", e);
                                break;
                            }
                        }
                    }
                }
            }

            // Connection closed - cleanup
            peers.write().remove(&endpoint_id_bytes);
            player_routes.write().retain(|_, v| v != &endpoint_id_bytes);
            let _ = message_tx.send(ReceivedMessage::ConnectionLost(endpoint_id));
        });
    }

    /// Establish mesh connection to a new player
    pub async fn establish_mesh_connection(&self, player_info: &PlayerInfo) -> TransportResult<()> {
        // Skip if it's us
        if player_info.node_id == self.endpoint_id_bytes() {
            return Ok(());
        }

        // Skip if already connected
        if self.peers.read().contains_key(&player_info.node_id) {
            // Just update routing
            self.add_player_route(player_info.player_id, player_info.node_id);
            return Ok(());
        }

        // Skip if no ticket
        if player_info.ticket.is_empty() {
            warn!("No ticket for player {}, can't establish mesh", player_info.player_id);
            return Ok(());
        }

        info!("Establishing mesh connection to player {}", player_info.player_id);

        // Connect using their ticket
        match self.connect_by_ticket(&player_info.ticket).await {
            Ok(_peer) => {
                // Send Hello to identify ourselves
                let our_player_id = self.session_manager.local_player_id().unwrap_or(0);
                let hello = Message::Hello {
                    player_id: our_player_id,
                    ticket: self.our_ticket.clone(),
                };

                if let Some(endpoint_id) = EndpointId::try_from(player_info.node_id.as_slice()).ok() {
                    if let Err(e) = self.send_to_peer(&endpoint_id, &hello) {
                        warn!("Failed to send Hello: {}", e);
                    }
                }

                // Add routing
                self.add_player_route(player_info.player_id, player_info.node_id);

                info!("Mesh connection established to player {}", player_info.player_id);
            }
            Err(e) => {
                warn!("Failed to establish mesh connection to player {}: {}", player_info.player_id, e);
            }
        }

        Ok(())
    }

    /// Handle an incoming connection
    pub async fn handle_incoming(&self, connection: Connection, endpoint_id: EndpointId) -> TransportResult<()> {
        let endpoint_id_bytes: [u8; 32] = *endpoint_id;

        info!("Incoming connection from: {}", endpoint_id);

        // Check if we already have this peer
        let is_new = !self.peers.read().contains_key(&endpoint_id_bytes);

        if is_new {
            // Ticket will be filled in on Hello
            let peer = PeerConnection::spawn(
                connection.clone(),
                endpoint_id,
                String::new(),
                self.options.peer_protocol_version,
            );
            self.peers.write().insert(endpoint_id_bytes, peer);
        }

        // Use shared helper to spawn the connection handler
        self.spawn_connection_handler(connection, endpoint_id);

        Ok(())
    }
}

/// Read a peer's ordered message stream: validate the preamble, then decode and
/// dispatch length-prefixed frames one at a time, in order. Returns when the
/// stream ends (peer closed it) or on a framing/protocol error.
async fn read_ordered_stream(
    mut recv: RecvStream,
    session_manager: &SessionManager,
    message_tx: &mpsc::UnboundedSender<ReceivedMessage>,
    player_routes: &Arc<RwLock<HashMap<DPID, [u8; 32]>>>,
    sender_endpoint_id: [u8; 32],
    cm: &ConnectionManager,
) -> TransportResult<()> {
    let peer_label = format!("{:08x}", u32::from_be_bytes([
        sender_endpoint_id[0],
        sender_endpoint_id[1],
        sender_endpoint_id[2],
        sender_endpoint_id[3],
    ]));

    // Preamble: magic + Peer protocol version. A mismatch means one machine runs a
    // stale build — fail loudly, this is exactly the silent-desync we refuse.
    // The ALPN already refuses mismatched builds in the handshake; this check
    // stays as a second line of defence.
    let our_version = cm.options.peer_protocol_version;
    let mut preamble = [0u8; 6];
    if let Err(e) = recv.read_exact(&mut preamble).await {
        debug!(peer = %peer_label, error = %e, "stream ended before preamble");
        return Ok(());
    }
    if &preamble[..4] != STREAM_MAGIC {
        error!(peer = %peer_label, "BAD STREAM MAGIC — peer is running an incompatible (pre-ordered-stream) build");
        return Err(TransportError::ProtocolMismatch(0, our_version));
    }
    let their_version = u16::from_le_bytes([preamble[4], preamble[5]]);
    if their_version != our_version {
        error!(peer = %peer_label, their_version, our_version,
               "PEER PROTOCOL VERSION MISMATCH — one machine has a stale build");
        return Err(TransportError::ProtocolMismatch(their_version, our_version));
    }

    let mut len_buf = [0u8; 4];
    loop {
        match recv.read_exact(&mut len_buf).await {
            Ok(()) => {}
            Err(_) => {
                debug!(peer = %peer_label, "ordered stream ended");
                return Ok(());
            }
        }
        let len = u32::from_le_bytes(len_buf);
        if len == 0 || len > MAX_FRAME_LEN {
            error!(peer = %peer_label, len, "invalid frame length on ordered stream");
            return Err(TransportError::InvalidMessage);
        }
        let mut data = vec![0u8; len as usize];
        recv.read_exact(&mut data).await.map_err(|e| {
            warn!(peer = %peer_label, error = %e, "ordered stream truncated mid-frame");
            TransportError::Connection(e.to_string())
        })?;

        let msg = decode_message(&data)?;
        debug!(from_endpoint = %peer_label, msg_type = msg.type_name(), "received message");

        // Dispatch SEQUENTIALLY — the next frame is not read until this
        // message's state updates are applied.
        handle_peer_message(
            msg,
            session_manager,
            message_tx,
            player_routes,
            sender_endpoint_id,
            cm,
        )
        .await;
    }
}

/// Apply one received peer message. State updates happen inline (order matters);
/// slow side-effects (dialing new peers) are spawned so they don't stall the
/// sender's stream.
async fn handle_peer_message(
    msg: Message,
    session_manager: &SessionManager,
    message_tx: &mpsc::UnboundedSender<ReceivedMessage>,
    player_routes: &Arc<RwLock<HashMap<DPID, [u8; 32]>>>,
    sender_endpoint_id: [u8; 32],
    cm: &ConnectionManager,
) {
    match msg {
        Message::Hello { player_id, ticket: _their_ticket } => {
            debug!("Received Hello from player {}", player_id);
            // Update routing
            player_routes.write().insert(player_id, sender_endpoint_id);
        }

        Message::SessionAnnounce { session } => {
            debug!("Received session announcement: {}", session.session_name);
            session_manager.add_discovered_session(session.clone());
            let _ = message_tx.send(ReceivedMessage::SessionDiscovered(session));
        }

        Message::SessionList { sessions } => {
            debug!("Received session list with {} sessions", sessions.len());
            for session in &sessions {
                session_manager.add_discovered_session(session.clone());
            }
            let _ = message_tx.send(ReceivedMessage::SessionList(sessions));
        }

        Message::PlayerJoined { player } => {
            debug!("Player joined: {} ({})", player.player_id, player.name.short_name);

            // Add routing for this player
            player_routes.write().insert(player.player_id, player.node_id);

            // Add to session
            session_manager.add_remote_player(player.clone());

            // Establish mesh connection to the new player. Spawned: dialing can
            // take seconds and must not stall this peer's ordered stream; the
            // local state updates above already happened in order.
            {
                let cm = cm.clone();
                let player = player.clone();
                tokio::spawn(async move {
                    if let Err(e) = cm.establish_mesh_connection(&player).await {
                        warn!("Failed to establish mesh to new player: {}", e);
                    }
                });
            }

            let _ = message_tx.send(ReceivedMessage::PlayerJoined(player));
        }

        Message::PlayerLeft { player_id, .. } => {
            debug!("Player left: {}", player_id);
            player_routes.write().remove(&player_id);
            session_manager.remove_player(player_id);
            let _ = message_tx.send(ReceivedMessage::PlayerLeft(player_id));
        }

        Message::GameMessage {
            from,
            to,
            flags,
            data,
        } => {
            let guaranteed = flags & dp_types::DPSEND_GUARANTEED != 0;
            debug!(from, to, size = data.len(), guaranteed, "received GameMessage, queuing to channel");

            // Deliver to the local DLL/game.
            //
            // TOPOLOGY: full mesh. Every player holds a direct connection to every
            // other player, so the origin already delivered this GameMessage to us
            // directly — a broadcast reached every peer once, a directed message
            // reached only its target. We therefore do NOT relay and do NOT echo:
            //   * relaying would double-deliver to peers the origin already reached;
            //   * echoing to the sender would duplicate the copy JACKAL already gives
            //     the sender via its internal loopback queue (game @0x64a210).
            let _ = message_tx.send(ReceivedMessage::GameMessage {
                from,
                to,
                data,
                guaranteed,
            });
        }

        Message::SessionClosed { .. } => {
            let _ = message_tx.send(ReceivedMessage::SessionClosed);
        }

        Message::PlayerDataUpdate {
            player_id,
            data,
            flags,
        } => {
            let local = flags & dp_types::DPSET_LOCAL != 0;
            debug!(player_id, data_len = data.len(), local, "Received PlayerDataUpdate, generating DPMSG_SETPLAYERORGROUPDATA");

            // Update session state
            session_manager.update_player_data(player_id, data.clone(), local);

            // Generate DPMSG_SETPLAYERORGROUPDATA for the local game
            let sys_msg = serialize_dpmsg_setplayerorgroupdata(player_id, DPPLAYERTYPE_PLAYER, &data);
            let _ = message_tx.send(ReceivedMessage::SystemMessage(sys_msg));
        }

        Message::Ping { timestamp } => {
            debug!("Received ping: {}", timestamp);
        }

        Message::PlayerNameUpdate { player_id, name } => {
            // A joiner's first name is its CreatePlayer: only now does the host's
            // game hear of it, named, as real DirectPlay would tell it.
            if let Some(joiner) = session_manager.rename_remote_player(player_id, name.clone()) {
                info!(player_id, long = %name.long_name, "Joiner created its player, announcing it to the game");
                let _ = message_tx.send(ReceivedMessage::PlayerJoined(joiner));
                return;
            }

            // Otherwise a rename: tell the game, though SMAC ignores it.
            debug!(player_id, short = %name.short_name, long = %name.long_name, "Received player name update, generating DPMSG_SETPLAYERORGROUPNAME");
            let sys_msg = serialize_dpmsg_setplayerorgroupname(
                player_id,
                DPPLAYERTYPE_PLAYER,
                &name.short_name,
                &name.long_name,
            );
            let _ = message_tx.send(ReceivedMessage::SystemMessage(sys_msg));
        }

        Message::SessionDescUpdate { session: desc } => {
            debug!(session_name = %desc.session_name, "Received session desc update, generating DPMSG_SETSESSIONDESC");

            // Generate DPMSG_SETSESSIONDESC for the local game
            let sys_msg = serialize_dpmsg_setsessiondesc(&desc);
            let _ = message_tx.send(ReceivedMessage::SystemMessage(sys_msg));

            // Update session state
            session_manager.update_session_desc(|existing| {
                *existing = desc;
            });
        }

        Message::HostMigration { new_host_id } => {
            info!("Received host migration notification: new host is {}", new_host_id);
            // Update session to reflect new host
            session_manager.with_session_mut(|session| {
                // If we are the new host, update is_host flag
                if session.local_player_id == Some(new_host_id) {
                    session.is_host = true;
                    info!("We are now the host!");
                }
            });
        }

        Message::Disconnect => {
            debug!("Received disconnect notification from peer");
            // The connection will be cleaned up when it closes
        }

        _ => {
            debug!("Unhandled uni message type");
        }
    }
}

/// Handle a bidirectional stream (request/response)
async fn handle_bi_stream(
    mut send: SendStream,
    mut recv: RecvStream,
    session_manager: &SessionManager,
    sender_endpoint_id: [u8; 32],
    cm: &ConnectionManager,
) -> TransportResult<()> {
    let data = recv.read_to_end(65536).await?;
    if data.is_empty() {
        return Ok(());
    }

    let msg = decode_message(&data)?;

    // Returns (response message, optional player to broadcast)
    let (response, broadcast_player): (Option<Message>, Option<PlayerInfo>) = match msg {
        Message::SessionQuery { app_guid } => {
            debug!("Received session query for {:?}", app_guid);

            // Only the HOST answers a session query — real DirectPlay answers
            // DPMSGCMD_ENUMSESSIONSREQUEST solely in the name server, which is the
            // host (wine/dlls/dplayx/name_server.c builds the reply from its own
            // hosted lpSessionDesc). A joiner must NOT advertise a session it merely
            // joined: doing so (a) shows a duplicate row to a 3rd party (the game
            // never dedups by guidInstance) and (b) stamps the joiner's OWN ticket as
            // the "host", so selecting that row dials the wrong peer. Gating on
            // is_host() guarantees every returned entry carries the real host ticket.
            let sessions = if session_manager.is_host() {
                match session_manager.get_session_info() {
                    Some(info)
                        if info.guid_application == app_guid || app_guid == GUID::zeroed() =>
                    {
                        vec![info]
                    }
                    _ => vec![],
                }
            } else {
                vec![]
            };

            (Some(Message::SessionList { sessions }), None)
        }

        Message::JoinRequest {
            session_id,
            player_name,
            // The joiner's player-data is not used: JoinRequest is sent at Open,
            // before the game's CreatePlayer, so it is always empty. We register
            // the joiner with the deterministic eligibility byte below instead.
            player_data: _,
            my_ticket,
        } => {
            debug!("Received join request for session {:?}", session_id);

            // Check if we're hosting this session
            let session_desc = session_manager.get_session_desc();

            if let Some(desc) = session_desc {
                // Check if joins are disabled
                if desc.flags & dp_types::DPSESSION_JOINDISABLED != 0 {
                    (Some(Message::JoinResponse {
                        accepted: false,
                        player_id: None,
                        reason: Some("Session is closed to new players".to_string()),
                        session_desc: None,
                        players: vec![],
                    }), None)
                } else if desc.guid_instance == session_id && session_manager.is_host() {
                    // IDEMPOTENCY: if this physical node already has a player, this is
                    // a reconnect / retry / double-Open. Reuse the existing DPID — do
                    // NOT allocate a second one (allocate_player_id fetch_adds
                    // unconditionally), do NOT re-add or re-announce. Minting two DPIDs
                    // for one node is the genuine roster-corruption vector.
                    if let Some(existing_id) = session_manager.get_player_id_by_node(&sender_endpoint_id) {
                        // Refresh the route (endpoint may have changed on reconnect),
                        // return the OTHER players (exclude this node's own player),
                        // and emit NO PlayerJoined broadcast.
                        cm.add_player_route(existing_id, sender_endpoint_id);
                        let players: Vec<PlayerInfo> = session_manager
                            .get_player_infos()
                            .into_iter()
                            .filter(|p| p.player_id != existing_id)
                            .collect();
                        info!("JoinRequest from known node: reusing player_id {}", existing_id);
                        (Some(Message::JoinResponse {
                            accepted: true,
                            player_id: Some(existing_id),
                            reason: None,
                            session_desc: Some(session_manager.get_session_desc().unwrap_or(desc)),
                            players,
                        }), None)
                    } else if let Some(player_id) = session_manager.allocate_player_id() {
                        // Get existing player list BEFORE adding joiner
                        // This prevents the joiner from receiving CREATEPLAYERORGROUP about themselves
                        // (they already know they exist - they just joined!)
                        let players = session_manager.get_player_infos();

                        // Create and add the joiner as a remote player.
                        //
                        // The player-data byte is REQUIRED for reliable delivery.
                        // JACKAL's reliable-broadcast fan-out (DirectPlaySendWrapper
                        // @0x64a210) skips any player whose registered flag byte
                        // (mgr slot +0x168) lacks bit1 (0x2), dropping the host's
                        // game-start sync before it reaches our DLL. The byte is
                        // deterministic — for a joiner it is 0x02 (bit1 = reliable-
                        // eligible; bit0 = is-host, always clear for a joiner) — so we
                        // bake it into the CREATEPLAYERORGROUP the host registers from
                        // (sent at the joiner's CreatePlayer, see add_joiner),
                        // rather than depending on the game's (empty-in-practice)
                        // CreatePlayer lpData or a later SETPLAYERORGROUPDATA. This also
                        // makes 3+ players work: the host's roster (JACKAL type 0x10)
                        // copies this byte into every joiner's player table.
                        let joiner_info = PlayerInfo {
                            player_id,
                            name: player_name.clone(),
                            flags: 0,
                            node_id: sender_endpoint_id,
                            ticket: my_ticket.clone(),
                            data: vec![dp_types::player_data_byte(false)],
                        };
                        // The host's game isn't told yet: it hears of the joiner at
                        // the joiner's CreatePlayer, already named (see add_joiner).
                        session_manager.add_joiner(joiner_info.clone());
                        // Register the route so directed sends to this player resolve
                        cm.add_player_route(player_id, sender_endpoint_id);
                        info!("Added remote player {} to session with route", player_id);

                        // Get updated session desc (current_players count updated)
                        let updated_desc = session_manager.get_session_desc().unwrap_or(desc);

                        // Return response AND the player info for broadcast to other peers
                        (Some(Message::JoinResponse {
                            accepted: true,
                            player_id: Some(player_id),
                            reason: None,
                            session_desc: Some(updated_desc),
                            players,
                        }), Some(joiner_info))
                    } else {
                        (Some(Message::JoinResponse {
                            accepted: false,
                            player_id: None,
                            reason: Some("Session is full".to_string()),
                            session_desc: None,
                            players: vec![],
                        }), None)
                    }
                } else {
                    (Some(Message::JoinResponse {
                        accepted: false,
                        player_id: None,
                        reason: Some("Session not found".to_string()),
                        session_desc: None,
                        players: vec![],
                    }), None)
                }
            } else {
                (Some(Message::JoinResponse {
                    accepted: false,
                    player_id: None,
                    reason: Some("No session".to_string()),
                    session_desc: None,
                    players: vec![],
                }), None)
            }
        }

        Message::Ping { timestamp } => {
            (Some(Message::Pong { timestamp }), None)
        }

        _ => (None, None),
    };

    if let Some(response) = response {
        let response_data = encode_message(&response)?;
        send.write_all(&response_data).await?;
        send.finish()?;
    }

    // Broadcast PlayerJoined to other peers if a NEW player was accepted.
    // Exclude the newcomer itself: it already knows it exists (it just joined), and
    // sending it a PlayerJoined about itself makes it register a self-entry and dial
    // itself. (broadcast_player is None for a reconnect — see the idempotency branch.)
    if let Some(player_info) = broadcast_player {
        let broadcast_msg = Message::PlayerJoined { player: player_info };
        let result = match EndpointId::try_from(&sender_endpoint_id[..]) {
            Ok(newcomer) => cm.broadcast_except(&newcomer, &broadcast_msg),
            Err(_) => cm.broadcast(&broadcast_msg),
        };
        if let Err(e) = result {
            warn!("Failed to broadcast PlayerJoined: {}", e);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    // Connection tests require actual Iroh infrastructure
    // These would be integration tests
}
