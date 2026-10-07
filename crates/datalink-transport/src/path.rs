//! Which way a connection's traffic goes: a direct connection between the two
//! players' machines, or a relayed connection through a relay server.
//!
//! The page shows each friend's route as plain text, and the Helper logs a
//! line at info level for every path change, always, whether or not the
//! traffic capture is on.

use iroh::endpoint::{Connection, PathEvent};
use iroh::TransportAddr;
use std::fmt;
use tracing::info;

/// Whether a connection is direct or relayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PathKind {
    Direct,
    Relayed,
}

/// What kind of address the remote end of a path has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddrKind {
    Ipv4,
    Ipv6,
    Relay,
}

/// A path as the page and the log describe it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathClass {
    pub kind: PathKind,
    pub addr: AddrKind,
}

impl fmt::Display for PathKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            PathKind::Direct => "Direct",
            PathKind::Relayed => "Relayed",
        })
    }
}

impl fmt::Display for AddrKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            AddrKind::Ipv4 => "IPv4",
            AddrKind::Ipv6 => "IPv6",
            AddrKind::Relay => "relay",
        })
    }
}

/// Classify a path by its remote address. `None` for an address kind this
/// doesn't know, which is neither shown nor logged.
pub fn classify(addr: &TransportAddr) -> Option<PathClass> {
    match addr {
        // An IPv4 address carried in an IPv6 socket address is still IPv4.
        TransportAddr::Ip(ip) => Some(PathClass {
            kind: PathKind::Direct,
            addr: if ip.ip().to_canonical().is_ipv4() { AddrKind::Ipv4 } else { AddrKind::Ipv6 },
        }),
        TransportAddr::Relay(_) => Some(PathClass { kind: PathKind::Relayed, addr: AddrKind::Relay }),
        _ => None,
    }
}

/// The class of a connection's selected path, once one is selected.
pub fn selected_path(connection: &Connection) -> Option<PathClass> {
    let paths = connection.paths();
    let selected = paths.iter().find(|p| p.is_selected())?;
    classify(selected.remote_addr())
}

/// Log a path change: peer, Direct or Relayed, and the address kind.
fn log_change(peer: &str, class: PathClass) {
    info!(peer, route = %class.kind, address = %class.addr, "path {}: {}, {}", peer, class.kind, class.addr);
}

/// Log a line at info level for every path change of a connection, for as long
/// as it lives. `peer` is the peer's short ID.
pub fn spawn_path_logger(connection: Connection, peer: String) {
    tokio::spawn(async move {
        use n0_future::StreamExt;
        // Subscribe before the first look, so no change falls between them.
        let mut events = connection.path_events();
        let mut current = selected_path(&connection);
        if let Some(class) = current {
            log_change(&peer, class);
        }
        while let Some(event) = events.next().await {
            let PathEvent::Selected { remote_addr, .. } = event else { continue };
            let Some(class) = classify(&remote_addr) else { continue };
            if current != Some(class) {
                current = Some(class);
                log_change(&peer, class);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> TransportAddr {
        TransportAddr::Ip(s.parse().unwrap())
    }

    #[test]
    fn an_ipv4_address_is_direct() {
        let class = classify(&ip("203.0.113.5:4000")).unwrap();
        assert_eq!(class, PathClass { kind: PathKind::Direct, addr: AddrKind::Ipv4 });
    }

    #[test]
    fn an_ipv6_address_is_direct() {
        let class = classify(&ip("[2001:db8::1]:4000")).unwrap();
        assert_eq!(class, PathClass { kind: PathKind::Direct, addr: AddrKind::Ipv6 });
    }

    #[test]
    fn an_ipv4_mapped_ipv6_address_is_ipv4() {
        let class = classify(&ip("[::ffff:203.0.113.5]:4000")).unwrap();
        assert_eq!(class.addr, AddrKind::Ipv4);
    }

    #[test]
    fn a_relay_is_relayed() {
        let url: iroh::RelayUrl = "https://relay.example.com".parse().unwrap();
        let class = classify(&TransportAddr::Relay(url)).unwrap();
        assert_eq!(class, PathClass { kind: PathKind::Relayed, addr: AddrKind::Relay });
    }

    #[test]
    fn the_words_are_the_ones_the_log_uses() {
        assert_eq!(PathKind::Direct.to_string(), "Direct");
        assert_eq!(PathKind::Relayed.to_string(), "Relayed");
        assert_eq!(AddrKind::Relay.to_string(), "relay");
    }
}
