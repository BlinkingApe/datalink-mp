//! datalink-mp - Native helper process for SMAC DirectPlay networking
//!
//! This binary runs natively on the host OS and handles all Iroh networking.
//! The DLL running in Wine connects to this helper via TCP localhost.
//! It is a thin command line over the `datalink_mp` library.
//!
//! ## Modes
//!
//! - no subcommand: start the web UI (the player's browser opens the page)
//! - `host`: Host a multiplayer session and print the Ticket
//! - `join`: Join an existing session via ticket
//!
//! ## Logging
//!
//! Set `SMAC_HELPER_LOG_FILE` to a path to log to a file instead of stderr.
//! The ticket is always printed to stdout for easy capture.

use anyhow::Result;
use clap::{Parser, Subcommand};
use datalink_mp::{Config, Helper, StartError, UiConfig};
use ipc_protocol::DEFAULT_PORT;
use iroh_transport::TransportOptions;
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::prelude::*;

/// Native helper process for SMAC DirectPlay networking via Iroh
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Port to serve the page on (UI mode)
    #[arg(long, env = "SMAC_UI_PORT", default_value_t = DEFAULT_UI_PORT)]
    ui_port: u16,

    /// Port to listen on for DLL connections (UI mode)
    #[arg(short, long, default_value_t = DEFAULT_PORT)]
    port: u16,

    /// Do not open the browser (UI mode)
    #[arg(long)]
    no_browser: bool,
}

/// The port the page is served on unless told otherwise
const DEFAULT_UI_PORT: u16 = 47700;

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
        None => run_ui(cli.port, cli.ui_port, cli.no_browser),
        Some(Command::Host { port }) => run_host(port),
        Some(Command::Join { port, ticket }) => run_join(port, ticket),
    }
}

/// Run the web UI: start the Helper with its page and serve until Quit
fn run_ui(port: u16, ui_port: u16, no_browser: bool) -> Result<()> {
    let ipc_port = resolve_ipc_port(port);
    info!("datalink-mp starting in UI mode on port {}", ipc_port);

    let started = datalink_mp::start(Config {
        game_folder: game_folder(),
        ipc_port,
        transport_options: TransportOptions::default(),
        ui: Some(UiConfig {
            port: ui_port,
            token: datalink_mp::generate_token()?,
            browser_opener: (!no_browser).then(datalink_mp::system_browser_opener),
        }),
    });
    let helper = match started {
        Ok(helper) => helper,
        // A second double-click: the running Helper has opened its page.
        Err(e @ StartError::AlreadyRunning) => {
            println!("{e}");
            return Ok(());
        }
        Err(e @ StartError::Transport(_)) => explain_and_wait(&e),
        Err(e) => return Err(e.into()),
    };

    // The launch URL carries the token: printing it here is the only place it appears.
    println!("datalink-mp {} (build {})", env!("CARGO_PKG_VERSION"), datalink_mp::BUILD_ID);
    println!(
        "{}",
        helper.launch_url().expect("a Helper started with a UI has a launch URL")
    );
    println!("If your browser did not open by itself, open the address above. To quit, press Quit on the page, or Ctrl+C in this window.");

    helper.wait();
    Ok(())
}

/// The Helper could not start its networking: say so in plain words and wait
/// for Enter, so that a double-clicked window does not vanish before the player
/// can read it. Then exit with an error.
fn explain_and_wait(error: &StartError) -> ! {
    let mut reasons = error.to_string();
    let mut source = std::error::Error::source(error);
    while let Some(cause) = source {
        reasons.push_str(&format!(": {cause}"));
        source = cause.source();
    }
    eprintln!("datalink-mp could not start its networking, so it cannot connect you to your friends.");
    eprintln!("The reason: {reasons}");
    eprintln!("Check that this computer is allowed to use the network (a firewall may be blocking datalink-mp), then start it again.");
    eprintln!();
    eprintln!("Press Enter to close this window.");
    let _ = std::io::stdin().read_line(&mut String::new());
    std::process::exit(1);
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

/// The Game folder: the folder this executable is in, wherever the Helper was
/// started from. Never the working directory, which is whatever the launcher
/// or the shell happened to be in.
///
/// If the OS cannot say where the executable is, the folder is unknown (an
/// empty path) and the self-check fails, instead of guessing.
fn game_folder() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
        .unwrap_or_default()
}

/// Start the Helper and print its Ticket
fn start_helper(ipc_port: u16) -> Result<Helper> {
    let helper = datalink_mp::start(Config {
        game_folder: game_folder(),
        ipc_port,
        transport_options: TransportOptions::default(),
        ui: None,
    })?;

    // Print ticket to stdout (NOT to log file) so it can be captured
    println!("{}", helper.controller().transport().our_ticket());
    info!("Our ticket printed to stdout");

    Ok(helper)
}
