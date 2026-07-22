//! smac-helper - Native helper process for SMAC DirectPlay networking
//!
//! This binary runs natively on the host OS and handles all Iroh networking.
//! The DLL running in Wine connects to this helper via TCP localhost.
//!
//! ## Subcommands
//!
//! - `host`: Host a multiplayer session (default behavior)
//! - `join`: Join an existing session via ticket
//!
//! ## Logging
//!
//! Set `SMAC_HELPER_LOG_FILE` to a path to log to a file instead of stderr.
//! The ticket is always printed to stdout for easy capture.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use ipc_protocol::{
    decode_request, encode_response, read_message, IpcRequest, IpcResponse, PlayerListEntry,
    QueuedMessage, SessionListEntry, DEFAULT_PORT, PROTOCOL_VERSION,
};
use iroh_transport::Transport;
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use tracing::{debug, debug_span, error, info, warn};
use tracing_subscriber::prelude::*;

/// Native helper process for SMAC DirectPlay networking via Iroh
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Host a multiplayer session
    Host {
        /// Port to listen on for DLL connections
        #[arg(short, long, default_value_t = DEFAULT_PORT)]
        port: u16,
    },

    /// Join an existing multiplayer session
    Join {
        /// Port to listen on for DLL connections
        #[arg(short, long, default_value_t = DEFAULT_PORT)]
        port: u16,

        /// Host ticket to connect to (required)
        #[arg(short, long)]
        ticket: String,
    },
}

fn init_logging() {
    use std::fs::OpenOptions;
    use std::sync::Mutex as StdMutex;
    use tracing_subscriber::{fmt, EnvFilter};

    // Only log to file if SMAC_HELPER_LOG_FILE is set
    // No fallback - stdout must be clean for ticket capture
    let Ok(log_path) = std::env::var("SMAC_HELPER_LOG_FILE") else {
        return;
    };

    let Ok(file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    else {
        return;
    };

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("debug")
            .add_directive("smac_helper=debug".parse().unwrap())
            .add_directive("iroh_transport=debug".parse().unwrap())
    });

    let file = StdMutex::new(file);
    tracing_subscriber::registry()
        .with(filter)
        .with(
            fmt::layer()
                .with_writer(move || file.lock().unwrap().try_clone().unwrap())
                .with_ansi(false)
                .with_target(true),
        )
        .init();
}

fn main() -> Result<()> {
    // Initialize logging (to file if SMAC_HELPER_LOG_FILE is set)
    init_logging();

    let cli = Cli::parse();

    match cli.command {
        // Default to host mode if no subcommand given (backwards compatibility)
        None => run_host(DEFAULT_PORT),
        Some(Command::Host { port }) => run_host(port),
        Some(Command::Join { port, ticket }) => run_join(port, ticket),
    }
}

/// Run in host mode - create transport and accept DLL connections
fn run_host(port: u16) -> Result<()> {
    // Allow env var override for backwards compatibility
    let port = std::env::var("SMAC_HELPER_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(port);

    info!("smac-helper starting in HOST mode on port {}", port);

    // Create the transport (this starts the Iroh endpoint)
    info!("Initializing Iroh transport...");
    let transport = Transport::new().context("Failed to create transport")?;
    let transport = Arc::new(transport);

    info!(
        "Transport initialized. Endpoint ID: {:?}",
        transport.endpoint_id()
    );

    // Print ticket to stdout (NOT to log file) so it can be captured
    println!("{}", transport.our_ticket());
    info!("Our ticket printed to stdout");

    // Start TCP server for DLL connections
    run_tcp_server(port, transport)
}

/// Run in join mode - connect to host, then accept DLL connections
fn run_join(port: u16, host_ticket: String) -> Result<()> {
    // Allow env var override for backwards compatibility
    let port = std::env::var("SMAC_HELPER_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(port);

    info!("smac-helper starting in JOIN mode on port {}", port);

    // Create the transport
    info!("Initializing Iroh transport...");
    let transport = Transport::new().context("Failed to create transport")?;
    let transport = Arc::new(transport);

    info!(
        "Transport initialized. Endpoint ID: {:?}",
        transport.endpoint_id()
    );

    // Print our ticket to stdout
    println!("{}", transport.our_ticket());
    info!("Our ticket printed to stdout");

    // Connect to host
    info!("Connecting to host ticket: {}", host_ticket);
    transport.connect_to_peer(&host_ticket)?;
    info!("Connected to host!");

    // Start TCP server for DLL connections
    run_tcp_server(port, transport)
}

/// Start TCP server and handle DLL connections
fn run_tcp_server(port: u16, transport: Arc<Transport>) -> Result<()> {
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr).context("Failed to bind TCP listener")?;
    info!("Listening on {}", addr);

    // Accept connections (single-threaded for simplicity - one DLL at a time)
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                info!("Client connected from {:?}", stream.peer_addr());
                if let Err(e) = handle_client(stream, transport.clone()) {
                    error!("Client error: {:?}", e);
                }
                info!("Client disconnected");
            }
            Err(e) => {
                error!("Accept error: {:?}", e);
            }
        }
    }

    Ok(())
}

fn handle_client(mut stream: TcpStream, transport: Arc<Transport>) -> Result<()> {
    // Disable Nagle's algorithm for lower latency
    stream.set_nodelay(true)?;

    loop {
        // Read request
        let data = match read_message(&mut stream) {
            Ok(data) => data,
            Err(ipc_protocol::IpcError::Io(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                // Client closed connection
                return Ok(());
            }
            Err(e) => return Err(e.into()),
        };

        let request = decode_request(&data)?;
        let request_type = request.type_name();

        let span = debug_span!("ipc_request", request_type);
        let _guard = span.enter();

        let response = handle_request(&request, &transport);

        let encoded = encode_response(&response)?;
        // encode_response already includes length prefix, so we write it directly
        stream.write_all(&encoded)?;
        stream.flush()?;
    }
}

fn handle_request(request: &IpcRequest, transport: &Transport) -> IpcResponse {
    match request {
        IpcRequest::Handshake { protocol_version } => {
            if *protocol_version != PROTOCOL_VERSION {
                return IpcResponse::Error {
                    message: format!(
                        "Protocol version mismatch: expected {}, got {}",
                        PROTOCOL_VERSION, protocol_version
                    ),
                };
            }
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
