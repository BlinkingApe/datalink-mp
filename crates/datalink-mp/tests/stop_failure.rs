//! Stop when the new Transport cannot be created: Stop reports the error and
//! nothing changes.
//!
//! A Transport cannot be created once the process may open no more files: it
//! needs file descriptors for its runtime and its sockets. This test takes
//! that away from the whole process for the length of one request, so it is
//! a test binary of its own: in a binary with other tests, theirs would fail
//! with it.
#![cfg(unix)]

mod common;

use common::{note_transport_unavailable, FakeDll};
use datalink_mp::{Config, StartError, UiConfig};
use datalink_transport::{Ticket, Transport, TransportOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::os::fd::AsRawFd;

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// How long the friend and the Helper may take to see each other connect.
const PEER_NOTICE_DEADLINE: std::time::Duration = std::time::Duration::from_secs(10);

/// An HTTP connection to the page's server that stays open between requests,
/// so that a request can be sent at a time the process can open no new
/// connection.
struct KeptConnection {
    reader: BufReader<TcpStream>,
    ui_port: u16,
}

impl KeptConnection {
    fn open(ui_port: u16) -> Self {
        let stream = TcpStream::connect(("127.0.0.1", ui_port)).expect("the UI should accept connections");
        stream.set_read_timeout(Some(std::time::Duration::from_secs(30))).unwrap();
        Self { reader: BufReader::new(stream), ui_port }
    }

    /// Send one request with the token, the way the page does, and read the
    /// reply's status and body. The connection stays open.
    fn request(&mut self, method: &str, path: &str) -> (u16, String) {
        let request = format!(
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nX-Token: {TOKEN}\r\n\
             Content-Type: application/json\r\nContent-Length: 2\r\n\r\n{{}}",
            self.ui_port
        );
        self.reader.get_mut().write_all(request.as_bytes()).expect("request should be written");

        let mut status_line = String::new();
        self.reader.read_line(&mut status_line).expect("the reply should have a status line");
        let status = status_line
            .split(' ')
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| panic!("the reply should have a status: {status_line:?}"));
        let mut length = 0;
        loop {
            let mut line = String::new();
            self.reader.read_line(&mut line).expect("the reply's head should be read");
            if line == "\r\n" {
                break;
            }
            if let Some((name, value)) = line.split_once(':') {
                if name.eq_ignore_ascii_case("content-length") {
                    length = value.trim().parse().expect("Content-Length should be a number");
                }
            }
        }
        let mut body = vec![0; length];
        self.reader.read_exact(&mut body).expect("the reply's body should be read");
        (status, String::from_utf8_lossy(&body).into_owned())
    }

    fn status(&mut self) -> serde_json::Value {
        let (status, body) = self.request("GET", "/api/status");
        assert_eq!(status, 200, "status should be served: {body}");
        serde_json::from_str(&body).expect("status should be JSON")
    }
}

/// While kept, the process can open no new file descriptor: each one it has
/// stays open, but every open, socket or dup fails with EMFILE.
struct NoNewFiles {
    restore: libc::rlimit,
}

impl NoNewFiles {
    /// `fd` is any descriptor the process has open.
    fn take(fd: i32) -> Self {
        let mut restore = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
        // SAFETY: `restore` is a valid `rlimit` for `getrlimit` to write into.
        assert_eq!(unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut restore) }, 0);
        let kept = Self { restore };
        // A new descriptor takes the lowest free number. With the limit at
        // that number, none is left below it. Another thread may close one
        // below it meanwhile: then the limit is set again, lower.
        for _ in 0..10 {
            // SAFETY: `fd` is a descriptor the process has open, valid for `dup`.
            let lowest_free = unsafe { libc::dup(fd) };
            if lowest_free < 0 {
                return kept;
            }
            // SAFETY: `lowest_free` was just returned by `dup` above, so it names
            // an open descriptor this call closes once, right here.
            unsafe { libc::close(lowest_free) };
            let limit = libc::rlimit { rlim_cur: lowest_free as libc::rlim_t, rlim_max: restore.rlim_max };
            // SAFETY: `limit` is a valid `rlimit` for `setrlimit` to read.
            assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &limit) }, 0);
        }
        panic!("could not keep the process from opening files");
    }
}

impl Drop for NoNewFiles {
    fn drop(&mut self) {
        // SAFETY: `self.restore` is the valid `rlimit` `take` read back before
        // lowering it, for `setrlimit` to read here.
        unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &self.restore) };
    }
}

#[test]
fn test_stop_that_cannot_create_the_new_transport_reports_it_and_changes_nothing() {
    let config = Config {
        game_folder: std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")),
        ipc_port: 0,
        transport_options: TransportOptions::default(),
        ui: Some(UiConfig { port: 0, token: TOKEN.to_string(), browser_opener: None }),
    };
    let helper = match datalink_mp::start(config) {
        Ok(helper) => helper,
        Err(StartError::Transport(e)) => return note_transport_unavailable(&e),
        Err(e) => panic!("the Helper should start on free ports: {e:?}"),
    };
    let friend = match Transport::new() {
        Ok(friend) => friend,
        Err(e) => return note_transport_unavailable(&e),
    };
    let mut page = KeptConnection::open(helper.ui_port().unwrap());
    let before = page.status();
    let ticket = before["ticket"].as_str().unwrap().to_string();
    friend.connect_to_peer(&ticket).expect("the friend should reach the Helper on loopback");
    let helper_id = Ticket::parse(&ticket).unwrap().addr().id;
    common::poll_until(PEER_NOTICE_DEADLINE, || {
        (page.status()["state"] == "hosting" && friend.connected_peers().contains(&helper_id)).then_some(())
    })
    .expect("the friend and the Helper should be connected");
    let mut dll = FakeDll::connect(helper.ipc_port()).expect("the Helper should accept a DLL connection");
    dll.handshake();

    let (stop_status, stop_body) = {
        let _no_new_files = NoNewFiles::take(page.reader.get_ref().as_raw_fd());
        page.request("POST", "/api/stop")
    };

    assert_eq!(stop_status, 500, "Stop should report that it failed: {stop_body}");
    let error: serde_json::Value = serde_json::from_str(&stop_body).expect("the error should be JSON");
    assert!(
        error["error"].as_str().is_some_and(|e| e.contains("Transport")),
        "Stop should say that the new Transport could not be created: {stop_body}"
    );
    let after = page.status();
    assert_eq!(after["ticket"], ticket.as_str(), "the Ticket should be unchanged");
    assert_eq!(after["ticket_seq"], 1, "the sequence number should be unchanged");
    assert_eq!(after["state"], "hosting");
    assert_eq!(after["peers"], before_peers(&friend));
    assert_eq!(after["game_connected"], true);
    assert!(friend.connected_peers().contains(&helper_id), "the friend should still be connected");
    let answer = dll.request(&datalink_ipc::IpcRequest::GetOurTicket);
    assert!(
        matches!(&answer, datalink_ipc::IpcResponse::StringValue { value } if *value == ticket),
        "the game should still be answered by the same Transport, got {answer:?}"
    );
}

/// The peer list status shows with `friend` the only Helper connected.
fn before_peers(friend: &Transport) -> serde_json::Value {
    serde_json::json!([friend.endpoint_id().fmt_short().to_string()])
}
