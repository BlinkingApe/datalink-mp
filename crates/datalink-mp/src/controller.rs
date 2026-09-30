//! Session controller: owns the Helper's current Transport and the status
//! the page shows.
//!
//! The IPC server and the command line both go through the controller, so
//! there is one place that knows which Transport is current. The IPC server
//! also reports here what it sees of the game's DLL.

use iroh_transport::{Transport, TransportOptions, TransportResult, STREAM_PROTO_VERSION};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::info;

/// The Ticket sequence number of the Ticket the Helper starts with.
const FIRST_TICKET_SEQ: u64 = 1;

/// Where the Helper is in a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    /// No peers, no dial in progress
    Ready,
}

/// Something the page must tell the player, as a code the page has the words for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Banner {
    /// Another program holds the IPC port, so the game cannot reach the Helper
    IpcPortInUse,
    /// The game's DLL does not speak this Helper's IPC version
    IpcVersionMismatch,
}

/// A snapshot of the Helper's status, as the page shows it.
#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub release_version: &'static str,
    pub ipc_version: u32,
    pub peer_protocol_version: u16,
    /// `windows`, `linux` or `macos`
    pub os: &'static str,
    pub state: State,
    pub ticket: String,
    pub ticket_seq: u64,
    pub ipc_port: u16,
    /// Whether the game's DLL is connected to the Helper
    pub game_connected: bool,
    /// Short IDs of the connected Helpers
    pub peers: Vec<String>,
    /// Codes of the active banners
    pub banners: Vec<Banner>,
}

/// Owns the current Transport, and therefore the Helper's Ticket.
pub struct SessionController {
    transport: Arc<Transport>,
    ipc_port: u16,
    ipc_port_in_use: bool,
    game_connected: AtomicBool,
    ipc_version_mismatch: AtomicBool,
}

impl SessionController {
    /// Create the controller with a live Transport (this starts the Iroh endpoint).
    ///
    /// `ipc_port_in_use` says that another program holds `ipc_port`, so no IPC
    /// server is listening on it.
    pub(crate) fn new(
        options: TransportOptions,
        ipc_port: u16,
        ipc_port_in_use: bool,
    ) -> TransportResult<Self> {
        info!("Initializing Iroh transport...");
        let transport = Transport::with_options(options)?;
        info!(
            "Transport initialized. Endpoint ID: {:?}",
            transport.endpoint_id()
        );

        Ok(Self {
            transport: Arc::new(transport),
            ipc_port,
            ipc_port_in_use,
            game_connected: AtomicBool::new(false),
            ipc_version_mismatch: AtomicBool::new(false),
        })
    }

    /// The current Transport.
    ///
    /// Ask each time one is needed instead of keeping the result: the current
    /// Transport is replaced by Stop.
    pub fn transport(&self) -> Arc<Transport> {
        self.transport.clone()
    }

    /// A snapshot of the Helper's status. Only non-blocking reads: safe to call
    /// from async code.
    pub fn status(&self) -> Status {
        let transport = self.transport();
        let mut banners = Vec::new();
        if self.ipc_port_in_use {
            banners.push(Banner::IpcPortInUse);
        }
        if self.ipc_version_mismatch.load(Ordering::Relaxed) {
            banners.push(Banner::IpcVersionMismatch);
        }
        Status {
            release_version: env!("CARGO_PKG_VERSION"),
            ipc_version: ipc_protocol::PROTOCOL_VERSION,
            peer_protocol_version: STREAM_PROTO_VERSION,
            os: OS,
            state: State::Ready,
            ticket: transport.our_ticket().to_string(),
            ticket_seq: FIRST_TICKET_SEQ,
            ipc_port: self.ipc_port,
            game_connected: self.game_connected.load(Ordering::Relaxed),
            peers: Vec::new(),
            banners,
        }
    }

    /// The IPC server reports that a DLL handshake succeeded: the game is
    /// connected, and the DLL in the Game folder is the right one.
    pub(crate) fn dll_handshake_succeeded(&self) {
        self.game_connected.store(true, Ordering::Relaxed);
        self.ipc_version_mismatch.store(false, Ordering::Relaxed);
    }

    /// The IPC server reports a DLL that does not speak this Helper's IPC version.
    pub(crate) fn dll_mismatched(&self) {
        self.ipc_version_mismatch.store(true, Ordering::Relaxed);
    }

    /// The IPC server reports that the DLL's connection ended, whichever way.
    pub(crate) fn dll_disconnected(&self) {
        self.game_connected.store(false, Ordering::Relaxed);
    }

    /// Dial the Helper named by a friend's Ticket (blocking).
    pub fn join(&self, ticket: &str) -> TransportResult<()> {
        info!("Connecting to host ticket: {}", ticket);
        self.transport().connect_to_peer(ticket)?;
        info!("Connected to host!");
        Ok(())
    }
}

const OS: &str = if cfg!(windows) {
    "windows"
} else if cfg!(target_os = "macos") {
    "macos"
} else {
    "linux"
};
