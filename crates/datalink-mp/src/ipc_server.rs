//! IPC server: answers the DLL's requests over TCP localhost.
//!
//! The listener is bound once at startup and served on a plain thread for the
//! life of the process.

use crate::controller::SessionController;
use anyhow::Result;
use ipc_protocol::{
    decode_request, encode_response, read_message, IpcError, IpcRequest, IpcResponse,
    PlayerListEntry, QueuedMessage, SessionListEntry, PROTOCOL_VERSION,
};
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread::JoinHandle;
use tracing::{debug, debug_span, error, info, warn};

/// Bind the listener for DLL connections. Port 0 picks a free port.
pub(crate) fn bind(port: u16) -> std::io::Result<TcpListener> {
    TcpListener::bind(("127.0.0.1", port))
}

/// Serve DLL connections on their own thread.
pub(crate) fn spawn(listener: TcpListener, controller: Arc<SessionController>) -> JoinHandle<()> {
    std::thread::spawn(move || serve(listener, &controller))
}

fn serve(listener: TcpListener, controller: &SessionController) {
    // Accept connections (single-threaded for simplicity - one DLL at a time)
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                info!("Client connected from {:?}", stream.peer_addr());
                if let Err(e) = handle_client(stream, controller) {
                    error!("Client error: {:?}", e);
                }
                // The connection is over, cleanly or not.
                controller.dll_disconnected();
                info!("Client disconnected");
            }
            Err(e) => {
                error!("Accept error: {:?}", e);
            }
        }
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
            match transport.join_session_by_ticket(host_ticket) {
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
