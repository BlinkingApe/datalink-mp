//! datalink-mp - Native helper process for SMAC DirectPlay networking
//!
//! This binary runs natively on the host OS and handles all Iroh networking.
//! The DLL running in Wine connects to this helper via TCP localhost.
//! It is a thin command line over the `datalink_mp` library.
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

use anyhow::Result;
use clap::{Parser, Subcommand};
use datalink_mp::{Config, Helper};
use ipc_protocol::DEFAULT_PORT;
use iroh_transport::TransportOptions;
use tracing::info;
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
            .add_directive("datalink_mp=debug".parse().unwrap())
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

/// Run in host mode - start the Helper and serve DLL connections
fn run_host(port: u16) -> Result<()> {
    let port = resolve_ipc_port(port);
    info!("datalink-mp starting in HOST mode on port {}", port);

    let helper = start_helper(port)?;

    helper.wait();
    Ok(())
}

/// Run in join mode - start the Helper, connect to host, then serve DLL connections
fn run_join(port: u16, host_ticket: String) -> Result<()> {
    let port = resolve_ipc_port(port);
    info!("datalink-mp starting in JOIN mode on port {}", port);

    let helper = start_helper(port)?;
    helper.controller().join(&host_ticket)?;

    helper.wait();
    Ok(())
}

/// The IPC port to use: `SMAC_HELPER_PORT` if set, otherwise the `--port` value
fn resolve_ipc_port(port: u16) -> u16 {
    // Allow env var override for backwards compatibility
    std::env::var("SMAC_HELPER_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(port)
}

/// Start the Helper and print its Ticket
fn start_helper(ipc_port: u16) -> Result<Helper> {
    let helper = datalink_mp::start(Config {
        ipc_port,
        transport_options: TransportOptions::default(),
    })?;

    // Print ticket to stdout (NOT to log file) so it can be captured
    println!("{}", helper.controller().transport().our_ticket());
    info!("Our ticket printed to stdout");

    Ok(helper)
}
