//! Mock DirectPlay client for testing the transport layer
//!
//! This program simulates how a DirectPlay application would use our library.

use dp_types::*;
use iroh_transport::Transport;
use std::io::{self, BufRead, Write};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

fn main() {
    // Set up logging
    let _subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .with_target(false)
        .init();

    println!("DirectPlay-Iroh Mock Client");
    println!("===========================");
    println!();

    // Initialize transport
    println!("Initializing transport...");
    let transport = match Transport::new() {
        Ok(t) => {
            println!("Transport initialized!");
            println!("Node ID: {}", t.endpoint_id());
            t
        }
        Err(e) => {
            eprintln!("Failed to initialize transport: {}", e);
            return;
        }
    };

    println!();
    println!("Commands:");
    println!("  host <name>     - Host a new session");
    println!("  join <node_id>  - Join a session by host's node ID");
    println!("  players         - List players in session");
    println!("  send <message>  - Send a message to all players");
    println!("  recv            - Receive pending messages");
    println!("  close           - Close the session");
    println!("  quit            - Exit");
    println!();

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        print!("> ");
        stdout.flush().unwrap();

        let mut line = String::new();
        if stdin.lock().read_line(&mut line).is_err() {
            break;
        }

        let parts: Vec<&str> = line.trim().split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        match parts[0] {
            "host" => {
                let name = if parts.len() > 1 {
                    parts[1..].join(" ")
                } else {
                    "Test Session".to_string()
                };

                let desc = SessionDesc {
                    guid_instance: GUID::new_random(),
                    guid_application: GUID::new_random(),
                    max_players: 8,
                    session_name: name.clone(),
                    ..Default::default()
                };

                match transport.create_session(desc) {
                    Some(guid) => {
                        println!("Created session '{}' with GUID {:?}", name, guid);
                        println!("Other players can join using your Node ID: {}", transport.endpoint_id());
                    }
                    None => {
                        println!("Failed to create session (already in one?)");
                    }
                }
            }

            "join" => {
                if parts.len() < 2 {
                    println!("Usage: join <node_id>");
                    continue;
                }

                let node_id_str = parts[1];
                println!("Joining session at {}...", node_id_str);

                // Parse node ID (simplified - in reality would need proper parsing)
                let mut node_id_bytes = [0u8; 32];
                if let Ok(bytes) = hex::decode(node_id_str.replace("-", "")) {
                    if bytes.len() >= 32 {
                        node_id_bytes.copy_from_slice(&bytes[..32]);
                    }
                }

                // Connect for discovery first
                match transport.connect_for_discovery(node_id_bytes) {
                    Ok(()) => {
                        println!("Connected! Waiting for session info...");
                    }
                    Err(e) => {
                        println!("Failed to connect: {}", e);
                    }
                }
            }

            "player" | "create" => {
                let name = if parts.len() > 1 {
                    parts[1..].join(" ")
                } else {
                    "Player".to_string()
                };

                let player_name = PlayerName {
                    short_name: name.clone(),
                    long_name: name.clone(),
                };

                match transport.create_player(player_name, 0, Vec::new()) {
                    Some(id) => {
                        println!("Created player '{}' with ID {}", name, id);
                    }
                    None => {
                        println!("Failed to create player");
                    }
                }
            }

            "players" => {
                let players = transport.session_manager().get_players();
                if players.is_empty() {
                    println!("No players in session");
                } else {
                    println!("Players:");
                    for player in players {
                        let local = if player.is_local { " (local)" } else { "" };
                        println!(
                            "  {} - {} / {}{}",
                            player.id, player.name.short_name, player.name.long_name, local
                        );
                    }
                }
            }

            "send" => {
                if parts.len() < 2 {
                    println!("Usage: send <message>");
                    continue;
                }

                let message = parts[1..].join(" ");
                let data = message.as_bytes().to_vec();

                // Get local player ID
                let from = transport.session_manager().local_player_id().unwrap_or(0);

                match transport.send(from, dpid::DPID_ALLPLAYERS, data, true) {
                    Ok(()) => {
                        println!("Message sent!");
                    }
                    Err(e) => {
                        println!("Failed to send: {}", e);
                    }
                }
            }

            "recv" => {
                match transport.receive() {
                    Some(msg) => {
                        let text = String::from_utf8_lossy(&msg.data);
                        println!(
                            "Message from {} to {}: {}",
                            msg.from, msg.to, text
                        );
                    }
                    None => {
                        println!("No messages pending");
                    }
                }
            }

            "status" => {
                println!("Node ID: {}", transport.endpoint_id());
                println!("In session: {}", transport.session_manager().in_session());
                println!("Is host: {}", transport.session_manager().is_host());
                if let Some(desc) = transport.session_manager().get_session_desc() {
                    println!("Session: {}", desc.session_name);
                    println!("Players: {}/{}", desc.current_players, desc.max_players);
                }
            }

            "close" => {
                transport.close_session();
                println!("Session closed");
            }

            "quit" | "exit" | "q" => {
                println!("Goodbye!");
                break;
            }

            "help" | "?" => {
                println!("Commands:");
                println!("  host <name>     - Host a new session");
                println!("  join <node_id>  - Join a session");
                println!("  player <name>   - Create a local player");
                println!("  players         - List players");
                println!("  send <message>  - Send a message");
                println!("  recv            - Receive messages");
                println!("  status          - Show status");
                println!("  close           - Close session");
                println!("  quit            - Exit");
            }

            _ => {
                println!("Unknown command: {}", parts[0]);
                println!("Type 'help' for available commands");
            }
        }
    }
}
