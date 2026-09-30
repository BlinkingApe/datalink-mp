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
mod platform;

pub use controller::{
    Banner, InvalidTicket, JoinError, JoinRefused, SessionController, State, Status,
};
pub use http::generate_token;
pub use platform::{system_browser_opener, SelfCheck};

use iroh_transport::{TransportError, TransportOptions};
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use tracing::{info, warn};

/// Everything the Helper needs to start.
pub struct Config {
    /// The Game folder: where the self-check looks for the DLL and the game.
    /// The binary passes the folder its own executable is in, never the
    /// working directory.
    pub game_folder: PathBuf,
    /// Port to listen on for DLL connections, on 127.0.0.1. Port 0 picks a
    /// free port; [`Helper::ipc_port`] reports the one that was bound.
    ///
    /// When another program holds the port, a Helper with a UI starts all the
    /// same and says so on the page; one without a UI does not start.
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

    #[error("Could not find a free port for the page: ports {first} to {last} are all taken. Close the programs using them, or pick another port with --ui-port")]
    UiBind {
        first: u16,
        last: u16,
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
    /// None when another program holds the IPC port: there is no IPC server.
    ipc_server: Option<ipc_server::IpcServer>,
}

/// Start the Helper: bind the IPC port, create the Transport and serve the DLL.
///
/// Returns once the Helper is running. It has a Ticket from this point on.
/// A Helper with a UI runs even when another program holds the IPC port: it
/// then serves no DLL, and its status carries the `ipc_port_in_use` banner.
pub fn start(config: Config) -> Result<Helper, StartError> {
    // Bind first: a taken port is found out before an Iroh endpoint is started.
    let (listener, ipc_port) = match ipc_server::bind(config.ipc_port) {
        Ok(listener) => {
            let port = listener.local_addr().map_err(StartError::IpcBind)?.port();
            info!("Listening on 127.0.0.1:{}", port);
            (Some(listener), port)
        }
        // With a page to say so on, a port held by another program is not
        // fatal: the Helper runs without the game and shows a banner.
        Err(e) if config.ui.is_some() && e.kind() == std::io::ErrorKind::AddrInUse => {
            warn!("IPC port {} is in use by another program", config.ipc_port);
            (None, config.ipc_port)
        }
        Err(e) => return Err(StartError::IpcBind(e)),
    };

    // Likewise the UI port. A taken port is not fatal: the Helper walks on to
    // the next free one, and reports the port it bound.
    let ui_listener = config
        .ui
        .as_ref()
        .map(|ui| {
            http::bind_walking(ui.port).map_err(|walk| StartError::UiBind {
                first: walk.first,
                last: walk.last,
                source: walk.source,
            })
        })
        .transpose()?;

    // The self-check, once at startup for the log. Status does it again on
    // every request, and a failed check stops nothing.
    let self_check = platform::check_game_folder(&config.game_folder);
    if self_check.passed {
        info!("Running from the Game folder {}", self_check.folder);
    } else {
        warn!(
            "{} is not the Game folder (DLL found: {}, game executable: {:?})",
            self_check.folder, self_check.dll_found, self_check.game_exe
        );
    }

    let controller = Arc::new(
        SessionController::new(
            config.transport_options,
            config.game_folder,
            ipc_port,
            listener.is_none(),
        )
        .map_err(StartError::Transport)?,
    );
    let ipc_server =
        listener.map(|listener| ipc_server::spawn(listener, ipc_port, controller.clone()));

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
        ipc_server,
    })
}

impl Helper {
    /// The port the IPC server is listening on, or the port it could not have
    /// because another program holds it
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

    /// Block until the Helper has shut down, which is what Quit makes it do,
    /// then stop its servers, which releases their ports.
    ///
    /// A Helper without a UI has no Quit: this blocks for the life of the
    /// process, unless the IPC server dies, which shuts the Helper down too.
    /// Its panic is passed on from here.
    pub fn wait(self) {
        self.controller.wait_for_shutdown();
        self.stop_servers();
    }

    /// Shut the Helper down (blocking): close every peer connection and the
    /// endpoint gracefully, then stop its servers, which releases their ports.
    ///
    /// Closing takes up to `Transport::SHUTDOWN_TIMEOUT`.
    pub fn shutdown(self) {
        self.controller.shutdown();
        self.stop_servers();
    }

    /// Stop the HTTP server and the IPC server, which releases their ports.
    fn stop_servers(self) {
        // The HTTP server first: see the field's declaration.
        drop(self.http);
        if let Some(ipc_server) = self.ipc_server {
            if let Err(panic) = ipc_server.stop() {
                // The IPC server died, and the Helper shut down because of it.
                std::panic::resume_unwind(panic);
            }
        }
    }
}
