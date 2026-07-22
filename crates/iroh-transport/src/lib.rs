//! Iroh-based transport layer for DirectPlay bridge
//!
//! This crate provides the networking layer that maps DirectPlay concepts
//! to Iroh P2P connections.

pub mod protocol;
pub mod session;
pub mod connection;
pub mod runtime;
pub mod ticket;

pub use protocol::*;
pub use session::*;
pub use connection::*;
pub use runtime::*;
pub use ticket::*;

use thiserror::Error;

/// ALPN protocol identifier for DirectPlay over Iroh
pub const DPLAY_ALPN: &[u8] = b"dplay-iroh/1";

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
