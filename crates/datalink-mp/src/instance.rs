//! Finding a Helper that already runs on the same IPC port.
//!
//! There are no lock files: a Helper that finds its IPC port taken asks the
//! ports the page may be on who answers. A tiny blocking HTTP client is all
//! that takes, and the Helper has no other need of one.

use crate::http::PORT_WALK_LEN;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;
use tracing::info;

/// How long to wait for a port to accept a connection. Loopback answers at
/// once or not at all.
const CONNECT_TIMEOUT: Duration = Duration::from_millis(500);

/// How long to wait for the running Helper to answer.
const REPLY_TIMEOUT: Duration = Duration::from_secs(2);

/// If a Helper is running on the IPC port `ipc_port`, ask it to show its page
/// and say so. Looks on the ports of the UI port walk that starts at
/// `ui_port`, which is where a running Helper serves its page. Anything else
/// that answers on those ports is not a Helper on this IPC port and is left alone.
pub(crate) fn show_running_helper(ipc_port: u16, ui_port: u16) -> bool {
    let last = ui_port.saturating_add(PORT_WALK_LEN - 1);
    for port in ui_port..=last {
        if !answers_for(ipc_port, port) {
            continue;
        }
        info!("A Helper on IPC port {ipc_port} is already running, serving its page on port {port}");
        // If the request fails, the player still gets told it is running.
        let _ = request(port, "POST", "/api/show");
        return true;
    }
    false
}

/// Whether the Helper on `ui_port` says it is on the IPC port `ipc_port`.
fn answers_for(ipc_port: u16, ui_port: u16) -> bool {
    let Some((200, body)) = request(ui_port, "GET", "/api/instance") else {
        return false;
    };
    serde_json::from_str::<serde_json::Value>(&body)
        .is_ok_and(|instance| instance["app"] == crate::APP_NAME && instance["ipc_port"] == ipc_port)
}

/// One request to the page on `port`, with a `Host` the Helper accepts.
/// Returns the status and the body, or None on any failure.
fn request(port: u16, method: &str, path: &str) -> Option<(u16, String)> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT).ok()?;
    stream.set_read_timeout(Some(REPLY_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(REPLY_TIMEOUT)).ok()?;
    // POST routes need a JSON content type; a GET ignores it.
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\
         Content-Type: application/json\r\nContent-Length: 0\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).ok()?;
    let mut raw = Vec::new();
    // A reply cut short by the timeout is still read as far as it got.
    let _ = stream.read_to_end(&mut raw);
    let raw = String::from_utf8_lossy(&raw);
    let (head, body) = raw.split_once("\r\n\r\n")?;
    let status = head.split(' ').nth(1)?.parse().ok()?;
    Some((status, body.to_string()))
}
