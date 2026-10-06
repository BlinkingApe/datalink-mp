//! Iroh-based transport layer for DirectPlay bridge
//!
//! This crate provides the networking layer that maps DirectPlay concepts
//! to Iroh P2P connections.

pub mod capture;
pub mod path;
pub mod protocol;
pub mod session;
pub mod connection;
pub mod runtime;
pub mod ticket;
pub mod turn_sync;

pub use protocol::*;
pub use session::*;
pub use connection::*;
pub use runtime::*;
pub use ticket::*;

use thiserror::Error;

/// Errors from the transport layer
#[derive(Debug, Error)]
pub enum TransportError {
    #[error("Iroh connection error: {0}")]
    Connection(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] postcard::Error),

    #[error("Session not found")]
    SessionNotFound,

    #[error("Player not found")]
    PlayerNotFound,

    #[error("Not connected")]
    NotConnected,

    #[error("Already connected")]
    AlreadyConnected,

    #[error("Session full")]
    SessionFull,

    #[error("Invalid message")]
    InvalidMessage,

    #[error("Protocol version mismatch (peer speaks {0:#06x}, we speak {1:#06x})")]
    ProtocolMismatch(u16, u16),

    /// A dial the other Helper rejected for the ALPN: it speaks a different
    /// Peer protocol version, so the two players run different builds.
    #[error("Peer protocol version mismatch (the other Helper is a different build)")]
    PeerProtocolMismatch,

    /// A dial that timed out or could not connect.
    #[error("Can't reach the other Helper")]
    CantReach,

    #[error("Invalid ticket: {0}")]
    InvalidTicket(#[from] ticket::TicketError),

    #[error("Timeout")]
    Timeout,

    #[error("Channel closed")]
    ChannelClosed,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}


impl From<iroh::endpoint::ConnectionError> for TransportError {
    fn from(e: iroh::endpoint::ConnectionError) -> Self {
        TransportError::Connection(e.to_string())
    }
}

impl From<iroh::endpoint::ConnectingError> for TransportError {
    fn from(e: iroh::endpoint::ConnectingError) -> Self {
        TransportError::Connection(e.to_string())
    }
}

impl From<iroh::endpoint::ConnectError> for TransportError {
    fn from(e: iroh::endpoint::ConnectError) -> Self {
        TransportError::Connection(e.to_string())
    }
}

impl From<iroh::endpoint::WriteError> for TransportError {
    fn from(e: iroh::endpoint::WriteError) -> Self {
        TransportError::Connection(e.to_string())
    }
}

impl From<iroh::endpoint::ClosedStream> for TransportError {
    fn from(e: iroh::endpoint::ClosedStream) -> Self {
        TransportError::Connection(e.to_string())
    }
}

impl From<iroh::endpoint::ReadToEndError> for TransportError {
    fn from(e: iroh::endpoint::ReadToEndError) -> Self {
        TransportError::Connection(e.to_string())
    }
}

impl From<iroh::endpoint::ReadError> for TransportError {
    fn from(e: iroh::endpoint::ReadError) -> Self {
        TransportError::Connection(e.to_string())
    }
}

impl From<anyhow::Error> for TransportError {
    fn from(e: anyhow::Error) -> Self {
        TransportError::Connection(e.to_string())
    }
}

pub type TransportResult<T> = Result<T, TransportError>;
