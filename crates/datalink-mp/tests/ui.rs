//! The running Helper, driven the way a browser drives it: a Helper is started
//! inside the test process on free ports, with a recording stand-in for the
//! browser opener, and an HTTP client talks to its page and JSON API.
//! Assertions are on HTTP responses and status fields only.

mod common;

use common::{http_get, note_transport_unavailable};
use datalink_mp::{Config, Helper, StartError, UiConfig};
use iroh_transport::{Ticket, TransportOptions};
use std::net::{TcpStream, UdpSocket};
use std::sync::{Arc, Mutex, OnceLock};

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// The URLs the browser opener was called with.
type OpenedUrls = Arc<Mutex<Vec<String>>>;

/// A started Helper and what its stand-in browser opener recorded.
struct Started {
    helper: Helper,
    opened: OpenedUrls,
}

impl Started {
    fn ui_port(&self) -> u16 {
        self.helper.ui_port().expect("a Helper started with a UI has a UI port")
    }

    /// GET a token-protected route with the right token.
    fn get_api(&self, path: &str) -> common::HttpResponse {
        http_get(self.ui_port(), path, &[("X-Token", TOKEN)])
    }

    fn status(&self) -> serde_json::Value {
        let response = self.get_api("/api/status");
        assert_eq!(response.status, 200, "status should be served: {}", response.body);
        response.json()
    }
}

/// Start a Helper on free ports. `open_browser` says whether the opener is
/// given to the Helper, the way the absence of `--no-browser` does.
///
/// None when a Transport cannot be created (sandboxed environments); the test
/// then returns early.
fn start(open_browser: bool) -> Option<Started> {
    let opened = OpenedUrls::default();
    let browser_opener = open_browser.then(|| {
        let opened = opened.clone();
        Box::new(move |url: &str| opened.lock().unwrap().push(url.to_string()))
            as datalink_mp::BrowserOpener
    });
    let config = Config {
        ipc_port: 0,
        transport_options: TransportOptions::default(),
        ui: Some(UiConfig {
            port: 0,
            token: TOKEN.to_string(),
            browser_opener,
        }),
    };
    match datalink_mp::start(config) {
        Ok(helper) => Some(Started { helper, opened }),
        Err(StartError::Transport(e)) => {
            note_transport_unavailable(&e);
            None
        }
        Err(e) => panic!("the Helper should start on free ports: {e:?}"),
    }
}

#[test]
fn test_fresh_helper_reports_ready_with_a_ticket_and_its_versions() {
    let Some(started) = start(false) else {
        return;
    };

    let status = started.status();

    assert_eq!(status["state"], "ready");
    let ticket = status["ticket"].as_str().expect("status should carry a Ticket");
    Ticket::parse(ticket).expect("the Ticket in status should parse");
    assert_eq!(status["ticket_seq"], 1);
    assert_eq!(status["release_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(status["ipc_version"], ipc_protocol::PROTOCOL_VERSION);
    assert_eq!(
        status["peer_protocol_version"],
        iroh_transport::STREAM_PROTO_VERSION
    );
    let os = status["os"].as_str().expect("status should carry the OS");
    assert!(["windows", "linux", "macos"].contains(&os), "unexpected OS {os}");
    assert_eq!(status["ipc_port"], started.helper.ipc_port());
    assert_eq!(status["peers"], serde_json::json!([]));
    assert_eq!(status["banners"], serde_json::json!([]));
}

#[test]
fn test_status_needs_the_token() {
    let Some(started) = start(false) else {
        return;
    };
    let port = started.ui_port();

    let missing = http_get(port, "/api/status", &[]);
    let wrong = http_get(port, "/api/status", &[("X-Token", "not-the-token")]);
    let prefix = http_get(port, "/api/status", &[("X-Token", &TOKEN[..TOKEN.len() - 1])]);

    assert_eq!(missing.status, 403);
    assert_eq!(wrong.status, 403);
    assert_eq!(prefix.status, 403);
    assert!(!missing.body.contains(TOKEN) && !wrong.body.contains(TOKEN));
}

#[test]
fn test_page_is_served_without_a_token_and_loads_nothing_from_other_hosts() {
    let Some(started) = start(false) else {
        return;
    };

    let page = http_get(started.ui_port(), "/", &[]);

    assert_eq!(page.status, 200);
    assert!(
        page.header("content-type").is_some_and(|t| t.starts_with("text/html")),
        "the page should be HTML, got {:?}",
        page.header("content-type")
    );
    assert!(page.body.contains("datalink-mp"));
    assert!(!page.body.contains("http://"), "the page should not reference other hosts");
    assert!(!page.body.contains("https://"), "the page should not reference other hosts");
    assert!(!page.body.contains(TOKEN), "the page holds no secrets");
}

#[test]
fn test_browser_opener_is_called_once_with_the_launch_url() {
    let Some(started) = start(true) else {
        return;
    };

    let expected = format!("http://127.0.0.1:{}/?t={TOKEN}", started.ui_port());
    assert_eq!(started.helper.launch_url().as_deref(), Some(expected.as_str()));
    assert_eq!(*started.opened.lock().unwrap(), vec![expected]);
}

#[test]
fn test_browser_opener_is_not_called_without_one() {
    let Some(started) = start(false) else {
        return;
    };

    assert!(started.opened.lock().unwrap().is_empty());
    assert!(started.helper.launch_url().is_some(), "the URL is still there to print");
}

#[test]
fn test_ui_listens_on_loopback_only() {
    let Some(started) = start(false) else {
        return;
    };

    // Ask the OS which address it would use to reach the outside; UDP connect
    // sends nothing. Without a network there is no other address to try.
    let Some(other_ip) = UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| s.connect("192.0.2.1:9").and_then(|_| s.local_addr()))
        .map(|a| a.ip())
        .ok()
        .filter(|ip| !ip.is_loopback() && !ip.is_unspecified())
    else {
        eprintln!("No non-loopback address on this machine; nothing to check against");
        return;
    };

    assert!(
        TcpStream::connect((other_ip, started.ui_port())).is_err(),
        "the UI should not be reachable on {other_ip}"
    );
}

#[test]
fn test_helper_without_a_ui_serves_no_page() {
    let config = Config {
        ipc_port: 0,
        transport_options: TransportOptions::default(),
        ui: None,
    };
    let helper = match datalink_mp::start(config) {
        Ok(helper) => helper,
        Err(StartError::Transport(e)) => return note_transport_unavailable(&e),
        Err(e) => panic!("the Helper should start on a free IPC port: {e:?}"),
    };

    assert_eq!(helper.ui_port(), None);
    assert_eq!(helper.launch_url(), None);
}

#[test]
fn test_start_fails_with_a_ui_bind_error_when_the_ui_port_is_taken() {
    let (_holder, taken_port) = common::hold_port();
    let config = Config {
        ipc_port: 0,
        transport_options: TransportOptions::default(),
        ui: Some(UiConfig {
            port: taken_port,
            token: TOKEN.to_string(),
            browser_opener: None,
        }),
    };

    match datalink_mp::start(config) {
        Err(StartError::UiBind { port, .. }) => assert_eq!(port, taken_port),
        Err(StartError::Transport(e)) => note_transport_unavailable(&e),
        other => panic!("start should fail while another program holds the UI port: {:?}", other.err()),
    }
}

/// Log output at info level and above, captured process-wide.
fn captured_logs() -> Arc<Mutex<Vec<u8>>> {
    static LOGS: OnceLock<Arc<Mutex<Vec<u8>>>> = OnceLock::new();
    LOGS.get_or_init(|| {
        #[derive(Clone)]
        struct Sink(Arc<Mutex<Vec<u8>>>);
        impl std::io::Write for Sink {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let logs = Arc::new(Mutex::new(Vec::new()));
        let sink = Sink(logs.clone());
        let _ = tracing::subscriber::set_global_default(
            tracing_subscriber::fmt()
                .with_max_level(tracing::Level::INFO)
                .with_writer(move || sink.clone())
                .finish(),
        );
        logs
    })
    .clone()
}

#[test]
fn test_token_is_not_logged_at_info_level_or_above() {
    let logs = captured_logs();
    let Some(started) = start(true) else {
        return;
    };
    started.status();
    http_get(started.ui_port(), "/api/status", &[("X-Token", "wrong")]);
    drop(started);

    let logged = String::from_utf8_lossy(&logs.lock().unwrap()).into_owned();
    assert!(!logged.contains(TOKEN), "the token was logged: {logged}");
}
