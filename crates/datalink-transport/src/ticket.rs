//! Ticket encoding for SMAC DirectPlay
//!
//! Uses Iroh-style ticket format: `{KIND}{BASE32_NOPAD(postcard_bytes)}`
//!
//! This makes tickets easy to copy/paste as a single string.

use iroh::EndpointAddr;
use std::fmt;

/// The ticket kind prefix
const TICKET_KIND: &str = "smac";

/// Error parsing a ticket
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TicketError {
    /// Missing the expected prefix
    MissingPrefix,
    /// Invalid base32 encoding
    InvalidBase32,
    /// Invalid postcard data
    InvalidPostcard,
}

impl fmt::Display for TicketError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TicketError::MissingPrefix => write!(f, "missing 'smac' prefix"),
            TicketError::InvalidBase32 => write!(f, "invalid base32 encoding"),
            TicketError::InvalidPostcard => write!(f, "invalid postcard data"),
        }
    }
}

impl std::error::Error for TicketError {}

/// A ticket containing an EndpointAddr, serialized in Iroh-style format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ticket {
    addr: EndpointAddr,
}

impl Ticket {
    /// Create a new ticket from an EndpointAddr
    pub fn new(addr: EndpointAddr) -> Self {
        Self { addr }
    }

    /// Get the underlying EndpointAddr
    pub fn addr(&self) -> &EndpointAddr {
        &self.addr
    }

    /// Consume the ticket and return the EndpointAddr
    pub fn into_addr(self) -> EndpointAddr {
        self.addr
    }

    /// Serialize the ticket to a string
    ///
    /// Format: `smac{BASE32_NOPAD(postcard(addr))}`
    pub fn serialize(&self) -> String {
        let bytes = postcard::to_stdvec(&self.addr).expect("EndpointAddr serialization cannot fail");
        let mut out = TICKET_KIND.to_string();
        data_encoding::BASE32_NOPAD.encode_append(&bytes, &mut out);
        out.to_ascii_lowercase()
    }

    /// Parse a ticket from a string
    pub fn parse(s: &str) -> Result<Self, TicketError> {
        // Strip the prefix (case-insensitive)
        let rest = s
            .strip_prefix(TICKET_KIND)
            .or_else(|| s.strip_prefix(&TICKET_KIND.to_ascii_uppercase()))
            .ok_or(TicketError::MissingPrefix)?;

        // Decode base32 (case-insensitive)
        let bytes = data_encoding::BASE32_NOPAD
            .decode(rest.to_ascii_uppercase().as_bytes())
            .map_err(|_| TicketError::InvalidBase32)?;

        // Deserialize with postcard
        let addr: EndpointAddr =
            postcard::from_bytes(&bytes).map_err(|_| TicketError::InvalidPostcard)?;

        Ok(Self { addr })
    }
}

impl fmt::Display for Ticket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.serialize())
    }
}

impl std::str::FromStr for Ticket {
    type Err = TicketError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    #[test]
    fn test_ticket_roundtrip() {
        // Create a test EndpointAddr
        let id = iroh::SecretKey::generate().public();
        let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();
        let endpoint_addr = EndpointAddr::new(id).with_ip_addr(addr);

        // Create ticket and serialize
        let ticket = Ticket::new(endpoint_addr.clone());
        let serialized = ticket.serialize();

        // Verify format
        assert!(serialized.starts_with("smac"), "should start with 'smac' prefix");
        assert!(serialized.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()),
                "should be lowercase alphanumeric");

        // Parse back
        let parsed = Ticket::parse(&serialized).expect("should parse");
        assert_eq!(parsed.addr().id, endpoint_addr.id);
    }

    #[test]
    fn test_ticket_case_insensitive() {
        let id = iroh::SecretKey::generate().public();
        let endpoint_addr = EndpointAddr::new(id);
        let ticket = Ticket::new(endpoint_addr);
        let serialized = ticket.serialize();

        // Should parse uppercase version too
        let upper = serialized.to_ascii_uppercase();
        let parsed = Ticket::parse(&upper).expect("should parse uppercase");
        assert_eq!(parsed.addr().id, ticket.addr().id);
    }

    #[test]
    fn test_ticket_invalid_prefix() {
        let err = Ticket::parse("wrongprefix123").unwrap_err();
        assert_eq!(err, TicketError::MissingPrefix);
    }

    #[test]
    fn test_ticket_invalid_base32() {
        // Invalid base32 characters after prefix
        let err = Ticket::parse("smac!!!invalid").unwrap_err();
        assert_eq!(err, TicketError::InvalidBase32);
    }
}
