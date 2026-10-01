//! Finding a Helper that already runs on the same IPC port, and asking it to
//! make way.
//!
//! There are no lock files: a Helper that finds its IPC port taken asks the
//! ports the page may be on who answers. A tiny blocking HTTP client is all
//! that takes, and the Helper has no other need of one.

use crate::http::PORT_WALK_LEN;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};
use tracing::info;

/// How long to wait for a port to accept a connection. Loopback answers at
/// once or not at all.
const CONNECT_TIMEOUT: Duration = Duration::from_millis(500);

/// How long to wait for the running Helper to answer.
const REPLY_TIMEOUT: Duration = Duration::from_secs(2);

/// What a Helper already running on the IPC port did when asked to make way.
pub(crate) enum Running {
    /// It is quitting. The ports it holds will be free shortly.
    MakingWay,
    /// It stays, and has been asked to show its page: it is in use, or it is
    /// from a release that can't be asked to make way.
    Staying,
}

/// If a Helper is running on the IPC port `ipc_port`, ask it to make way for
/// this one, and say what it did. None when no Helper answers. Looks on the
/// ports of the UI port walk that starts at `ui_port`, which is where a
/// running Helper serves its page. Anything else that answers on those ports
/// is not a Helper on this IPC port and is left alone.
pub(crate) fn ask_running_helper_to_make_way(ipc_port: u16, ui_port: u16) -> Option<Running> {
    let last = ui_port.saturating_add(PORT_WALK_LEN - 1);
    for port in ui_port..=last {
        if !answers_for(ipc_port, port) {
            continue;
        }
        info!("A Helper on IPC port {ipc_port} is already running, serving its page on port {port}");
        return Some(match request(port, "POST", "/api/replace") {
            Some((202, _)) => {
                info!("It is not in use, and is making way");
                Running::MakingWay
            }
            // In use: it has shown its page, with a banner saying why it stays.
            Some((409, _)) => Running::Staying,
            // No answer, or a release without the route: it still gets asked
            // to show its page, and the player gets told it is running.
            _ => {
                let _ = request(port, "POST", "/api/show");
                Running::Staying
            }
        });
    }
    None
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
    stream.set_write_timeout(Some(REPLY_TIMEOUT)).ok()?;
    // POST routes need a JSON content type; a GET ignores it.
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\
         Content-Type: application/json\r\nContent-Length: 0\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).ok()?;
    // One deadline for the whole reply: a program that accepts the connection
    // and then stays silent, or trickles bytes, must not hold up startup.
    let deadline = Instant::now() + REPLY_TIMEOUT;
    let mut raw = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() || stream.set_read_timeout(Some(left)).is_err() {
            break;
        }
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => raw.extend_from_slice(&chunk[..n]),
        }
    }
    let raw = String::from_utf8_lossy(&raw);
    let (head, body) = raw.split_once("\r\n\r\n")?;
    let status = head.split(' ').nth(1)?.parse().ok()?;
    Some((status, body.to_string()))
}
