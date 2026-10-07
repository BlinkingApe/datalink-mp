//! datalink-mp - Native helper process for SMAC DirectPlay networking
//!
//! The Helper runs natively on the host OS and handles all Iroh networking.
//! The DLL running in Wine connects to it via TCP localhost.
//!
//! This library is the Helper itself; the `datalink-mp` binary is a thin
//! command line on top of [`start`].

mod controller;
mod http;
mod instance;
mod ipc_server;
mod platform;

pub use controller::{
    Banner, InvalidTicket, JoinError, JoinRefused, SessionController, State, Status, BUILD_ID,
};
pub use http::generate_token;
pub use platform::{system_browser_opener, SelfCheck};

use iroh_transport::{TransportError, TransportOptions};
use std::path::PathBuf;
use std::net::TcpListener;
use std::sync::Arc;
use std::time::{Duration, Instant};
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

/// Called with the launch URL to open the player's browser on it. Answers
/// whether the browser was started.
pub type BrowserOpener = Box<dyn Fn(&str) -> bool + Send + Sync>;

/// The application's name, as `GET /api/instance` reports it and as the
/// binary prints it.
pub const APP_NAME: &str = "datalink-mp";

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

    /// Another Helper is running on this IPC port and stays, because it is in
    /// use. It has been asked to show its page. Not a failure: the binary says
    /// so and exits with status 0.
    #[error("datalink-mp is already running and in use, so it kept running. Its page has opened.")]
    AlreadyRunning,

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
    let bound = match ipc_server::bind(config.ipc_port) {
        // Unless the program holding it is a Helper: then the player has
        // started datalink-mp again. A Helper nothing is using makes way;
        // one in use stays and shows its page.
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            let running = config
                .ui
                .as_ref()
                .and_then(|ui| instance::ask_running_helper_to_make_way(config.ipc_port, ui.port));
            match running {
                Some(instance::Running::Staying) => return Err(StartError::AlreadyRunning),
                Some(instance::Running::MakingWay) => bind_once_freed(config.ipc_port)
                    .map_err(|_| StartError::AlreadyRunning)?,
                None => return start_without_the_ipc_port(config, e),
            }
        }
        bound => bound.map_err(StartError::IpcBind)?,
    };
    let port = bound.local_addr().map_err(StartError::IpcBind)?.port();
    info!("Listening on 127.0.0.1:{}", port);
    start_with(config, Some(bound), port)
}

/// How long a Helper that makes way may take to free the IPC port.
const MAKE_WAY_DEADLINE: Duration = Duration::from_secs(10);

/// Bind the IPC port once the Helper making way has freed it.
fn bind_once_freed(port: u16) -> std::io::Result<TcpListener> {
    let deadline = Instant::now() + MAKE_WAY_DEADLINE;
    loop {
        match ipc_server::bind(port) {
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(100));
            }
            bound => return bound,
        }
    }
}

/// Another program holds the IPC port. With a page to say so on, that is not
/// fatal: the Helper runs without the game and shows a banner.
fn start_without_the_ipc_port(config: Config, e: std::io::Error) -> Result<Helper, StartError> {
    if config.ui.is_none() {
        return Err(StartError::IpcBind(e));
    }
    warn!("IPC port {} is in use by another program", config.ipc_port);
    let port = config.ipc_port;
    start_with(config, None, port)
}

/// Start the Helper with the IPC port bound, or, when `listener` is None,
/// held by another program.
fn start_with(config: Config, listener: Option<TcpListener>, ipc_port: u16) -> Result<Helper, StartError> {
    // A taken UI port is not fatal either: the Helper walks on to
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
            "{} is not the Game folder (DLL found: {}, game executables: {:?})",
            self_check.folder, self_check.dll_found, self_check.game_exes
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
            let opener = ui.browser_opener.map(Arc::new);
            let server = http::spawn(listener, controller.clone(), ipc_server.as_ref().map(|_| ipc_port), ui.token, opener.clone())
                .map_err(StartError::HttpServer)?;
            if let Some(open) = opener {
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
