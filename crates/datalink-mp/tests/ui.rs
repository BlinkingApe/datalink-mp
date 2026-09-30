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

// ---------------------------------------------------------------------------
// Ticket 07: HTTP security hardening.
//
// The rules hold for every request, whatever the route, so they are checked
// on the page, on the API, on unknown paths and on methods no route takes.
// ---------------------------------------------------------------------------

#[test]
fn test_request_with_another_host_name_is_refused() {
    let Some(started) = start(false) else {
        return;
    };
    let port = started.ui_port();
    let host = format!("rebind.example:{port}");

    let page = http_get(port, "/", &[("Host", &host)]);
    let status = http_get(port, "/api/status", &[("Host", &host), ("X-Token", TOKEN)]);

    assert_eq!(page.status, 403);
    assert_eq!(status.status, 403);
    assert!(!status.body.contains("ticket"), "a refused request reveals nothing");
}

#[test]
fn test_request_with_the_right_host_name_and_another_port_is_refused() {
    let Some(started) = start(false) else {
        return;
    };
    let port = started.ui_port();
    let other = port.wrapping_add(1);

    for host in [format!("127.0.0.1:{other}"), format!("localhost:{other}"), "127.0.0.1".into()] {
        let page = http_get(port, "/", &[("Host", &host)]);
        assert_eq!(page.status, 403, "Host {host} should be refused");
    }
}

#[test]
fn test_both_loopback_names_with_the_bound_port_are_accepted() {
    let Some(started) = start(false) else {
        return;
    };
    let port = started.ui_port();

    // Host names are case-insensitive; browsers send them lowercased anyway.
    for host in [format!("127.0.0.1:{port}"), format!("localhost:{port}"), format!("LocalHost:{port}")] {
        let page = http_get(port, "/", &[("Host", &host)]);
        let status = http_get(port, "/api/status", &[("Host", &host), ("X-Token", TOKEN)]);
        assert_eq!(page.status, 200, "Host {host} should be accepted on the page");
        assert_eq!(status.status, 200, "Host {host} should be accepted on the API");
    }
}

#[test]
fn test_host_names_that_only_look_like_ours_are_refused() {
    let Some(started) = start(false) else {
        return;
    };
    let port = started.ui_port();

    for host in [
        format!("localhost.:{port}"),
        format!("127.0.0.1.:{port}"),
        format!("localhost.rebind.example:{port}"),
        format!("rebind.example:{port}@127.0.0.1:{port}"),
        format!("127.0.0.1:0{port}"),
        format!("127.0.0.1:{port}:{port}"),
        format!("[::1]:{port}"),
        format!("127.1:{port}"),
        String::new(),
    ] {
        let page = http_get(port, "/", &[("Host", &host)]);
        assert_eq!(page.status, 403, "Host {host:?} should be refused");
    }
}

#[test]
fn test_request_without_exactly_one_host_is_refused() {
    let Some(started) = start(false) else {
        return;
    };
    let port = started.ui_port();
    let ours = format!("127.0.0.1:{port}");

    let missing = send_raw(port, "GET / HTTP/1.1\r\nConnection: close\r\n\r\n");
    let missing_old = send_raw(port, "GET / HTTP/1.0\r\n\r\n");
    let twice_ours = http_get(port, "/", &[("Host", &ours), ("Host", &ours)]);
    let ours_then_foreign = http_get(port, "/", &[("Host", &ours), ("Host", "rebind.example")]);

    assert_eq!(status_of(&missing), 403, "{missing}");
    assert_eq!(status_of(&missing_old), 403, "{missing_old}");
    assert_eq!(twice_ours.status, 403);
    assert_eq!(ours_then_foreign.status, 403);
}

#[test]
fn test_absolute_request_target_naming_another_host_is_refused() {
    let Some(started) = start(false) else {
        return;
    };
    let port = started.ui_port();
    let absolute = |authority: &str| {
        format!(
            "GET http://{authority}/api/status HTTP/1.1\r\n\
             Host: 127.0.0.1:{port}\r\nX-Token: {TOKEN}\r\nConnection: close\r\n\r\n"
        )
    };

    let foreign = send_raw(port, &absolute(&format!("rebind.example:{port}")));
    let ours = send_raw(port, &absolute(&format!("localhost:{port}")));

    assert_eq!(status_of(&foreign), 403, "{foreign}");
    assert_eq!(status_of(&ours), 200, "{ours}");
}

/// POST to `path` with the right Host, the token and a JSON content type, plus
/// `extra` headers. No POST route exists yet, so the tests aim at a GET route:
/// a POST that passes the rules reaches routing and gets 405 there, and one
/// that breaks them gets the rule's own status instead.
fn post(started: &Started, path: &str, extra: &[(&str, &str)]) -> common::HttpResponse {
    let mut headers = vec![("X-Token", TOKEN), ("Content-Type", "application/json")];
    headers.extend_from_slice(extra);
    common::http_request(started.ui_port(), "POST", path, &headers)
}

#[test]
fn test_post_with_a_foreign_origin_is_refused() {
    let Some(started) = start(false) else {
        return;
    };
    let port = started.ui_port();

    for origin in [
        "http://rebind.example".to_string(),
        format!("http://rebind.example:{port}"),
        format!("http://127.0.0.1:{}", port.wrapping_add(1)),
        format!("https://127.0.0.1:{port}"),
        format!("http://localhost.:{port}"),
        "null".to_string(),
        String::new(),
    ] {
        let response = post(&started, "/api/status", &[("Origin", &origin)]);
        assert_eq!(response.status, 403, "Origin {origin:?} should be refused");
    }
    let twice = format!("http://127.0.0.1:{port}");
    let response = post(&started, "/api/status", &[("Origin", &twice), ("Origin", &twice)]);
    assert_eq!(response.status, 403, "two Origins should be refused");
}

#[test]
fn test_post_with_no_origin_or_our_own_passes_the_origin_check() {
    let Some(started) = start(false) else {
        return;
    };
    let port = started.ui_port();

    let none = post(&started, "/api/status", &[]);
    assert_eq!(none.status, 405, "no Origin should reach routing");
    for origin in [format!("http://127.0.0.1:{port}"), format!("http://localhost:{port}")] {
        let response = post(&started, "/api/status", &[("Origin", &origin)]);
        assert_eq!(response.status, 405, "Origin {origin} should reach routing");
    }
}

#[test]
fn test_post_without_a_json_content_type_is_refused() {
    let Some(started) = start(false) else {
        return;
    };
    let port = started.ui_port();
    let with_type = |content_type: &str| {
        common::http_request(
            port,
            "POST",
            "/api/status",
            &[("X-Token", TOKEN), ("Content-Type", content_type)],
        )
    };

    let missing = common::http_request(port, "POST", "/api/status", &[("X-Token", TOKEN)]);
    assert_eq!(missing.status, 415, "a POST with no Content-Type should be refused");
    // The types a cross-site form can send without asking first, and look-alikes.
    for content_type in [
        "text/plain",
        "application/x-www-form-urlencoded",
        "multipart/form-data; boundary=x",
        "application/jsonx",
        "application/json-patch+json",
        "text/plain; application/json",
        "",
    ] {
        assert_eq!(
            with_type(content_type).status,
            415,
            "Content-Type {content_type:?} should be refused"
        );
    }
    let twice = common::http_request(
        port,
        "POST",
        "/api/status",
        &[
            ("X-Token", TOKEN),
            ("Content-Type", "application/json"),
            ("Content-Type", "text/plain"),
        ],
    );
    assert_eq!(twice.status, 415, "two Content-Types should be refused");
}

#[test]
fn test_post_with_a_json_content_type_passes_the_content_type_check() {
    let Some(started) = start(false) else {
        return;
    };
    let port = started.ui_port();

    for content_type in ["application/json", "application/json; charset=utf-8", "Application/JSON"] {
        let response = common::http_request(
            port,
            "POST",
            "/api/status",
            &[("X-Token", TOKEN), ("Content-Type", content_type)],
        );
        assert_eq!(response.status, 405, "Content-Type {content_type:?} should reach routing");
    }
}

/// One response of every kind the server gives: the page, the API, and each
/// way of being refused or not found.
fn responses_of_every_kind(started: &Started) -> Vec<(&'static str, common::HttpResponse)> {
    let port = started.ui_port();
    let request = |method: &str, path: &str, headers: &[(&str, &str)]| {
        common::http_request(port, method, path, headers)
    };
    let preflight = [
        ("Origin", "http://rebind.example"),
        ("Access-Control-Request-Method", "POST"),
        ("Access-Control-Request-Headers", "content-type, x-token"),
    ];
    let own_origin = format!("http://127.0.0.1:{port}");
    let own_preflight = [
        ("Origin", own_origin.as_str()),
        ("Access-Control-Request-Method", "POST"),
        ("Access-Control-Request-Headers", "content-type, x-token"),
    ];
    let foreign_host = format!("rebind.example:{port}");
    let with_token = [("X-Token", TOKEN)];
    vec![
        ("page", request("GET", "/", &[])),
        ("page by HEAD", request("HEAD", "/", &[])),
        ("status", request("GET", "/api/status", &with_token)),
        ("status without a token", request("GET", "/api/status", &[])),
        ("foreign Host", request("GET", "/", &[("Host", &foreign_host)])),
        ("foreign Origin", request("GET", "/api/status", &[("X-Token", TOKEN), ("Origin", "null")])),
        ("unknown path", request("GET", "/nope", &with_token)),
        ("unknown API path", request("GET", "/api/nope", &with_token)),
        ("POST on a GET route", post(started, "/api/status", &[])),
        ("POST that is not JSON", request("POST", "/api/status", &with_token)),
        ("foreign preflight", request("OPTIONS", "/api/status", &preflight)),
        ("own-origin preflight", request("OPTIONS", "/api/status", &own_preflight)),
        ("preflight on the page", request("OPTIONS", "/", &own_preflight)),
        ("OPTIONS *", request("OPTIONS", "*", &[])),
        ("PUT", request("PUT", "/api/status", &with_token)),
        ("DELETE", request("DELETE", "/", &[])),
    ]
}

#[test]
fn test_no_response_carries_cors_headers_and_no_preflight_succeeds() {
    let Some(started) = start(false) else {
        return;
    };

    for (kind, response) in responses_of_every_kind(&started) {
        let cors = response
            .headers
            .iter()
            .filter(|(name, _)| name.starts_with("access-control-"))
            .collect::<Vec<_>>();
        assert!(cors.is_empty(), "{kind}: CORS headers sent: {cors:?}");
    }
    let port = started.ui_port();
    for path in ["/", "/api/status"] {
        let preflight = common::http_request(port, "OPTIONS", path, &[("X-Token", TOKEN)]);
        assert!(
            preflight.status >= 400,
            "OPTIONS {path} should not succeed, got {}",
            preflight.status
        );
    }
}

#[test]
fn test_every_response_carries_the_security_headers() {
    let Some(started) = start(false) else {
        return;
    };

    for (kind, response) in responses_of_every_kind(&started) {
        let only = |name: &str| {
            let values = response
                .headers
                .iter()
                .filter(|(n, _)| n == name)
                .map(|(_, v)| v.as_str())
                .collect::<Vec<_>>();
            assert_eq!(values.len(), 1, "{kind} ({}): {name} should appear once: {values:?}", response.status);
            values[0].to_string()
        };
        assert_eq!(only("cache-control"), "no-store", "{kind}");
        assert_eq!(only("x-content-type-options"), "nosniff", "{kind}");
        assert_eq!(only("referrer-policy"), "no-referrer", "{kind}");
        assert_csp_is_strict(kind, &only("content-security-policy"));
    }
}

/// The page's own inline script and style, connections to itself, no framing,
/// and nothing else.
fn assert_csp_is_strict(kind: &str, csp: &str) {
    let directives = csp
        .split(';')
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .map(|d| {
            let mut words = d.split_ascii_whitespace();
            let name = words.next().unwrap().to_ascii_lowercase();
            (name, words.collect::<Vec<_>>())
        })
        .collect::<std::collections::HashMap<_, _>>();
    let sources = |name: &str| {
        directives
            .get(name)
            .unwrap_or_else(|| panic!("{kind}: CSP {csp:?} has no {name}"))
            .clone()
    };

    assert_eq!(sources("default-src"), ["'none'"], "{kind}: {csp}");
    assert_eq!(sources("connect-src"), ["'self'"], "{kind}: {csp}");
    assert_eq!(sources("frame-ancestors"), ["'none'"], "{kind}: {csp}");
    assert_eq!(sources("base-uri"), ["'none'"], "{kind}: {csp}");
    assert_eq!(sources("form-action"), ["'none'"], "{kind}: {csp}");
    assert_eq!(sources("style-src"), ["'unsafe-inline'"], "{kind}: {csp}");
    // Scripts: only the page's own, named by hash. No inline script an
    // attacker could inject, no script from anywhere, and no eval.
    let scripts = sources("script-src");
    assert!(!scripts.is_empty(), "{kind}: {csp}");
    assert!(
        scripts.iter().all(|s| s.starts_with("'sha256-") && s.ends_with('\'')),
        "{kind}: script-src should list only hashes: {csp}"
    );
    for name in directives.keys() {
        assert!(
            [
                "default-src",
                "connect-src",
                "frame-ancestors",
                "base-uri",
                "form-action",
                "style-src",
                "script-src"
            ]
            .contains(&name.as_str()),
            "{kind}: unexpected directive {name} in {csp}"
        );
    }
}

/// Send `raw` as it is, for requests the test client cannot express, and
/// return the whole reply.
fn send_raw(ui_port: u16, raw: &str) -> String {
    use std::io::{Read, Write};
    let mut stream =
        TcpStream::connect(("127.0.0.1", ui_port)).expect("the UI should accept connections");
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .unwrap();
    stream.write_all(raw.as_bytes()).expect("request should be written");
    let mut reply = Vec::new();
    stream.read_to_end(&mut reply).expect("reply should be read to the end");
    String::from_utf8_lossy(&reply).into_owned()
}

/// The status code of a raw reply.
fn status_of(reply: &str) -> u16 {
    reply
        .split(' ')
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| panic!("reply should have a status line: {reply:?}"))
}

// ---------------------------------------------------------------------------
// Ticket 06: startup, UI port walk, browser opening
// ---------------------------------------------------------------------------

/// Hold `count` consecutive loopback ports, the way other programs would.
/// The ports stay taken for as long as the listeners are kept.
fn hold_port_range(count: u16) -> Vec<std::net::TcpListener> {
    loop {
        let (first_holder, first) = common::hold_port();
        let mut held = vec![first_holder];
        for port in first.saturating_add(1)..first.saturating_add(count) {
            match std::net::TcpListener::bind(("127.0.0.1", port)) {
                Ok(holder) => held.push(holder),
                Err(_) => break,
            }
        }
        if held.len() == usize::from(count) {
            return held;
        }
    }
}

/// A Helper configured to serve the page from `port` upwards.
fn ui_config_from(port: u16) -> Config {
    Config {
        ipc_port: 0,
        transport_options: TransportOptions::default(),
        ui: Some(UiConfig {
            port,
            token: TOKEN.to_string(),
            browser_opener: None,
        }),
    }
}

#[test]
fn test_taken_ui_port_makes_the_helper_bind_the_next_one() {
    let mut held = hold_port_range(10);
    let taken = held[0].local_addr().unwrap().port();
    // The next port is free; the eight after it are not.
    drop(held.remove(1));

    let helper = match datalink_mp::start(ui_config_from(taken)) {
        Ok(helper) => helper,
        Err(StartError::Transport(e)) => return note_transport_unavailable(&e),
        Err(e) => panic!("the Helper should walk past a taken UI port: {e:?}"),
    };

    assert_eq!(helper.ui_port(), Some(taken + 1));
    let expected = format!("http://127.0.0.1:{}/?t={TOKEN}", taken + 1);
    assert_eq!(helper.launch_url().as_deref(), Some(expected.as_str()));
    assert_eq!(
        http_get(taken + 1, "/", &[]).status,
        200,
        "the page is served on the port the handle reports"
    );
}

#[test]
fn test_start_fails_with_a_clear_message_when_all_ten_ui_ports_are_taken() {
    let held = hold_port_range(10);
    let first = held[0].local_addr().unwrap().port();

    let error = match datalink_mp::start(ui_config_from(first)) {
        Err(StartError::Transport(e)) => return note_transport_unavailable(&e),
        Err(e) => e,
        Ok(_) => panic!("the Helper should not start with every UI port taken"),
    };

    let message = error.to_string();
    assert!(
        message.contains(&first.to_string()) && message.contains(&(first + 9).to_string()),
        "the message should name the ports that were tried: {message}"
    );
    assert!(message.contains("--ui-port"), "the message should say what to do: {message}");
}

/// A `datalink-mp` process in UI mode.
struct UiProcess {
    child: std::process::Child,
    stdout_lines: std::sync::mpsc::Receiver<String>,
}

impl UiProcess {
    /// Start the binary with `args`, `envs` set and `PATH` replaced by `path`,
    /// so the test decides which programs the Helper can find.
    fn spawn(args: &[String], envs: &[(&str, &str)], path: &std::path::Path) -> Self {
        use std::io::{BufRead, BufReader};
        use std::process::{Command, Stdio};
        let mut child = Command::new(env!("CARGO_BIN_EXE_datalink-mp"))
            .args(args)
            // The developer's own settings must not leak into the test.
            .env_remove("SMAC_HELPER_PORT")
            .env_remove("SMAC_HELPER_LOG_FILE")
            .env_remove("SMAC_UI_PORT")
            .env("PATH", path)
            .envs(envs.iter().copied())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the datalink-mp binary should start");
        let stdout = child.stdout.take().expect("stdout is piped");
        let (tx, stdout_lines) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        Self { child, stdout_lines }
    }

    /// The launch URL, printed on standard output. None when the Helper exited
    /// first because no Transport could be created (sandboxed environments).
    fn launch_url(&mut self) -> Option<String> {
        loop {
            match self.stdout_lines.recv_timeout(std::time::Duration::from_secs(30)) {
                Ok(line) if line.starts_with("http://") => return Some(line),
                Ok(_) => {}
                Err(_) => {
                    let exit = self.wait_for_exit();
                    if exit.stderr.contains("Failed to create transport") {
                        note_transport_unavailable(&exit.stderr);
                        return None;
                    }
                    panic!("the Helper printed no launch URL and exited with {:?}: {}", exit.status, exit.stderr);
                }
            }
        }
    }

    /// Wait for the process to end by itself and collect standard error.
    fn wait_for_exit(&mut self) -> UiExit {
        let status = common::poll_until(std::time::Duration::from_secs(30), || {
            self.child.try_wait().expect("should poll the process")
        })
        .expect("the Helper should have exited");
        let mut stderr = String::new();
        if let Some(mut pipe) = self.child.stderr.take() {
            let _ = std::io::Read::read_to_string(&mut pipe, &mut stderr);
        }
        UiExit { status, stderr }
    }

    #[cfg(target_os = "linux")]
    fn is_running(&mut self) -> bool {
        self.child.try_wait().expect("should poll the process").is_none()
    }
}

struct UiExit {
    status: std::process::ExitStatus,
    stderr: String,
}

impl Drop for UiProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn port_of(launch_url: &str) -> u16 {
    launch_url
        .strip_prefix("http://127.0.0.1:")
        .and_then(|rest| rest.split_once("/?t="))
        .and_then(|(port, _)| port.parse().ok())
        .unwrap_or_else(|| panic!("not a launch URL: {launch_url}"))
}

/// The UI-mode command line for the binary, with the IPC port picked by the OS.
fn ui_args_from(ui_port: u16, no_browser: bool) -> Vec<String> {
    let mut args = vec!["--ui-port".to_string(), ui_port.to_string(), "--port".into(), "0".into()];
    if no_browser {
        args.push("--no-browser".into());
    }
    args
}

/// A fresh empty directory for a test to put files in.
#[cfg(target_os = "linux")]
fn scratch_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("datalink-mp-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("should create a scratch directory");
    dir
}

/// Hold ten consecutive ports, free the second, and return the holders and the first port.
fn ports_with_only_the_second_free() -> (Vec<std::net::TcpListener>, u16) {
    let mut held = hold_port_range(10);
    let first = held[0].local_addr().unwrap().port();
    drop(held.remove(1));
    (held, first)
}

#[test]
fn test_ui_port_flag_is_where_the_walk_starts() {
    let (_held, taken) = ports_with_only_the_second_free();
    let mut helper = UiProcess::spawn(&ui_args_from(taken, true), &[], std::path::Path::new(""));

    let Some(url) = helper.launch_url() else {
        return;
    };

    assert_eq!(port_of(&url), taken + 1);
}

#[test]
fn test_smac_ui_port_is_where_the_walk_starts() {
    let (_held, taken) = ports_with_only_the_second_free();
    // No --ui-port flag, so that the variable is what names the port.
    let args = vec!["--port".to_string(), "0".to_string(), "--no-browser".to_string()];
    let mut helper = UiProcess::spawn(&args, &[("SMAC_UI_PORT", &taken.to_string())], std::path::Path::new(""));

    let Some(url) = helper.launch_url() else {
        return;
    };

    assert_eq!(port_of(&url), taken + 1);
}

#[test]
fn test_ui_mode_exits_with_the_walk_message_when_all_ten_ui_ports_are_taken() {
    let held = hold_port_range(10);
    let first = held[0].local_addr().unwrap().port();
    let mut helper = UiProcess::spawn(&ui_args_from(first, true), &[], std::path::Path::new(""));

    let exit = helper.wait_for_exit();
    if exit.stderr.contains("Failed to create transport") {
        return note_transport_unavailable(&exit.stderr);
    }

    assert!(!exit.status.success(), "expected an error exit, got {:?}", exit.status);
    assert!(
        exit.stderr.contains(&first.to_string()) && exit.stderr.contains(&(first + 9).to_string()),
        "the message should name the ports that were tried: {}",
        exit.stderr
    );
}

#[cfg(target_os = "linux")]
#[test]
fn test_linux_opens_the_browser_with_xdg_open_at_the_launch_url_on_the_bound_port() {
    use std::os::unix::fs::PermissionsExt;
    // A stand-in `xdg-open` that records what it was asked to open.
    let bin = scratch_dir("xdg-open");
    let record = bin.join("opened");
    let script = bin.join("xdg-open");
    std::fs::write(&script, format!("#!/bin/sh\nprintf '%s' \"$1\" > '{}'\n", record.display())).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let (_held, taken) = ports_with_only_the_second_free();
    let mut helper = UiProcess::spawn(&ui_args_from(taken, false), &[], &bin);

    let Some(printed) = helper.launch_url() else {
        return;
    };

    let opened = common::poll_until(std::time::Duration::from_secs(10), || {
        std::fs::read_to_string(&record).ok().filter(|url| !url.is_empty())
    })
    .expect("xdg-open should have been run with the launch URL");
    let _ = std::fs::remove_dir_all(&bin);
    assert_eq!(opened, printed, "the opener and the printed URL should agree");
    assert_eq!(port_of(&printed), taken + 1, "both should use the port actually bound");
}

#[cfg(target_os = "linux")]
#[test]
fn test_helper_keeps_running_and_prints_the_url_when_the_opener_fails() {
    // No xdg-open on PATH: the opener cannot start.
    let bin = scratch_dir("no-opener");
    let (_held, taken) = ports_with_only_the_second_free();
    let mut helper = UiProcess::spawn(&ui_args_from(taken, false), &[], &bin);

    let Some(printed) = helper.launch_url() else {
        return;
    };
    let _ = std::fs::remove_dir_all(&bin);

    assert!(helper.is_running(), "a failed opener must not stop the Helper");
    let token = printed.split_once("?t=").expect("the URL carries the token").1;
    let status = http_get(port_of(&printed), "/api/status", &[("X-Token", token)]);
    assert_eq!(status.status, 200, "the page and API are served at the printed URL");
}

// ---------------------------------------------------------------------------
// Ticket 09: step 3, the game's link to the Helper
//
// A fake DLL connects to the IPC port the way the game does; what the page
// would show of it is read from status.
// ---------------------------------------------------------------------------

/// How long the Helper may take to notice that a DLL connection changed.
const GAME_LINK_DEADLINE: std::time::Duration = std::time::Duration::from_secs(10);

impl Started {
    fn connect_fake_dll(&self) -> common::FakeDll {
        common::FakeDll::connect(self.helper.ipc_port())
            .expect("the Helper should accept a DLL connection")
    }

    /// Wait for status to report the game as connected, or as not connected.
    fn wait_for_game_connected(&self, connected: bool) {
        common::poll_until(GAME_LINK_DEADLINE, || {
            (self.status()["game_connected"] == connected).then_some(())
        })
        .unwrap_or_else(|| panic!("status should report game_connected = {connected}"));
    }

    /// Wait for status to report exactly these banners.
    fn wait_for_banners(&self, expected: &[&str]) {
        common::poll_until(GAME_LINK_DEADLINE, || (self.banners() == expected).then_some(()))
            .unwrap_or_else(|| panic!("expected banners {expected:?}, got {:?}", self.banners()));
    }

    /// The codes of the banners status reports.
    fn banners(&self) -> Vec<String> {
        let status = self.status();
        let banners = status["banners"].as_array().expect("status should carry a banner list");
        banners
            .iter()
            .map(|code| code.as_str().expect("a banner is a code").to_string())
            .collect()
    }
}

/// A message payload that is no IPC request.
const GARBAGE: &[u8] = &[0xff; 4];

#[test]
fn test_fresh_helper_reports_the_game_as_not_connected() {
    let Some(started) = start(false) else {
        return;
    };

    assert_eq!(started.status()["game_connected"], false);
}

#[test]
fn test_game_connected_follows_the_dll_connection() {
    let Some(started) = start(false) else {
        return;
    };
    let mut dll = started.connect_fake_dll();
    // Whatever the answer, the Helper has taken the connection up by now.
    dll.request(&ipc_protocol::IpcRequest::GetOurTicket);
    assert_eq!(
        started.status()["game_connected"], false,
        "a connection that has not shaken hands is not the game yet"
    );

    dll.handshake();
    assert_eq!(started.status()["game_connected"], true);

    // The game was closed.
    drop(dll);
    started.wait_for_game_connected(false);
}

#[test]
fn test_game_connected_turns_false_when_the_dll_connection_ends_on_an_error() {
    let Some(started) = start(false) else {
        return;
    };
    let mut dll = started.connect_fake_dll();
    dll.handshake();
    assert_eq!(started.status()["game_connected"], true);

    // The Helper cannot read this and gives the connection up. The DLL's end
    // stays open, so this is not a clean close.
    dll.send_undecodable(GARBAGE);

    started.wait_for_game_connected(false);
    assert!(
        started.banners().is_empty(),
        "only the first message can say that the DLL does not match"
    );
}

#[test]
fn test_handshake_with_the_wrong_ipc_version_sets_the_ipc_version_mismatch_banner() {
    let Some(started) = start(false) else {
        return;
    };
    let mut dll = started.connect_fake_dll();

    dll.handshake_with_version(ipc_protocol::PROTOCOL_VERSION + 1);

    assert_eq!(started.banners(), ["ipc_version_mismatch"]);
    assert_eq!(
        started.status()["game_connected"], false,
        "a DLL that does not match is not a connected game"
    );
}

#[test]
fn test_garbage_first_message_sets_the_ipc_version_mismatch_banner() {
    let Some(started) = start(false) else {
        return;
    };
    let mut dll = started.connect_fake_dll();

    dll.send_undecodable(GARBAGE);

    started.wait_for_banners(&["ipc_version_mismatch"]);
}

#[test]
fn test_first_message_that_is_not_framed_as_ours_sets_the_ipc_version_mismatch_banner() {
    let Some(started) = start(false) else {
        return;
    };
    let mut dll = started.connect_fake_dll();

    // Read as a length prefix, this announces a message far beyond any the
    // IPC protocol sends.
    dll.send_raw(b"DPLAY/9 hello\n");

    started.wait_for_banners(&["ipc_version_mismatch"]);
}

#[test]
fn test_later_good_handshake_clears_the_ipc_version_mismatch_banner() {
    let Some(started) = start(false) else {
        return;
    };
    let mut mismatched = started.connect_fake_dll();
    mismatched.handshake_with_version(ipc_protocol::PROTOCOL_VERSION + 1);
    assert_eq!(started.banners(), ["ipc_version_mismatch"]);
    drop(mismatched);
    // The Helper serves one DLL connection at a time: an answer on a new
    // one, whatever it is, says the Helper has seen the old one close.
    let mut matching = started.connect_fake_dll();
    matching.request(&ipc_protocol::IpcRequest::GetOurTicket);
    assert_eq!(
        started.banners(),
        ["ipc_version_mismatch"],
        "the banner outlives the connection that set it"
    );

    // The player extracted the archive again and restarted the game.
    matching.handshake();

    assert!(started.banners().is_empty(), "got {:?}", started.banners());
    assert_eq!(started.status()["game_connected"], true);
}

#[test]
fn test_ui_works_with_the_ipc_port_in_use_banner_when_the_ipc_port_is_taken() {
    // Another program holds the IPC port.
    let (_holder, taken_port) = common::hold_port();
    let config = Config {
        ipc_port: taken_port,
        ..ui_config_from(0)
    };

    let helper = match datalink_mp::start(config) {
        Ok(helper) => helper,
        Err(StartError::Transport(e)) => return note_transport_unavailable(&e),
        Err(e) => panic!("a taken IPC port should not stop the Helper in UI mode: {e:?}"),
    };
    let started = Started { helper, opened: OpenedUrls::default() };

    assert_eq!(http_get(started.ui_port(), "/", &[]).status, 200, "the page is served");
    let status = started.status();
    assert_eq!(status["banners"], serde_json::json!(["ipc_port_in_use"]));
    assert_eq!(status["ipc_port"], taken_port, "status names the port that is taken");
    assert_eq!(status["game_connected"], false);
    Ticket::parse(status["ticket"].as_str().expect("status should carry a Ticket"))
        .expect("the Helper still has a Ticket");
}
