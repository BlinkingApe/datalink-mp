//! Connection management for Iroh peers
//!
//! Handles establishing and maintaining connections to other players.
//!
//! ## Mesh Networking
//!
//! When a player joins, they connect to the host. The host broadcasts PlayerJoined
//! with the new player's ticket. All existing players then connect to the new player
//! directly, forming a full mesh.

use crate::friend_pipeline::FriendPipeline;
use crate::friend_registry::{endpoint_short, FriendRegistry, Link, LinkEnded, LinkId, LocalFacts, RosterFacts};
use crate::inbox::{GameInbox, SystemEvent};
use crate::protocol::{decode_message, encode_message, Message, PlayerInfo};
use crate::runtime::{NetworkMode, TransportOptions};
use crate::session::{HostClosed, SessionManager};
use crate::{TransportError, TransportResult};
use dp_types::structs::{serialize_dpmsg_setplayerorgroupdata, serialize_dpmsg_setplayerorgroupname, serialize_dpmsg_setsessiondesc};
use dp_types::{PlayerName, GUID};

/// DPPLAYERTYPE_PLAYER constant (player, not group)
const DPPLAYERTYPE_PLAYER: u32 = 1;
use iroh::endpoint::{
    ConnectError, ConnectingError, Connection, ConnectionError, RecvStream, SendStream,
    TransportErrorCode,
};
use iroh::{Endpoint, EndpointAddr, EndpointId};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

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

/// How long a lost host of a game in progress may stay away before it counts
/// as gone. A brief blip is re-dialled well inside this. It is shorter than
/// the five re-dials' whole run (about 80 s when each dial times out) because
/// SMAC's own "Player ... not responding" popup appears after about 30 s
/// (game-session-sync ticket 02's evidence), and the game should have its new
/// host before then. The Stop warning (README.md, packaging/README.txt, page.html)
/// says "about 10 seconds": keep it in step with this.
pub const DEFAULT_HOST_GONE_AFTER: Duration = Duration::from_secs(10);

/// Magic prefix opening every ordered message stream: b"SMAC" + version (u16 LE).
pub(crate) const STREAM_MAGIC: &[u8; 4] = b"SMAC";

/// Upper bound on a single framed message (defensive against corrupt frames).
const MAX_FRAME_LEN: u32 = 1 << 20; // 1 MiB

/// The friend registry as the connection manager holds it: each link's
/// handle is its iroh connection.
pub type Friends = FriendRegistry<Connection>;

/// Connection manager for mesh networking: the iroh work behind the friend
/// registry. It dials and accepts connections, builds a link for each, runs
/// the readers and the ordered writers, and tells the registry about links
/// opening and ending.
pub struct ConnectionManager {
    /// Iroh endpoint
    endpoint: Endpoint,
    /// Every friend, and everything sent to or received from them
    friends: Arc<Friends>,
    /// Session manager
    session_manager: Arc<SessionManager>,
    /// The game inbox
    inbox: GameInbox,
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
            friends: self.friends.clone(),
            session_manager: self.session_manager.clone(),
            inbox: self.inbox.clone(),
            endpoint_id: self.endpoint_id,
            our_ticket: self.our_ticket.clone(),
            network_mode: self.network_mode,
            options: self.options,
        }
    }
}

impl ConnectionManager {
    /// Create a new connection manager. The endpoint must advertise
    /// `peer_protocol_alpn(options.peer_protocol_version)`. `friends` is the
    /// Transport's friend registry, delivering into `inbox`.
    pub fn new(
        endpoint: Endpoint,
        friends: Arc<Friends>,
        session_manager: Arc<SessionManager>,
        inbox: GameInbox,
        our_ticket: String,
        network_mode: NetworkMode,
        options: TransportOptions,
    ) -> Self {
        let endpoint_id = endpoint.id();
        Self {
            endpoint,
            friends,
            session_manager,
            inbox,
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

    /// Build the link for a new connection: spawn its ordered writer and give
    /// it a friend pipeline that reads the connection's RTT.
    /// Must be called from within the tokio runtime context.
    fn build_link(&self, connection: &Connection, endpoint_id: EndpointId) -> Link<Connection> {
        // All sends to this connection are queued on the outbox (synchronously,
        // so enqueue order == caller order) and written to ONE long-lived
        // framed QUIC stream by the writer task. Per-friend FIFO delivery
        // depends on this single-queue/single-stream design: separate streams
        // or tasks per message would leave ordering to the scheduler.
        let (outbox, outbox_rx) = mpsc::unbounded_channel();
        tokio::spawn(crate::frame_sink::run_ordered_writer(
            crate::frame_sink::IrohFrameSink::new(connection.clone(), self.options.peer_protocol_version),
            outbox_rx,
            endpoint_short(endpoint_id.as_bytes()),
        ));
        let pipeline = FriendPipeline::new(*endpoint_id.as_bytes(), std::time::Instant::now, {
            let connection = connection.clone();
            move || connection.paths().iter().find(|p| p.is_selected()).map(|p| p.stats().rtt)
        });
        let pipeline = pipeline.with_link_stats({
            let connection = connection.clone();
            move || {
                let paths = connection.paths();
                let s = paths.iter().find(|p| p.is_selected())?.stats();
                Some(crate::friend_pipeline::LinkStats {
                    datagrams_sent: s.udp_tx.datagrams,
                    lost_packets: s.lost_packets,
                    congestion_events: s.congestion_events,
                    cwnd: s.cwnd,
                })
            }
        });
        Link::new(outbox, pipeline.with_early_acks(self.options.early_acks), connection.clone())
    }

    /// Connect to a peer by ticket, unless it is already connected.
    pub async fn connect_by_ticket(&self, ticket: &str) -> TransportResult<EndpointId> {
        let endpoint_addr = Self::parse_ticket(ticket)?;

        // Check if already connected
        if self.friends.is_connected(*endpoint_addr.id) {
            return Ok(endpoint_addr.id);
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

        let link = self.build_link(&connection, endpoint_addr.id);
        let link_id = self.friends.link_opened(*endpoint_addr.id, Some(ticket.to_string()), link, &self.local_facts());

        // Outgoing connections need a receiver loop just like incoming ones:
        // without it we could send to the peer but never receive from them.
        self.spawn_connection_handler(connection, endpoint_addr.id, link_id);

        Ok(endpoint_addr.id)
    }

    /// Connect to a peer by EndpointId (if we already have a connection)
    pub async fn connect(&self, endpoint_id: EndpointId) -> TransportResult<()> {
        if self.friends.is_connected(*endpoint_id) {
            return Ok(());
        }
        Err(TransportError::NotConnected)
    }

    /// Disconnect from a peer: close every connection to it.
    pub fn disconnect(&self, endpoint_id: &EndpointId) {
        for connection in self.friends.disconnect(**endpoint_id) {
            connection.close(0u32.into(), b"disconnect");
        }
    }

    /// Disconnect all peers
    pub fn disconnect_all(&self) {
        for connection in self.friends.disconnect_all() {
            connection.close(0u32.into(), b"session closed");
        }
    }

    /// Get all connected peer endpoint IDs
    pub fn get_connected_peers(&self) -> Vec<EndpointId> {
        self.friends.connected_friends().into_iter().map(|(_, c)| c.remote_id()).collect()
    }

    /// The class of each connected peer's selected path; a peer without a
    /// selected path yet is left out.
    pub fn peer_paths(&self) -> Vec<(EndpointId, crate::path::PathClass)> {
        self.friends
            .connected_friends()
            .into_iter()
            .filter_map(|(_, c)| Some((c.remote_id(), crate::path::selected_path(&c)?)))
            .collect()
    }

    /// Send and wait for response (bidirectional)
    pub async fn send_and_receive(&self, endpoint_id: &EndpointId, msg: &Message) -> TransportResult<Message> {
        let connection = self.friends.link_handle(**endpoint_id).ok_or(TransportError::NotConnected)?;

        let data = encode_message(msg)?;

        let (mut send, mut recv) = connection.open_bi().await?;
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
    fn spawn_connection_handler(&self, connection: Connection, endpoint_id: EndpointId, link_id: LinkId) {
        let endpoint_id_bytes: [u8; 32] = *endpoint_id;
        let session_manager = self.session_manager.clone();
        let inbox = self.inbox.clone();
        let cm = self.clone();

        crate::capture::spawn_path_watcher(connection.clone(), endpoint_id_bytes);
        crate::path::spawn_path_logger(connection.clone(), endpoint_short(endpoint_id.as_bytes()));

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
                                let inbox = inbox.clone();
                                let cm = cm.clone();

                                tokio::spawn(async move {
                                    if let Err(e) = read_ordered_stream(
                                        recv,
                                        &session_manager,
                                        &inbox,
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

            // Connection closed. The registry decides what the loss means,
            // from the roster's facts, at once: the game polling later must
            // not delay a re-dial.
            let roster = RosterFacts {
                was_host: cm.session_manager.is_node_host(&endpoint_id_bytes),
                in_progress: cm.session_manager.is_in_progress(),
                ticket: cm.session_manager.get_player_ticket_by_node(&endpoint_id_bytes),
            };
            let was_host = roster.was_host;
            match cm.friends.link_ended(endpoint_id_bytes, link_id, roster) {
                LinkEnded::Nothing => {}
                LinkEnded::Redial { ticket } => cm.spawn_redial(endpoint_id_bytes, ticket, was_host),
                LinkEnded::HostGone => {
                    apply_host_closed(&cm.session_manager, &cm.friends, &cm.inbox, &endpoint_id_bytes)
                }
            }
        });
    }

    /// Up to five dial attempts at growing intervals; whether one succeeded.
    /// Cancelling this future (the host-gone bound) is safe: a dial that
    /// completed has already opened its link, so `redial_gave_up` finds it.
    async fn redial_attempts(&self, endpoint_id_bytes: [u8; 32], ticket: &str) -> bool {
        for attempt in 1..=5u32 {
            tokio::time::sleep(std::time::Duration::from_millis(500 * attempt as u64)).await;
            match self.connect_by_ticket(ticket).await {
                Ok(_) => {
                    info!(attempt, "reconnected to peer after transport loss");
                    crate::capture::reconnect(&endpoint_id_bytes, attempt, true);
                    return true;
                }
                Err(e) => {
                    debug!(attempt, error = %e, "reconnect attempt failed");
                    crate::capture::reconnect(&endpoint_id_bytes, attempt, false);
                }
            }
        }
        false
    }

    /// Re-dial a lost friend by its ticket (bounded best-effort): five
    /// attempts, backing off. Nothing else re-establishes a friend's
    /// connection after a transient transport loss, and under full mesh a
    /// dropped edge silently stops that pair from exchanging
    /// broadcasts/ACKs. connect_by_ticket is idempotent, so a race with a
    /// heal from the other side is harmless. When a dial succeeds, the
    /// registry re-attaches the new link to the friend's kept record.
    ///
    /// The host of a game in progress is re-dialled only for
    /// `options.host_gone_after`: if it hasn't come back by then (or the
    /// attempts run out first) it is gone, and the host role migrates.
    fn spawn_redial(&self, endpoint_id_bytes: [u8; 32], ticket: String, is_host: bool) {
        let cm = self.clone();
        tokio::spawn(async move {
            let reconnected = if is_host {
                tokio::time::timeout(cm.options.host_gone_after, cm.redial_attempts(endpoint_id_bytes, &ticket))
                    .await
                    .unwrap_or(false)
            } else {
                cm.redial_attempts(endpoint_id_bytes, &ticket).await
            };
            if reconnected {
                return;
            }
            let forgotten = cm.friends.redial_gave_up(endpoint_id_bytes);
            if forgotten && is_host {
                info!("host did not come back, treating it as gone");
                apply_host_closed(&cm.session_manager, &cm.friends, &cm.inbox, &endpoint_id_bytes);
            }
        });
    }

    /// Who we are, for the Hello a re-attached link carries first.
    fn local_facts(&self) -> LocalFacts {
        LocalFacts {
            player: self.session_manager.local_player_id(),
            ticket: self.our_ticket.clone(),
        }
    }

    /// Establish mesh connection to a new player
    pub async fn establish_mesh_connection(&self, player_info: &PlayerInfo) -> TransportResult<()> {
        // Skip if it's us
        if player_info.node_id == self.endpoint_id_bytes() {
            return Ok(());
        }

        // Skip if already connected
        if self.friends.is_connected(player_info.node_id) {
            // Just update routing
            self.friends.player_joined(player_info.player_id, player_info.node_id);
            return Ok(());
        }

        // Skip if no ticket
        if player_info.ticket.is_empty() {
            warn!("No ticket for player {}, can't establish mesh", player_info.player_id);
            return Ok(());
        }

        info!("Establishing mesh connection to player {}", player_info.player_id);

        // Route first: the link then opens onto the friend's record, and the
        // registry queues our Hello on it before anything else.
        self.friends.player_joined(player_info.player_id, player_info.node_id);

        // Connect using their ticket
        match self.connect_by_ticket(&player_info.ticket).await {
            Ok(_) => {
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
        info!("Incoming connection from: {}", endpoint_id);

        // The ticket comes with the friend's Hello. If the friend already has
        // a link, the registry keeps this one as a spare.
        let link = self.build_link(&connection, endpoint_id);
        let link_id = self.friends.link_opened(*endpoint_id, None, link, &self.local_facts());

        // Use shared helper to spawn the connection handler
        self.spawn_connection_handler(connection, endpoint_id, link_id);

        Ok(())
    }
}

/// Read a peer's ordered message stream: validate the preamble, then decode and
/// dispatch length-prefixed frames one at a time, in order. Returns when the
/// stream ends (peer closed it) or on a framing/protocol error.
async fn read_ordered_stream(
    mut recv: RecvStream,
    session_manager: &SessionManager,
    inbox: &GameInbox,
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
            inbox,
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
    inbox: &GameInbox,
    sender_endpoint_id: [u8; 32],
    cm: &ConnectionManager,
) {
    if !matches!(msg, Message::GameMessage { .. }) {
        crate::capture::inbound_control(&sender_endpoint_id, &msg);
    }
    match msg {
        Message::Hello { player_id, ticket } => {
            debug!("Received Hello from player {}", player_id);
            // Update routing
            cm.friends.route_player_and_record_ticket(sender_endpoint_id, player_id, ticket);
        }

        Message::SessionAnnounce { session } => {
            debug!("Received session announcement: {}", session.session_name);
            session_manager.add_discovered_session(session.clone());
        }

        Message::SessionList { sessions } => {
            debug!("Received session list with {} sessions", sessions.len());
            for session in &sessions {
                session_manager.add_discovered_session(session.clone());
            }
        }

        Message::PlayerJoined { player } => {
            debug!("Player joined: {} ({})", player.player_id, player.name.short_name);

            // Add routing for this player
            cm.friends.player_joined(player.player_id, player.node_id);

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

            inbox.add_system(SystemEvent::PlayerJoined {
                current_players: session_manager.get_players().len() as u32,
                info: player,
            });
        }

        Message::PlayerLeft { player_id, .. } => {
            // Only the player's own Helper or the host may say it left. Anything
            // else is about a session we're no longer in, and the DPID may since
            // belong to someone else.
            let belongs = session_manager.get_player(player_id).is_some_and(|p| {
                p.node_id == Some(sender_endpoint_id)
                    || session_manager.is_node_host(&sender_endpoint_id)
            });
            if !belongs {
                debug!(player_id, "Ignoring PlayerLeft for a player not in our session");
                return;
            }
            debug!("Player left: {}", player_id);
            cm.friends.player_left(player_id);
            let (name, flags) = session_manager
                .get_player(player_id)
                .map(|p| (p.name.clone(), p.flags))
                .unwrap_or_else(|| (PlayerName::default(), 0));
            session_manager.remove_player(player_id);
            inbox.add_system(SystemEvent::PlayerLeft { player_id, name, flags });
        }

        Message::GameMessage {
            from,
            to,
            flags,
            data,
        } => {
            let guaranteed = flags & dp_types::DPSEND_GUARANTEED != 0;
            debug!(from, to, size = data.len(), guaranteed, "received GameMessage, queuing to channel");

            // Deliver to the local DLL/game, through the friend's pipeline.
            //
            // TOPOLOGY: full mesh. Every player holds a direct connection to every
            // other player, so the origin already delivered this GameMessage to us
            // directly — a broadcast reached every peer once, a directed message
            // reached only its target. We therefore do NOT relay and do NOT echo:
            //   * relaying would double-deliver to peers the origin already reached;
            //   * echoing to the sender would duplicate the copy JACKAL already gives
            //     the sender via its internal loopback queue (game @0x64a210).
            cm.friends.receive_game(sender_endpoint_id, from, to, guaranteed, data);
        }

        Message::SessionClosed { .. } => {
            // Only the host's closing ends the session. A Helper from `-rc.5` or
            // earlier sends this when its Joiner merely leaves.
            apply_host_closed(session_manager, &cm.friends, inbox, &sender_endpoint_id);
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
            inbox.add_system(SystemEvent::Message(sys_msg));
        }

        Message::Ping { timestamp } => {
            debug!("Received ping: {}", timestamp);
        }

        Message::PlayerNameUpdate { player_id, name } => {
            // A joiner's first name is its CreatePlayer: only now does the host's
            // game hear of it, named, as real DirectPlay would tell it.
            if let Some(joiner) = session_manager.rename_remote_player(player_id, name.clone()) {
                info!(player_id, long = %name.long_name, "Joiner created its player, announcing it to the game");
                inbox.add_system(SystemEvent::PlayerJoined {
                    current_players: session_manager.get_players().len() as u32,
                    info: joiner,
                });
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
            inbox.add_system(SystemEvent::Message(sys_msg));
        }

        Message::SessionDescUpdate { session: desc } => {
            debug!(session_name = %desc.session_name, "Received session desc update, generating DPMSG_SETSESSIONDESC");

            // Generate DPMSG_SETSESSIONDESC for the local game
            let sys_msg = serialize_dpmsg_setsessiondesc(&desc);
            inbox.add_system(SystemEvent::Message(sys_msg));

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
                if desc.is_in_progress() {
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
                        cm.friends.player_joined(existing_id, sender_endpoint_id);
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
                        cm.friends.player_joined(player_id, sender_endpoint_id);
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
        if let Err(e) = cm.friends.send_control_to_all_but(sender_endpoint_id, &broadcast_msg) {
            warn!("Failed to broadcast PlayerJoined: {}", e);
        }
    }

    Ok(())
}

/// The host is done with the Game session, by saying so (`SessionClosed`) or
/// by being gone for good: the game is told the session is lost, or, for a
/// game in progress, its host's players left and the host role migrates.
fn apply_host_closed(session_manager: &SessionManager, friends: &Friends, inbox: &GameInbox, sender: &[u8; 32]) {
    match session_manager.host_closed(sender) {
        HostClosed::NotFromHost => {
            debug!("Ignoring the end of a peer that isn't our session's host");
        }
        HostClosed::SessionLost => {
            inbox.add_system(SystemEvent::SessionLost);
        }
        HostClosed::Migrated { departed, we_are_host } => {
            // A game in progress asked for host migration: its host's
            // players left, and JACKAL picks the new host itself.
            info!(we_are_host, "host left a game in progress, host role migrated");
            for p in departed {
                friends.player_left(p.id);
                inbox.add_system(SystemEvent::PlayerLeft {
                    player_id: p.id,
                    name: p.name,
                    flags: p.flags,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    // Connection tests require actual Iroh infrastructure
    // These would be integration tests
}
