//! datalink-mp - Native helper process for SMAC DirectPlay networking
//!
//! The Helper runs natively on the host OS and handles all Iroh networking.
//! The DLL running in Wine connects to it via TCP localhost.
//!
//! This library is the Helper itself; the `datalink-mp` binary is a thin
//! command line on top of [`start`].

mod controller;
mod http;
mod ipc_server;

pub use controller::{SessionController, State, Status};
pub use http::generate_token;

use iroh_transport::{TransportError, TransportOptions};
use std::sync::Arc;
use std::thread::JoinHandle;
use thiserror::Error;
use tracing::info;

/// Everything the Helper needs to start.
pub struct Config {
    /// Port to listen on for DLL connections, on 127.0.0.1. Port 0 picks a
    /// free port; [`Helper::ipc_port`] reports the one that was bound.
    pub ipc_port: u16,
    /// Options for the Transport. Production code uses the defaults.
    pub transport_options: TransportOptions,
    /// The web UI. None starts no HTTP server (the `host` and `join` subcommands).
    pub ui: Option<UiConfig>,
}

/// Called with the launch URL to open the player's browser on it.
pub type BrowserOpener = Box<dyn Fn(&str) + Send + Sync>;

/// The web UI's configuration.
pub struct UiConfig {
    /// Port to serve the page on, on 127.0.0.1. Port 0 picks a free port;
    /// [`Helper::ui_port`] reports the one that was bound.
    pub port: u16,
    /// The secret every API request must carry. See [`generate_token`].
    pub token: String,
    /// Opens the launch URL once the page is being served. None opens nothing.
    pub browser_opener: Option<BrowserOpener>,
}

/// Why the Helper could not start.
#[derive(Debug, Error)]
pub enum StartError {
    #[error("Failed to bind TCP listener")]
    IpcBind(#[source] std::io::Error),

    #[error("Failed to bind UI port {port}: another program may be using it. Close it, or pick another port with --ui-port")]
    UiBind {
        port: u16,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to start the HTTP server")]
    HttpServer(#[source] std::io::Error),

    #[error("Failed to create transport")]
    Transport(#[source] TransportError),
}

/// A running Helper.
pub struct Helper {
    ipc_port: u16,
    // Declared before the controller: the HTTP server is stopped first, so
    // the last reference to a Transport is never dropped on one of its
    // async worker threads.
    http: Option<http::HttpServer>,
    controller: Arc<SessionController>,
    ipc_thread: JoinHandle<()>,
}

/// Start the Helper: bind the IPC port, create the Transport and serve the DLL.
///
/// Returns once the Helper is running. It has a Ticket from this point on.
pub fn start(config: Config) -> Result<Helper, StartError> {
    // Bind first: a taken port is found out before an Iroh endpoint is started.
    let listener = ipc_server::bind(config.ipc_port).map_err(StartError::IpcBind)?;
    let ipc_port = listener.local_addr().map_err(StartError::IpcBind)?.port();
    info!("Listening on 127.0.0.1:{}", ipc_port);

    // Likewise the UI port.
    let ui_listener = config
        .ui
        .as_ref()
        .map(|ui| http::bind(ui.port).map_err(|source| StartError::UiBind { port: ui.port, source }))
        .transpose()?;

    let controller = Arc::new(
        SessionController::new(config.transport_options, ipc_port)
            .map_err(StartError::Transport)?,
    );
    let ipc_thread = ipc_server::spawn(listener, controller.clone());

    let http = match (config.ui, ui_listener) {
        (Some(ui), Some(listener)) => {
            let server = http::spawn(listener, controller.clone(), ui.token)
                .map_err(StartError::HttpServer)?;
            if let Some(open) = ui.browser_opener {
                open(&server.launch_url());
            }
            Some(server)
        }
        _ => None,
    };

    Ok(Helper {
        ipc_port,
        http,
        controller,
        ipc_thread,
    })
}

impl Helper {
    /// The port the IPC server is listening on
    pub fn ipc_port(&self) -> u16 {
        self.ipc_port
    }

    /// The port the page is served on, if the Helper has a UI
    pub fn ui_port(&self) -> Option<u16> {
        self.http.as_ref().map(|http| http.port())
    }

    /// The URL that opens the page, token included, if the Helper has a UI.
    ///
    /// The token is a secret: print it for the player, never log it.
    pub fn launch_url(&self) -> Option<String> {
        self.http.as_ref().map(|http| http.launch_url())
    }

    /// The Session controller
    pub fn controller(&self) -> &SessionController {
        &self.controller
    }

    /// Block for as long as the IPC server runs, which is the life of the process.
    pub fn wait(self) {
        if let Err(panic) = self.ipc_thread.join() {
            // A Helper whose IPC server died is of no use to the game.
            std::panic::resume_unwind(panic);
        }
    }

    /// Close every peer connection and the endpoint gracefully (blocking).
    ///
    /// Takes up to `Transport::SHUTDOWN_TIMEOUT`. The IPC listener stays bound:
    /// it lives as long as the process does.
    pub fn shutdown(self) {
        self.controller.transport().shutdown();
    }
}
