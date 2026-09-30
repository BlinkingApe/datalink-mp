//! datalink-mp - Native helper process for SMAC DirectPlay networking
//!
//! The Helper runs natively on the host OS and handles all Iroh networking.
//! The DLL running in Wine connects to it via TCP localhost.
//!
//! This library is the Helper itself; the `datalink-mp` binary is a thin
//! command line on top of [`start`].

mod controller;
mod ipc_server;

pub use controller::SessionController;

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
}

/// Why the Helper could not start.
#[derive(Debug, Error)]
pub enum StartError {
    #[error("Failed to bind TCP listener")]
    IpcBind(#[source] std::io::Error),

    #[error("Failed to create transport")]
    Transport(#[source] TransportError),
}

/// A running Helper.
pub struct Helper {
    ipc_port: u16,
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

    let controller = Arc::new(
        SessionController::new(config.transport_options).map_err(StartError::Transport)?,
    );
    let ipc_thread = ipc_server::spawn(listener, controller.clone());

    Ok(Helper {
        ipc_port,
        controller,
        ipc_thread,
    })
}

impl Helper {
    /// The port the IPC server is listening on
    pub fn ipc_port(&self) -> u16 {
        self.ipc_port
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
