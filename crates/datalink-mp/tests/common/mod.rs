//! Helpers shared by the Helper's tests.
//!
//! Each test file compiles this module separately and uses only part of it.
#![allow(dead_code)]

use ipc_protocol::{
    decode_response, encode_request, read_message, IpcRequest, IpcResponse, PROTOCOL_VERSION,
};
use std::io::Write;
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

    /// Handshake with the IPC version this build of the DLL would send.
    pub fn handshake(&mut self) -> IpcResponse {
        self.handshake_with_version(PROTOCOL_VERSION)
    }

    /// Handshake as a DLL from another build would, with its own IPC version.
    pub fn handshake_with_version(&mut self, ipc_version: u32) -> IpcResponse {
        self.request(&IpcRequest::Handshake {
            protocol_version: ipc_version,
        })
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

/// A loopback port that was free a moment ago, for a Helper started as a
/// separate process. In-process tests pass port 0 and read the bound port back.
pub fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("should bind a loopback port")
        .local_addr()
        .expect("bound listener should have an address")
        .port()
}
