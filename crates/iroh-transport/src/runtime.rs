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
    peer_protocol_alpn, ConnectionManager, ReceivedMessage, DEFAULT_DIAL_TIMEOUT,
    STREAM_PROTO_VERSION,
};
use crate::protocol::{Message, PlayerInfo, SessionInfo};
use crate::session::SessionManager;
use crate::{TransportError, TransportResult};
use dp_types::{dpid, PlayerName, SessionDesc, DPID, GUID};
use iroh::{Endpoint, EndpointId, SecretKey};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::{Builder, Runtime};
use tokio::sync::mpsc;
use tracing::{debug, debug_span, error, info, warn};

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
    /// Received message queue (for DirectPlay Receive calls)
    message_queue: Arc<Mutex<VecDeque<QueuedMessage>>>,
    /// Message receiver
    message_rx: Arc<Mutex<mpsc::UnboundedReceiver<ReceivedMessage>>>,
    /// Our ticket string (for sharing)
    our_ticket: String,
    /// Track player IDs for which CREATEPLAYERORGROUP has been queued (to prevent duplicates)
    createplayerorgroup_sent: Arc<Mutex<std::collections::HashSet<DPID>>>,
}

/// A queued message for DirectPlay
#[derive(Debug, Clone)]
pub struct QueuedMessage {
    pub from: DPID,
    pub to: DPID,
    pub data: Vec<u8>,
    pub guaranteed: bool,
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
        let runtime = Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| TransportError::Io(e))?;

        let (endpoint, connection_manager, session_manager, message_rx, our_ticket) =
            runtime.block_on(async { Self::init_async(options).await })?;

        Ok(Self {
            runtime,
            endpoint: Arc::new(endpoint),
            connection_manager: Arc::new(connection_manager),
            session_manager,
            message_queue: Arc::new(Mutex::new(VecDeque::new())),
            message_rx: Arc::new(Mutex::new(message_rx)),
            our_ticket,
            createplayerorgroup_sent: Arc::new(Mutex::new(std::collections::HashSet::new())),
        })
    }

    async fn init_async(options: TransportOptions) -> TransportResult<(
        Endpoint,
        ConnectionManager,
        Arc<SessionManager>,
        mpsc::UnboundedReceiver<ReceivedMessage>,
        String,
    )> {
        // Generate a new secret key for this instance
        let secret_key = SecretKey::generate();

        info!("Initializing Iroh endpoint...");

        // The N0 preset provides relay + address-lookup defaults (n0 dns/pkarr);
        // the builder pre-binds IPv4 0.0.0.0 with an OS-assigned port.
        let endpoint = match Endpoint::builder(iroh::endpoint::presets::N0)
            .secret_key(secret_key)
            .alpns(vec![peer_protocol_alpn(options.peer_protocol_version)])
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

        let (message_tx, message_rx) = mpsc::unbounded_channel();

        let connection_manager =
            ConnectionManager::with_options(endpoint.clone(), session_manager.clone(), message_tx, our_ticket.clone(), NetworkMode::default(), options);

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

        Ok((endpoint, connection_manager, session_manager, message_rx, our_ticket))
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
            info!("Created session as host");
        }
        result
    }

    /// Join a session using the host ticket
    pub fn join_session(&self, session_info: &SessionInfo, _password: Option<&str>) -> TransportResult<DPID> {
        let host_ticket = &session_info.host_ticket;
        if host_ticket.is_empty() {
            return Err(TransportError::InvalidMessage);
        }

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
                        self.message_queue.lock().push_back(QueuedMessage {
                            from: 0, // DPID_SYSMSG
                            to: 0,
                            data: sys_msg,
                            guaranteed: true,
                        });
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

                        // Check dedup set before queuing CREATEPLAYERORGROUP
                        let mut sent = self.createplayerorgroup_sent.lock();
                        if sent.contains(&player.player_id) {
                            debug!("Skipping duplicate CREATEPLAYERORGROUP for player {} in join_session", player.player_id);
                            continue;
                        }
                        sent.insert(player.player_id);
                        drop(sent);

                        // Queue system message so game discovers this existing player
                        let current_players = self.session_manager.get_players().len() as u32;
                        let sys_msg = create_player_joined_msg(player, current_players);
                        self.message_queue.lock().push_back(QueuedMessage {
                            from: 0, // DPID_SYSMSG
                            to: 0,
                            data: sys_msg,
                            guaranteed: true,
                        });
                    }

                    // Queue CREATEPLAYERORGROUP for ourselves too. In real
                    // DirectPlay the joiner receives SUPERENUMPLAYERSREPLY, which
                    // includes their own player, and DirectPlay then generates
                    // CREATEPLAYERORGROUP for ALL players including self; the game
                    // does not consider itself joined until it sees this message.
                    // See DirectPlay spec MC-DPL4CS section 3.1.5.7.

                    // Check dedup set before queuing CREATEPLAYERORGROUP for self
                    let mut sent = self.createplayerorgroup_sent.lock();
                    if !sent.contains(&id) {
                        sent.insert(id);
                        drop(sent);

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
                        let my_msg = create_player_joined_msg(my_info, current_players);
                        self.message_queue.lock().push_back(QueuedMessage {
                            from: 0, // DPID_SYSMSG
                            to: 0,
                            data: my_msg,
                            guaranteed: true,
                        });
                        info!("Queued CREATEPLAYERORGROUP for self (player_id={})", id);
                    } else {
                        debug!("Skipping duplicate CREATEPLAYERORGROUP for self (player_id={})", id);
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
    pub fn close_session(&self) {
        // Notify peers
        let msg = Message::SessionClosed {
            reason: "Host closed session".to_string(),
        };

        if let Err(e) = self.connection_manager.broadcast(&msg) {
            warn!("SessionClosed broadcast failed: {}", e);
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

        // Add to dedup set (host's own player)
        self.createplayerorgroup_sent.lock().insert(player_id);

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
        let sys_msg = create_player_joined_msg(player_info.clone(), current_players);
        self.message_queue.lock().push_back(QueuedMessage {
            from: 0, // DPID_SYSMSG
            to: 0,   // DPID_ALLPLAYERS
            data: sys_msg,
            guaranteed: true,
        });
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
            let sys_msg = create_player_left_msg(player_id, &player.name, player.flags);
            self.message_queue.lock().push_back(QueuedMessage {
                from: dpid::DPID_SYSMSG,
                to: dpid::DPID_ALLPLAYERS,
                data: sys_msg,
                guaranteed: true,
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
                    self.message_queue.lock().push_back(QueuedMessage {
                        from,
                        to,
                        data,
                        guaranteed,
                    });
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
            Err(e) => debug!(error = %e, "send failed"),
        }

        result
    }

    /// Receive a message (non-blocking)
    pub fn receive(&self) -> Option<QueuedMessage> {
        let span = debug_span!("transport_receive");
        let _guard = span.enter();

        // Drain channel into queue first
        self.drain_channel_to_queue();

        // Now pop from the queue
        let mut queue = self.message_queue.lock();
        if let Some(msg) = queue.pop_front() {
            debug!(from = msg.from, to = msg.to, size = msg.data.len(), "returning message from queue");
            return Some(msg);
        }

        debug!("no messages available");
        None
    }

    /// Get message count
    ///
    /// Drains pending messages from the channel into the queue before counting,
    /// so the count is accurate. This is critical for DirectPlay's GetMessageCount.
    pub fn message_count(&self) -> usize {
        // Drain any pending messages from channel into queue
        self.drain_channel_to_queue();
        self.message_queue.lock().len()
    }

    /// Drain pending messages from the channel into the message queue
    fn drain_channel_to_queue(&self) {
        let mut rx = self.message_rx.lock();
        let mut queue = self.message_queue.lock();

        while let Ok(received) = rx.try_recv() {
            if let Some(queued) = self.convert_received_to_queued(received) {
                queue.push_back(queued);
            }
        }
    }

    /// Convert a ReceivedMessage to a QueuedMessage, if applicable
    fn convert_received_to_queued(&self, received: ReceivedMessage) -> Option<QueuedMessage> {
        match received {
            ReceivedMessage::GameMessage {
                from,
                to,
                data,
                guaranteed,
            } => Some(QueuedMessage {
                from,
                to,
                data,
                guaranteed,
            }),
            ReceivedMessage::PlayerJoined(info) => {
                // Check if we've already sent CREATEPLAYERORGROUP for this player (dedup)
                let mut sent = self.createplayerorgroup_sent.lock();
                if sent.contains(&info.player_id) {
                    tracing::debug!(
                        "Skipping duplicate CREATEPLAYERORGROUP for player {}",
                        info.player_id
                    );
                    return None;
                }
                sent.insert(info.player_id);
                drop(sent);

                // Get current player count from session manager
                let current_players = self.session_manager.get_players().len() as u32;
                let sys_msg = create_player_joined_msg(info, current_players);
                Some(QueuedMessage {
                    from: dpid::DPID_SYSMSG,
                    to: dpid::DPID_ALLPLAYERS,
                    data: sys_msg,
                    guaranteed: true,
                })
            }
            ReceivedMessage::PlayerLeft(player_id) => {
                // Try to get player info before they're fully removed
                // If not found, use placeholder values
                let (name, flags) = self
                    .session_manager
                    .get_player(player_id)
                    .map(|p| (p.name.clone(), p.flags))
                    .unwrap_or_else(|| (PlayerName::default(), 0));
                let sys_msg = create_player_left_msg(player_id, &name, flags);
                Some(QueuedMessage {
                    from: dpid::DPID_SYSMSG,
                    to: dpid::DPID_ALLPLAYERS,
                    data: sys_msg,
                    guaranteed: true,
                })
            }
            ReceivedMessage::SessionClosed => {
                let sys_msg = create_session_lost_msg();
                Some(QueuedMessage {
                    from: dpid::DPID_SYSMSG,
                    to: dpid::DPID_ALLPLAYERS,
                    data: sys_msg,
                    guaranteed: true,
                })
            }
            ReceivedMessage::ConnectionLost(endpoint_id) => {
                let endpoint_id_bytes: [u8; 32] = *endpoint_id;

                // Check if the disconnected peer was the host
                if self.session_manager.is_node_host(&endpoint_id_bytes) {
                    // Host disconnected - session is lost
                    let sys_msg = create_session_lost_msg();
                    Some(QueuedMessage {
                        from: dpid::DPID_SYSMSG,
                        to: dpid::DPID_ALLPLAYERS,
                        data: sys_msg,
                        guaranteed: true,
                    })
                } else {
                    // Non-host peer lost. Heal the full mesh by re-dialing the peer
                    // via its ticket (bounded best-effort). This is the reconnect
                    // driver the design requires: nothing else re-establishes a peer
                    // connection after a transient transport loss, and under full
                    // mesh a dropped edge silently stops that pair from exchanging
                    // broadcasts/ACKs. connect_by_ticket is idempotent, so a race
                    // with a heal from the other side is harmless. If the peer is
                    // genuinely gone the retries simply fail out; session state is
                    // untouched here.
                    if let Some(ticket) =
                        self.session_manager.get_player_ticket_by_node(&endpoint_id_bytes)
                    {
                        let cm = self.connection_manager.clone();
                        self.runtime.spawn(async move {
                            for attempt in 1..=5u32 {
                                tokio::time::sleep(std::time::Duration::from_millis(
                                    500 * attempt as u64,
                                ))
                                .await;
                                match cm.connect_by_ticket(&ticket).await {
                                    Ok(_) => {
                                        info!(attempt, "reconnected to peer after transport loss");
                                        break;
                                    }
                                    Err(e) => {
                                        debug!(attempt, error = %e, "reconnect attempt failed");
                                    }
                                }
                            }
                        });
                    }
                    None
                }
            }
            ReceivedMessage::SystemMessage(data) => {
                // Pre-serialized DirectPlay system message - deliver directly to game
                Some(QueuedMessage {
                    from: dpid::DPID_SYSMSG,
                    to: dpid::DPID_ALLPLAYERS,
                    data,
                    guaranteed: true,
                })
            }
            // Internal messages don't generate game-visible messages
            _ => None,
        }
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

// Helper functions to create system messages

fn create_player_joined_msg(info: PlayerInfo, current_players: u32) -> Vec<u8> {
    // Use the full DPMSG_CREATEPLAYERORGROUP serialization
    dp_types::serialize_dpmsg_createplayerorgroup(
        info.player_id,
        dp_types::DPPLAYERTYPE_PLAYER,
        current_players,
        &info.name,
        &info.data,
        info.flags,
    )
}

fn create_player_left_msg(player_id: DPID, name: &PlayerName, flags: u32) -> Vec<u8> {
    // Use the full DPMSG_DESTROYPLAYERORGROUP serialization
    dp_types::serialize_dpmsg_destroyplayerorgroup(
        player_id,
        dp_types::DPPLAYERTYPE_PLAYER,
        name,
        flags,
    )
}

fn create_session_lost_msg() -> Vec<u8> {
    dp_types::serialize_dpmsg_sessionlost()
}

#[cfg(test)]
mod tests {
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
