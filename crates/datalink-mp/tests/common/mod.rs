//! Test support shared by the tests of the Helper.
//!
//! Each test file compiles this module separately and uses only part of it.
#![allow(dead_code)]

use ipc_protocol::{
    decode_response, encode_request, read_message, IpcRequest, IpcResponse, PROTOCOL_VERSION,
};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

/// How long the Helper may take to answer one IPC request.
const REPLY_DEADLINE: Duration = Duration::from_secs(10);

/// A fake DLL: a TCP client that speaks the IPC protocol to a running Helper,
/// the way the game's `dplayx.dll` does.
pub struct FakeDll {
    stream: TcpStream,
}

impl FakeDll {
    /// Connect to the Helper's IPC port. Sends nothing yet.
    pub fn connect(ipc_port: u16) -> std::io::Result<Self> {
        let stream = TcpStream::connect(("127.0.0.1", ipc_port))?;
        stream.set_read_timeout(Some(REPLY_DEADLINE))?;
        stream.set_write_timeout(Some(REPLY_DEADLINE))?;
        stream.set_nodelay(true)?;
        Ok(Self { stream })
    }

    /// Send one request and wait for the Helper's reply.
    pub fn request(&mut self, request: &IpcRequest) -> IpcResponse {
        let encoded = encode_request(request).expect("request should encode");
        self.stream
            .write_all(&encoded)
            .expect("request should be written to the Helper");
        let reply = read_message(&mut self.stream).expect("the Helper should reply");
        decode_response(&reply).expect("the Helper's reply should decode")
    }

    /// Handshake as a DLL from another build would, with its own IPC version.
    pub fn handshake_with_version(&mut self, ipc_version: u32) -> IpcResponse {
        self.request(&IpcRequest::Handshake {
            protocol_version: ipc_version,
        })
    }

    /// Handshake with this build's IPC version, and return the endpoint ID and
    /// the Ticket the Helper answers with. Panics on any other reply.
    pub fn handshake(&mut self) -> ([u8; 32], String) {
        match self.handshake_with_version(PROTOCOL_VERSION) {
            IpcResponse::HandshakeOk {
                endpoint_id,
                our_ticket,
            } => (endpoint_id, our_ticket),
            other => panic!("expected HandshakeOk, got {other:?}"),
        }
    }
}

/// Poll until `cond` returns Some or the deadline passes.
pub fn poll_until<T>(timeout: Duration, mut cond: impl FnMut() -> Option<T>) -> Option<T> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if let Some(v) = cond() {
            return Some(v);
        }
        if std::time::Instant::now() > deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Take a loopback port the way another program would. The port stays taken
/// for as long as the listener is kept.
pub fn hold_port() -> (TcpListener, u16) {
    let holder = TcpListener::bind("127.0.0.1:0").expect("should bind a loopback port");
    let port = holder
        .local_addr()
        .expect("bound listener should have an address")
        .port();
    (holder, port)
}

/// A loopback port that was free a moment ago, for a Helper started as a
/// separate process. In-process tests pass port 0 and read the bound port back.
pub fn free_port() -> u16 {
    hold_port().1
}

/// Say why a test returns early: no Transport could be created, which is
/// expected in sandboxed environments.
pub fn note_transport_unavailable(detail: &dyn std::fmt::Debug) {
    eprintln!("Transport creation failed (expected in sandboxed environments): {detail:?}");
}

/// What an HTTP client sees of one response.
pub struct HttpResponse {
    pub status: u16,
    /// Header names lowercased.
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl HttpResponse {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// The body parsed as JSON.
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body)
            .unwrap_or_else(|e| panic!("body should be JSON ({e}): {}", self.body))
    }
}

/// GET `path` from the UI on `ui_port`, with the given extra headers.
pub fn http_get(ui_port: u16, path: &str, headers: &[(&str, &str)]) -> HttpResponse {
    http_request(ui_port, "GET", path, headers)
}

/// Send one HTTP/1.1 request to the UI on `ui_port` and read the whole reply.
///
/// A raw client on purpose: tests need to control headers such as `Host`.
pub fn http_request(
    ui_port: u16,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
) -> HttpResponse {
    let mut stream = TcpStream::connect(("127.0.0.1", ui_port)).expect("the UI should accept connections");
    stream.set_read_timeout(Some(REPLY_DEADLINE)).unwrap();
    stream.set_write_timeout(Some(REPLY_DEADLINE)).unwrap();

    let mut request = format!("{method} {path} HTTP/1.1\r\nConnection: close\r\n");
    if !headers.iter().any(|(n, _)| n.eq_ignore_ascii_case("host")) {
        request.push_str(&format!("Host: 127.0.0.1:{ui_port}\r\n"));
    }
    for (name, value) in headers {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    request.push_str("\r\n");
    stream.write_all(request.as_bytes()).expect("request should be written");

    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).expect("response should be read to the end");
    let raw = String::from_utf8_lossy(&raw).into_owned();

    let (head, body) = raw.split_once("\r\n\r\n").expect("response should have a head");
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .and_then(|l| l.split(' ').nth(1))
        .and_then(|s| s.parse().ok())
        .expect("response should have a status line");
    let headers = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(n, v)| (n.trim().to_ascii_lowercase(), v.trim().to_string()))
        .collect::<Vec<_>>();
    let body = if headers.iter().any(|(n, v)| n == "transfer-encoding" && v == "chunked") {
        dechunk(body)
    } else {
        body.to_string()
    };
    HttpResponse { status, headers, body }
}

fn dechunk(mut rest: &str) -> String {
    let mut out = String::new();
    while let Some((size, after)) = rest.split_once("\r\n") {
        let size = usize::from_str_radix(size.trim(), 16).expect("chunk size");
        if size == 0 {
            break;
        }
        out.push_str(&after[..size]);
        rest = &after[size + 2..];
    }
    out
}
