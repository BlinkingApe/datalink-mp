//! IPC server: answers the DLL's requests over TCP localhost.
//!
//! The listener is bound once at startup and served on a plain thread until
//! the Helper shuts down.

use crate::controller::SessionController;
use anyhow::Result;
use ipc_protocol::{
    decode_request, encode_response, read_message, IpcError, IpcRequest, IpcResponse,
    PlayerListEntry, QueuedMessage, SessionListEntry, PROTOCOL_VERSION,
};
use std::io::Write;
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tracing::{debug, debug_span, error, info, warn};

/// How long waking the server's thread to stop it may take.
const WAKE_TIMEOUT: Duration = Duration::from_secs(1);

/// Bind the listener for DLL connections. Port 0 picks a free port.
pub(crate) fn bind(port: u16) -> std::io::Result<TcpListener> {
    TcpListener::bind(("127.0.0.1", port))
}

/// A running IPC server. It runs until [`stop`](Self::stop)ped; dropping this
/// leaves it running.
pub(crate) struct IpcServer {
    /// Where the server listens.
    addr: SocketAddr,
    stopper: Arc<Stopper>,
    thread: JoinHandle<()>,
}

/// How the server is told to stop while its thread is blocked on a socket.
#[derive(Default)]
struct Stopper {
    serving: Mutex<Serving>,
}

/// What the server's thread and whoever stops it agree on.
#[derive(Default)]
struct Serving {
    /// Set once: the server serves no further connection.
    stop_asked: bool,
    /// The DLL connection being served, kept to end the read its thread is
    /// blocked in.
    client: Option<TcpStream>,
}

impl Stopper {
    fn serving(&self) -> MutexGuard<'_, Serving> {
        self.serving.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The server's thread is about to serve `client`. False when the server
    /// was asked to stop instead, and must not.
    fn begin_serving(&self, client: &TcpStream) -> std::io::Result<bool> {
        let mut serving = self.serving();
        if serving.stop_asked {
            return Ok(false);
        }
        serving.client = Some(client.try_clone()?);
        Ok(true)
    }

    fn done_serving(&self) {
        self.serving().client = None;
    }

    fn stop_asked(&self) -> bool {
        self.serving().stop_asked
    }

    /// Serve no further connection, and end the one being served.
    fn ask_to_stop(&self) {
        let mut serving = self.serving();
        serving.stop_asked = true;
        if let Some(client) = serving.client.take() {
            // The game's link ends here, as it would when the process exits.
            let _ = client.shutdown(Shutdown::Both);
        }
    }
}

/// Serve DLL connections on `listener`, which is bound to `port`, on their
/// own thread.
pub(crate) fn spawn(
    listener: TcpListener,
    port: u16,
    controller: Arc<SessionController>,
) -> IpcServer {
    let stopper = Arc::new(Stopper::default());
    let thread = {
        let stopper = stopper.clone();
        std::thread::spawn(move || {
            let _ends_the_helper = ShutDownOnPanic(&controller);
            serve(listener, &controller, &stopper)
        })
    };
    IpcServer {
        addr: SocketAddr::from(([127, 0, 0, 1], port)),
        stopper,
        thread,
    }
}

/// A Helper whose IPC server died is of no use to the game: shut it down, so
/// that whoever waits on the Helper finds the panic instead of waiting on.
struct ShutDownOnPanic<'a>(&'a SessionController);

impl Drop for ShutDownOnPanic<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.0.shutdown();
        }
    }
}

impl IpcServer {
    /// Stop serving, end the DLL connection being served and release the port
    /// (blocking). An error is the panic the server's thread died of.
    ///
    /// The thread finishes the request it is answering first. Shut the
    /// Transport down before this, so that a request waiting on it returns.
    pub(crate) fn stop(self) -> std::thread::Result<()> {
        self.stopper.ask_to_stop();
        // The thread is most likely blocked accepting; a connection is what
        // wakes it, and it then sees that it was asked to stop.
        let woken = TcpStream::connect_timeout(&self.addr, WAKE_TIMEOUT).is_ok();
        if !woken && !self.ends_within(WAKE_TIMEOUT) {
            // Waiting for a thread that nothing wakes would never end. It
            // keeps the port.
            warn!("Could not wake the IPC server to stop it; leaving it behind");
            return Ok(());
        }
        self.thread.join()
    }

    /// Whether the server's thread ends by itself within `timeout`. Nothing
    /// listens on its port any more when it is ending on a panic, or has ended.
    fn ends_within(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while !self.thread.is_finished() {
            if Instant::now() > deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        true
    }
}

fn serve(listener: TcpListener, controller: &SessionController, stopper: &Stopper) {
    // Accept connections (single-threaded for simplicity - one DLL at a time)
    for stream in listener.incoming() {
        let stream = match stream {
            Ok(stream) => stream,
            Err(e) => {
                error!("Accept error: {:?}", e);
                if stopper.stop_asked() {
                    break;
                }
                continue;
            }
        };
        match stopper.begin_serving(&stream) {
            Ok(true) => {}
            // Asked to stop: this is the connection that woke the accept.
            Ok(false) => break,
            Err(e) => {
                // A connection that could not be ended later is not served.
                error!("Client error: {:?}", e);
                continue;
            }
        }
        info!("Client connected from {:?}", stream.peer_addr());
        if let Err(e) = handle_client(stream, controller) {
            error!("Client error: {:?}", e);
        }
        stopper.done_serving();
        // The connection is over, cleanly or not.
        controller.dll_disconnected();
        info!("Client disconnected");
    }
}

fn handle_client(mut stream: TcpStream, controller: &SessionController) -> Result<()> {
    // Disable Nagle's algorithm for lower latency
    stream.set_nodelay(true)?;

    // A DLL from another build shows in its first message: either the
    // handshake says so, or the message cannot be read at all.
    let mut first_message = true;

    loop {
        let request = match read_request(&mut stream) {
            Ok(Some(request)) => request,
            // Client closed connection
            Ok(None) => return Ok(()),
            Err(e) => {
                if first_message && is_unreadable(&e) {
                    controller.dll_mismatched();
                }
                return Err(e.into());
            }
        };
        first_message = false;
        let request_type = request.type_name();

        let span = debug_span!("ipc_request", request_type);
        let _guard = span.enter();

        let response = handle_request(&request, controller);

        let encoded = encode_response(&response)?;
        // encode_response already includes length prefix, so we write it directly
        stream.write_all(&encoded)?;
        stream.flush()?;
    }
}

/// Read the next request. None when the DLL closed the connection.
fn read_request(stream: &mut TcpStream) -> Result<Option<IpcRequest>, IpcError> {
    let data = match read_message(stream) {
        Ok(data) => data,
        Err(IpcError::Io(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    };
    Ok(Some(decode_request(&data)?))
}

/// Whether `error` says that what arrived was not a message this Helper can
/// read, as opposed to the connection failing.
fn is_unreadable(error: &IpcError) -> bool {
    match error {
        IpcError::Codec(_) => true,
        // A length prefix beyond any real message
        IpcError::Io(e) => e.kind() == std::io::ErrorKind::InvalidData,
        _ => false,
    }
}

fn handle_request(request: &IpcRequest, controller: &SessionController) -> IpcResponse {
    // Fetch the Transport for every request, not once per connection: a
    // DLL connection outlives a Transport when Stop replaces it.
    let transport = controller.transport();
    match request {
        IpcRequest::Handshake { protocol_version } => {
            if *protocol_version != PROTOCOL_VERSION {
                controller.dll_mismatched();
                return IpcResponse::Error {
                    message: format!(
                        "Protocol version mismatch: expected {}, got {}",
                        PROTOCOL_VERSION, protocol_version
                    ),
                };
            }
            controller.dll_handshake_succeeded();
            IpcResponse::HandshakeOk {
                endpoint_id: transport.endpoint_id_bytes(),
                our_ticket: transport.our_ticket().to_string(),
            }
        }

        IpcRequest::CreateSession { desc } => match transport.create_session(desc.clone()) {
            Some(guid) => IpcResponse::SessionCreated { guid },
            None => IpcResponse::Error {
                message: "Failed to create session (already in session?)".to_string(),
            },
        },

        IpcRequest::JoinSessionByTicket { host_ticket } => {
            // Through the controller, so that the page hears how it went.
            match controller.join_session(host_ticket) {
                Ok(player_id) => IpcResponse::SessionJoined { player_id },
                Err(e) => IpcResponse::Error {
                    message: format!("Failed to join session: {:?}", e),
                },
            }
        }

        IpcRequest::CloseSession => {
            // close_session doesn't return an error, but we log that it happened
            transport.close_session();
            info!("Session closed");
            IpcResponse::Ok
        }

        IpcRequest::EnumSessions {
            app_guid,
            timeout_ms,
        } => {
            let sessions = transport.enum_sessions(app_guid, *timeout_ms);
            let entries: Vec<SessionListEntry> = sessions
                .into_iter()
                .map(|s| SessionListEntry {
                    guid_instance: s.guid_instance,
                    guid_application: s.guid_application,
                    session_name: s.session_name,
                    max_players: s.max_players,
                    current_players: s.current_players,
                    flags: s.flags,
                    host_ticket: s.host_ticket,
                })
                .collect();
            IpcResponse::SessionList { sessions: entries }
        }

        IpcRequest::CreatePlayer { name, flags, data } => {
            match transport.create_player(name.clone(), *flags, data.clone()) {
                Some(player_id) => IpcResponse::PlayerCreated { player_id },
                None => IpcResponse::Error {
                    message: "Failed to create player".to_string(),
                },
            }
        }

        IpcRequest::DestroyPlayer { player_id } => {
            if transport.destroy_player(*player_id) {
                IpcResponse::Ok
            } else {
                IpcResponse::Error {
                    message: format!("Player {} not found or not destroyable", player_id),
                }
            }
        }

        IpcRequest::Send {
            from,
            to,
            data,
            guaranteed,
        } => {
            debug!(from, to, size = data.len(), guaranteed, "handling Send");
            match transport.send(*from, *to, data.clone(), *guaranteed) {
                Ok(()) => {
                    debug!("send completed");
                    IpcResponse::Ok
                }
                Err(e) => {
                    warn!(error = ?e, "send failed");
                    IpcResponse::Error {
                        message: format!("Send failed: {:?}", e),
                    }
                }
            }
        }

        IpcRequest::Receive => {
            let msg = transport.receive().map(|m| QueuedMessage {
                from: m.from,
                to: m.to,
                data: m.data,
                guaranteed: m.guaranteed,
            });
            match &msg {
                Some(m) => {
                    debug!(from = m.from, to = m.to, size = m.data.len(), "returning message")
                }
                None => debug!("no messages"),
            }
            IpcResponse::Message { message: msg }
        }

        IpcRequest::GetMessageCount => IpcResponse::MessageCount {
            count: transport.message_count(),
        },

        IpcRequest::GetOurTicket => IpcResponse::StringValue {
            value: transport.our_ticket().to_string(),
        },

        IpcRequest::IsHost => IpcResponse::Bool {
            value: transport.session_manager().is_host(),
        },

        IpcRequest::InSession => IpcResponse::Bool {
            value: transport.session_manager().in_session(),
        },

        IpcRequest::GetPlayers => {
            let players = transport.session_manager().get_players();
            let entries: Vec<PlayerListEntry> = players
                .into_iter()
                .map(|p| PlayerListEntry {
                    player_id: p.id,
                    name: p.name,
                    flags: p.flags,
                    is_local: p.is_local,
                })
                .collect();
            IpcResponse::PlayerList { players: entries }
        }

        IpcRequest::GetSessionDesc => IpcResponse::SessionDescValue {
            desc: transport.session_manager().get_session_desc(),
        },

        IpcRequest::SetPlayerData {
            player_id,
            data,
            local,
        } => {
            transport.set_player_data(*player_id, data.clone(), *local);
            IpcResponse::Ok
        }

        IpcRequest::GetPlayerData { player_id, local } => IpcResponse::PlayerData {
            data: transport.get_player_data(*player_id, *local),
        },

        IpcRequest::SetPlayerName { player_id, name } => {
            if transport.set_player_name(*player_id, name.clone()) {
                IpcResponse::Ok
            } else {
                IpcResponse::Error {
                    message: "Failed to set player name".to_string(),
                }
            }
        }

        IpcRequest::SetSessionDesc { desc } => {
            if transport.set_session_desc(desc.clone()) {
                IpcResponse::Ok
            } else {
                IpcResponse::Error {
                    message: "Failed to set session description (not host?)".to_string(),
                }
            }
        }
    }
}
