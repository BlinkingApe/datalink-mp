//! Session controller: owns the Helper's current Transport.
//!
//! The IPC server and the command line both go through the controller, so
//! there is one place that knows which Transport is current.

use iroh_transport::{Transport, TransportOptions, TransportResult};
use std::sync::Arc;
use tracing::info;

/// Owns the current Transport, and therefore the Helper's Ticket.
pub struct SessionController {
    transport: Arc<Transport>,
}

impl SessionController {
    /// Create the controller with a live Transport (this starts the Iroh endpoint)
    pub(crate) fn new(options: TransportOptions) -> TransportResult<Self> {
        info!("Initializing Iroh transport...");
        let transport = Transport::with_options(options)?;
        info!(
            "Transport initialized. Endpoint ID: {:?}",
            transport.endpoint_id()
        );

        Ok(Self {
            transport: Arc::new(transport),
        })
    }

    /// The current Transport.
    ///
    /// Ask each time one is needed instead of keeping the result: the current
    /// Transport is replaced by Stop.
    pub fn transport(&self) -> Arc<Transport> {
        self.transport.clone()
    }

    /// Dial the Helper named by a friend's Ticket (blocking).
    pub fn join(&self, ticket: &str) -> TransportResult<()> {
        info!("Connecting to host ticket: {}", ticket);
        self.transport().connect_to_peer(ticket)?;
        info!("Connected to host!");
        Ok(())
    }
}
