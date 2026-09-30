//! Session controller: owns the Helper's current Transport.
//!
//! The IPC server and the command line both go through the controller, so
//! there is one place that knows which Transport is current.

use iroh_transport::{Transport, TransportOptions, TransportResult, STREAM_PROTO_VERSION};
use serde::Serialize;
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
    /// Short IDs of the connected Helpers
    pub peers: Vec<String>,
    /// Codes of the active banners
    pub banners: Vec<&'static str>,
}

/// Owns the current Transport, and therefore the Helper's Ticket.
pub struct SessionController {
    transport: Arc<Transport>,
    ipc_port: u16,
}

impl SessionController {
    /// Create the controller with a live Transport (this starts the Iroh endpoint)
    pub(crate) fn new(options: TransportOptions, ipc_port: u16) -> TransportResult<Self> {
        info!("Initializing Iroh transport...");
        let transport = Transport::with_options(options)?;
        info!(
            "Transport initialized. Endpoint ID: {:?}",
            transport.endpoint_id()
        );

        Ok(Self {
            transport: Arc::new(transport),
            ipc_port,
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
        Status {
            release_version: env!("CARGO_PKG_VERSION"),
            ipc_version: ipc_protocol::PROTOCOL_VERSION,
            peer_protocol_version: STREAM_PROTO_VERSION,
            os: OS,
            state: State::Ready,
            ticket: transport.our_ticket().to_string(),
            ticket_seq: FIRST_TICKET_SEQ,
            ipc_port: self.ipc_port,
            peers: Vec::new(),
            banners: Vec::new(),
        }
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
