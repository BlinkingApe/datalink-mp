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

    /// GET `/api/status` with `token`, the way a tab holding it would: one
    /// from startup or from a show. 403 if `token` is no longer the current one.
    fn status_with_token(&self, token: &str) -> common::HttpResponse {
        http_get(self.ui_port(), "/api/status", &[("X-Token", token)])
    }
}

/// A Game folder that passes the self-check, for the tests that are about
/// something else. There is one for the whole test run, and no test changes it.
fn game_folder_that_passes() -> std::path::PathBuf {
    static FOLDER: OnceLock<std::path::PathBuf> = OnceLock::new();
    FOLDER
        .get_or_init(|| {
            let folder = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("game-folder-that-passes");
            std::fs::create_dir_all(&folder).expect("should create the Game folder");
            for name in ["dplayx.dll", "thinker.exe"] {
                std::fs::write(folder.join(name), b"").expect("should write into the Game folder");
            }
            folder
        })
        .clone()
}

/// Start a Helper on free ports, in a Game folder that passes the self-check.
/// `open_browser` says whether the opener is given to the Helper, the way the
/// absence of `--no-browser` does.
///
/// None when a Transport cannot be created (sandboxed environments); the test
/// then returns early.
fn start(open_browser: bool) -> Option<Started> {
    let opened = OpenedUrls::default();
    let browser_opener = open_browser.then(|| {
        let opened = opened.clone();
        Box::new(move |url: &str| {
            opened.lock().unwrap().push(url.to_string());
            true
        }) as datalink_mp::BrowserOpener
    });
    let config = Config {
        game_folder: game_folder_that_passes(),
        ipc_port: 0,
        transport_options: TransportOptions::default(),
        ui: Some(UiConfig {
            port: 0,
            token: TOKEN.to_string(),
            browser_opener,
        }),
    };
    start_with(config, opened)
}

/// Start a Helper with `config`, which asks for free ports. None when a
/// Transport cannot be created.
fn start_with(config: Config, opened: OpenedUrls) -> Option<Started> {
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
fn test_page_carries_its_fonts_inside_it() {
    let Some(started) = start(false) else {
        return;
    };

    let page = http_get(started.ui_port(), "/", &[]);

    let faces: Vec<&str> = page.body.split("@font-face").skip(1).collect();
    for family in ["Chakra Petch", "IBM Plex Sans", "IBM Plex Mono"] {
        assert!(
            faces.iter().any(|face| face.contains(&format!("font-family: \"{family}\""))),
            "the page should embed {family}"
        );
    }
    // Every font is a woff2 file inside the page, not a file to fetch.
    const PREFIX: &str = "url(data:font/woff2;base64,";
    for face in &faces {
        let start = face.find(PREFIX).expect("each font face is a data URL") + PREFIX.len();
        let encoded = &face[start..start + face[start..].find(')').unwrap()];
        let font = data_encoding::BASE64.decode(encoded.as_bytes()).expect("the font is base64");
        assert_eq!(&font[..4], b"wOF2", "the font is a woff2 file");
    }
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
        game_folder: game_folder_that_passes(),
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
/// `extra` headers. The tests of the rules aim at a GET route, where a POST
/// changes nothing: one that passes the rules reaches routing and gets 405
/// there, and one that breaks them gets the rule's own status instead.
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

/// The page's own inline script, style and fonts, connections to itself, no
/// framing, and nothing else.
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
    // Fonts: only the ones the page carries inside it.
    assert_eq!(sources("font-src"), ["data:"], "{kind}: {csp}");
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
                "font-src",
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
        game_folder: game_folder_that_passes(),
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
        Self::spawn_in(std::path::Path::new("."), args, envs, path)
    }

    /// The same, started from `working_dir` the way a launcher or a shell
    /// would start it from a folder of its own.
    fn spawn_in(
        working_dir: &std::path::Path,
        args: &[String],
        envs: &[(&str, &str)],
        path: &std::path::Path,
    ) -> Self {
        use std::io::{BufRead, BufReader};
        use std::process::{Command, Stdio};
        let mut child = Command::new(env!("CARGO_BIN_EXE_datalink-mp"))
            .current_dir(working_dir)
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
        // The reader thread ends at the end of the output, which closes the channel.
        let stdout = self.stdout_lines.iter().collect::<Vec<_>>().join("\n");
        UiExit { status, stdout, stderr }
    }

    #[cfg(target_os = "linux")]
    fn is_running(&mut self) -> bool {
        self.child.try_wait().expect("should poll the process").is_none()
    }
}

struct UiExit {
    status: std::process::ExitStatus,
    stdout: String,
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

fn token_of(launch_url: &str) -> &str {
    launch_url
        .split_once("/?t=")
        .map(|(_, token)| token)
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

/// A fresh empty directory for a test to put files in. Each call gives a
/// directory of its own, so tests running side by side do not share one.
fn scratch_dir(name: &str) -> std::path::PathBuf {
    static MADE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let nth = MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("datalink-mp-{name}-{}-{nth}", std::process::id()));
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

/// Start a Helper on an IPC port that another program holds, which is the
/// listener returned with it. None when a Transport cannot be created.
fn start_with_the_ipc_port_taken() -> Option<(Started, std::net::TcpListener)> {
    let (holder, taken_port) = common::hold_port();
    let config = Config {
        ipc_port: taken_port,
        ..ui_config_from(0)
    };
    match datalink_mp::start(config) {
        Ok(helper) => Some((Started { helper, opened: OpenedUrls::default() }, holder)),
        Err(StartError::Transport(e)) => {
            note_transport_unavailable(&e);
            None
        }
        Err(e) => panic!("a taken IPC port should not stop the Helper in UI mode: {e:?}"),
    }
}

#[test]
fn test_ui_works_with_the_ipc_port_in_use_banner_when_the_ipc_port_is_taken() {
    let Some((started, holder)) = start_with_the_ipc_port_taken() else {
        return;
    };
    let taken_port = holder.local_addr().unwrap().port();

    assert_eq!(http_get(started.ui_port(), "/", &[]).status, 200, "the page is served");
    let status = started.status();
    assert_eq!(status["banners"], serde_json::json!(["ipc_port_in_use"]));
    assert_eq!(status["ipc_port"], taken_port, "status names the port that is taken");
    assert_eq!(status["game_connected"], false);
    Ticket::parse(status["ticket"].as_str().expect("status should carry a Ticket"))
        .expect("the Helper still has a Ticket");
}

// ---------------------------------------------------------------------------
// Ticket 14: Quit
//
// Quit ends the Helper. In the test process that shows as the Helper's handle
// finishing; the built binary exits then (see the smoke tests).
// ---------------------------------------------------------------------------

/// How long the Helper may take to finish once it was told to quit.
const QUIT_DEADLINE: std::time::Duration = std::time::Duration::from_secs(10);

/// Wait on `helper` on another thread, the way the binary's `main` does from
/// the moment the Helper has started.
fn wait_in_the_background(helper: Helper) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || helper.wait())
}

/// Assert that the wait on a Helper ends: the Helper has finished. Panics if
/// it is still running at the deadline.
fn assert_finishes(waiting: std::thread::JoinHandle<()>) {
    common::poll_until(QUIT_DEADLINE, || waiting.is_finished().then_some(()))
        .expect("the Helper should finish after Quit");
    waiting.join().expect("the Helper should finish without a panic");
}

/// Wait on `helper` and return once it has finished.
fn wait_for_the_helper_to_finish(helper: Helper) {
    assert_finishes(wait_in_the_background(helper));
}

#[test]
fn test_quit_is_answered_and_then_the_helper_finishes() {
    let Some(started) = start(false) else {
        return;
    };
    let ui_port = started.ui_port();
    // Whatever waits on the Helper stops its servers once it has quit: the
    // answer to Quit has to get out first.
    let waiting = wait_in_the_background(started.helper);

    quit_on(ui_port);

    assert_finishes(waiting);
}

/// Quit the Helper whose page is on `ui_port` with the token, the way the
/// page does, and assert it was answered.
fn quit_on(ui_port: u16) {
    let response = common::http_request(
        ui_port,
        "POST",
        "/api/quit",
        &[("X-Token", TOKEN), ("Content-Type", "application/json")],
    );
    assert_eq!(response.status, 204, "Quit should be answered before the Helper goes away");
}

fn quit(started: &Started) {
    quit_on(started.ui_port());
}

/// Assert that another program could take `port` now.
fn assert_port_is_released(what: &str, port: u16) {
    if let Err(e) = std::net::TcpListener::bind(("127.0.0.1", port)) {
        panic!("the {what} port {port} should be released after Quit: {e}");
    }
}

#[test]
fn test_quit_releases_the_ui_port_and_the_ipc_port() {
    let Some(started) = start(false) else {
        return;
    };
    let (ui_port, ipc_port) = (started.ui_port(), started.helper.ipc_port());

    quit(&started);
    wait_for_the_helper_to_finish(started.helper);

    assert_port_is_released("UI", ui_port);
    assert_port_is_released("IPC", ipc_port);
}

#[test]
fn test_quit_with_the_game_connected_finishes_and_ends_the_games_link() {
    let Some(started) = start(false) else {
        return;
    };
    // The game is open, and its DLL is waiting on the Helper.
    let mut dll = started.connect_fake_dll();
    dll.handshake();
    let ipc_port = started.helper.ipc_port();

    quit(&started);
    wait_for_the_helper_to_finish(started.helper);

    assert!(dll.is_closed_by_the_helper(), "the game's link should end with the Helper");
    assert_port_is_released("IPC", ipc_port);
}

/// How long a friend's Transport may take to see a connect, or a graceful
/// close, on loopback. Well below the 30 s idle timeout, which is how a friend
/// finds out about a Helper that went away without telling them.
const PEER_NOTICE_DEADLINE: std::time::Duration = std::time::Duration::from_secs(10);

#[test]
fn test_connected_friend_sees_the_connection_close_promptly_after_quit() {
    let Some(started) = start(false) else {
        return;
    };
    let Some(friend) = friend() else {
        return;
    };
    dial_the_helper(&friend, &started);
    wait_until_the_friend_lists_the_helper(&friend, &started);

    quit(&started);

    // The Helper's handle is still held, so its Transport is not dropped:
    // the friend can only be seeing a close that was sent to it.
    let closed = common::poll_until(PEER_NOTICE_DEADLINE, || {
        friend.connected_peers().is_empty().then_some(())
    });
    assert!(
        closed.is_some(),
        "the friend should see the close within {PEER_NOTICE_DEADLINE:?}, still lists {:?}",
        friend.connected_peers()
    );
    wait_for_the_helper_to_finish(started.helper);
}

#[test]
fn test_quit_is_refused_without_the_token_from_a_foreign_origin_and_by_get() {
    let Some(started) = start(false) else {
        return;
    };
    let port = started.ui_port();
    let json = ("Content-Type", "application/json");

    let no_token = common::http_request(port, "POST", "/api/quit", &[json]);
    let wrong_token =
        common::http_request(port, "POST", "/api/quit", &[json, ("X-Token", "not-the-token")]);
    let foreign_origin = post(&started, "/api/quit", &[("Origin", "http://rebind.example")]);
    let opaque_origin = post(&started, "/api/quit", &[("Origin", "null")]);
    let by_get = started.get_api("/api/quit");

    assert_eq!(no_token.status, 403);
    assert_eq!(wrong_token.status, 403);
    assert_eq!(foreign_origin.status, 403);
    assert_eq!(opaque_origin.status, 403);
    assert_eq!(by_get.status, 405);
    // Each was refused before it reached Quit. The Helper is still there for
    // its own page to quit.
    assert_eq!(started.status()["state"], "ready");
    let own_origin = format!("http://127.0.0.1:{port}");
    let from_the_page = post(&started, "/api/quit", &[("Origin", &own_origin)]);
    assert_eq!(from_the_page.status, 204, "the page's own Quit should be accepted");
    wait_for_the_helper_to_finish(started.helper);
}

#[test]
fn test_quit_finishes_a_helper_that_has_no_ipc_server() {
    // The Helper serves only its page: there is no IPC server to wait on.
    let Some((started, _holder)) = start_with_the_ipc_port_taken() else {
        return;
    };
    let ui_port = started.ui_port();

    quit(&started);
    wait_for_the_helper_to_finish(started.helper);

    assert_port_is_released("UI", ui_port);
}

// ---------------------------------------------------------------------------
// Ticket 08: step 1, the Game folder self-check
//
// Each test makes a Game folder of its own and starts a Helper on it; what the
// page would show of the self-check is read from status.
// ---------------------------------------------------------------------------

/// A Game folder the test controls: a scratch directory holding empty files
/// with the names the self-check looks for. Removed when dropped.
struct GameFolder {
    path: std::path::PathBuf,
}

impl GameFolder {
    fn holding(file_names: &[&str]) -> Self {
        let folder = Self { path: scratch_dir("game-folder") };
        for name in file_names {
            folder.add(name);
        }
        folder
    }

    fn add(&self, file_name: &str) {
        std::fs::write(self.path.join(file_name), b"").expect("should write into the Game folder");
    }
}

impl Drop for GameFolder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Start a Helper on free ports whose Game folder is `game_folder`. None when
/// a Transport cannot be created.
fn start_in(game_folder: &GameFolder) -> Option<Started> {
    let config = Config {
        game_folder: game_folder.path.clone(),
        ..ui_config_from(0)
    };
    start_with(config, OpenedUrls::default())
}

/// The status of a Helper started in a Game folder holding these files. None
/// when a Transport cannot be created.
fn status_in_a_folder_holding(file_names: &[&str]) -> Option<serde_json::Value> {
    let folder = GameFolder::holding(file_names);
    Some(start_in(&folder)?.status())
}

#[test]
fn test_empty_folder_fails_the_self_check_and_sets_the_not_game_folder_banner() {
    let Some(status) = status_in_a_folder_holding(&[]) else {
        return;
    };

    assert_eq!(status["self_check"]["passed"], false);
    assert_eq!(status["self_check"]["dll_found"], false);
    assert_eq!(status["self_check"]["game_exe"], serde_json::Value::Null);
    assert_eq!(status["banners"], serde_json::json!(["not_game_folder"]));
}

#[test]
fn test_folder_with_only_the_dll_fails_the_self_check_and_reports_the_dll_as_found() {
    let Some(status) = status_in_a_folder_holding(&["dplayx.dll"]) else {
        return;
    };

    assert_eq!(status["self_check"]["passed"], false);
    assert_eq!(status["self_check"]["dll_found"], true);
    assert_eq!(status["self_check"]["game_exe"], serde_json::Value::Null);
    assert_eq!(status["banners"], serde_json::json!(["not_game_folder"]));
}

#[test]
fn test_folder_with_only_a_game_executable_fails_the_self_check_and_reports_the_dll_as_missing() {
    let Some(status) = status_in_a_folder_holding(&["thinker.exe"]) else {
        return;
    };

    assert_eq!(status["self_check"]["passed"], false);
    assert_eq!(status["self_check"]["dll_found"], false);
    assert_eq!(status["self_check"]["game_exe"], "thinker.exe");
    assert_eq!(status["banners"], serde_json::json!(["not_game_folder"]));
}

#[test]
fn test_folder_with_the_dll_and_thinker_passes_the_self_check() {
    let Some(status) = status_in_a_folder_holding(&["dplayx.dll", "thinker.exe"]) else {
        return;
    };

    assert_eq!(status["self_check"]["passed"], true);
    assert_eq!(status["self_check"]["dll_found"], true);
    assert_eq!(status["self_check"]["game_exe"], "thinker.exe");
    assert_eq!(status["banners"], serde_json::json!([]));
}

#[test]
fn test_folder_with_the_dll_and_pracx_passes_the_self_check() {
    let Some(status) = status_in_a_folder_holding(&["dplayx.dll", "terran_PRACX.exe"]) else {
        return;
    };

    assert_eq!(status["self_check"]["passed"], true);
    assert_eq!(status["self_check"]["dll_found"], true);
    assert_eq!(status["self_check"]["game_exe"], "terran_PRACX.exe");
    assert_eq!(status["banners"], serde_json::json!([]));
}

#[test]
fn test_folder_with_the_dll_and_wtp_passes_the_self_check() {
    let Some(status) = status_in_a_folder_holding(&["dplayx.dll", "wtp.exe"]) else {
        return;
    };

    assert_eq!(status["self_check"]["passed"], true);
    assert_eq!(status["self_check"]["dll_found"], true);
    assert_eq!(status["self_check"]["game_exe"], "wtp.exe");
    assert_eq!(status["banners"], serde_json::json!([]));
}

#[test]
fn test_differently_cased_file_names_pass_the_self_check() {
    // The game's files as some installs have them, and as Wine users find them.
    for file_names in [
        ["DPLAYX.DLL", "Thinker.EXE"],
        ["Dplayx.dll", "TERRAN_pracx.exe"],
        ["Dplayx.dll", "WTP.exe"],
    ] {
        let Some(status) = status_in_a_folder_holding(&file_names) else {
            return;
        };

        assert_eq!(status["self_check"]["passed"], true, "{file_names:?} should pass");
        assert_eq!(status["self_check"]["dll_found"], true, "{file_names:?}");
        assert_eq!(
            status["self_check"]["game_exe"], file_names[1],
            "the game executable is named as it is in the folder"
        );
        assert_eq!(status["banners"], serde_json::json!([]), "{file_names:?}");
    }
}

#[test]
fn test_restoring_the_missing_file_clears_the_not_game_folder_banner_without_a_restart() {
    // The antivirus took the DLL away.
    let folder = GameFolder::holding(&["thinker.exe"]);
    let Some(started) = start_in(&folder) else {
        return;
    };
    assert_eq!(started.banners(), ["not_game_folder"]);

    // The player restored it.
    folder.add("dplayx.dll");

    let status = started.status();
    assert_eq!(status["banners"], serde_json::json!([]));
    assert_eq!(status["self_check"]["passed"], true);
    assert_eq!(status["self_check"]["dll_found"], true);
}

#[test]
fn test_self_check_reports_the_folder_it_checked() {
    let folder = GameFolder::holding(&[]);
    let Some(started) = start_in(&folder) else {
        return;
    };

    let status = started.status();

    assert_eq!(status["self_check"]["folder"], folder.path.to_str().unwrap());
}

#[test]
fn test_game_folder_is_where_the_executable_is_not_the_working_directory() {
    // The folder the built binary is in. No game is installed there.
    let exe_folder = std::path::Path::new(env!("CARGO_BIN_EXE_datalink-mp"))
        .parent()
        .expect("the binary is in a folder")
        .canonicalize()
        .expect("the binary's folder should resolve");
    // Started from a Game folder that would pass, the way a shell sitting in
    // the Game folder would start a Helper that was left in Downloads.
    let working_dir = GameFolder::holding(&["dplayx.dll", "thinker.exe"]);
    let mut helper =
        UiProcess::spawn_in(&working_dir.path, &ui_args_from(0, true), &[], std::path::Path::new(""));

    let Some(url) = helper.launch_url() else {
        return;
    };

    let token = url.split_once("?t=").expect("the URL carries the token").1;
    let status = http_get(port_of(&url), "/api/status", &[("X-Token", token)]).json();
    let checked = status["self_check"]["folder"].as_str().expect("status should carry the folder");
    assert_eq!(
        std::path::Path::new(checked).canonicalize().expect("the checked folder should resolve"),
        exe_folder
    );
    assert_eq!(status["self_check"]["passed"], false, "the working directory is not what is checked");
    assert_eq!(status["banners"], serde_json::json!(["not_game_folder"]));
}

#[test]
fn test_ticket_is_still_in_status_when_the_self_check_fails() {
    let Some(status) = status_in_a_folder_holding(&[]) else {
        return;
    };

    assert_eq!(status["self_check"]["passed"], false);
    Ticket::parse(status["ticket"].as_str().expect("status should carry a Ticket"))
        .expect("a failed self-check blocks nothing: the Helper still has a Ticket");
    assert_eq!(status["state"], "ready");
}

// ---------------------------------------------------------------------------
// Ticket 10: step 4, hosting and the connected Helpers
//
// A friend is a second real Transport on loopback that dials the Helper's
// Ticket, the way a friend's Helper does; what the page would show of it is
// read from status.
// ---------------------------------------------------------------------------

/// A friend's Transport. None when one cannot be created (sandboxed
/// environments); the test then returns early.
fn friend() -> Option<iroh_transport::Transport> {
    match iroh_transport::Transport::new() {
        Ok(friend) => Some(friend),
        Err(e) => {
            note_transport_unavailable(&e);
            None
        }
    }
}

/// Dial the Helper's Ticket from `friend`, the way a friend who was sent the
/// Ticket does.
fn dial_the_helper(friend: &iroh_transport::Transport, started: &Started) {
    friend
        .connect_to_peer(&started.ticket())
        .expect("the friend should reach the Helper on loopback");
}

/// Wait for `friend` to list the Helper among its connected Helpers.
fn wait_until_the_friend_lists_the_helper(friend: &iroh_transport::Transport, started: &Started) {
    let helper_id = Ticket::parse(&started.ticket()).expect("the Ticket should parse").addr().id;
    common::poll_until(PEER_NOTICE_DEADLINE, || {
        friend.connected_peers().contains(&helper_id).then_some(())
    })
    .expect("the friend should list the Helper as connected");
}

impl Started {
    /// The Helper's Ticket, as the player would copy it from the page.
    fn ticket(&self) -> String {
        let status = self.status();
        status["ticket"].as_str().expect("status should carry a Ticket").to_string()
    }

    /// Wait for status to report `state`.
    fn wait_for_state(&self, state: &str) {
        common::poll_until(PEER_NOTICE_DEADLINE, || (self.status()["state"] == state).then_some(()))
            .unwrap_or_else(|| panic!("expected the state {state}, got {}", self.status()["state"]));
    }

    /// Wait for status to list exactly these short IDs as the connected Helpers.
    fn wait_for_peers(&self, expected: &[String]) {
        let expected = serde_json::json!(expected);
        common::poll_until(PEER_NOTICE_DEADLINE, || (self.status()["peers"] == expected).then_some(()))
            .unwrap_or_else(|| panic!("expected the peers {expected}, got {}", self.status()["peers"]));
    }
}

#[test]
fn test_friend_dialling_the_helpers_ticket_makes_the_state_hosting() {
    let Some(started) = start(false) else {
        return;
    };
    let Some(friend) = friend() else {
        return;
    };
    assert_eq!(started.status()["state"], "ready");

    dial_the_helper(&friend, &started);

    started.wait_for_state("hosting");
}

/// The short ID the page shows for `transport`'s Helper: iroh's short form of
/// its endpoint ID.
fn short_id(transport: &iroh_transport::Transport) -> String {
    transport.endpoint_id().fmt_short().to_string()
}

#[test]
fn test_connected_friend_is_listed_by_its_short_id_and_lists_the_helper() {
    let Some(started) = start(false) else {
        return;
    };
    let Some(friend) = friend() else {
        return;
    };

    dial_the_helper(&friend, &started);

    started.wait_for_peers(&[short_id(&friend)]);
    wait_until_the_friend_lists_the_helper(&friend, &started);
}

#[test]
fn test_peer_list_empties_and_the_state_returns_to_ready_when_the_friend_goes_away() {
    let Some(started) = start(false) else {
        return;
    };
    let Some(friend) = friend() else {
        return;
    };
    dial_the_helper(&friend, &started);
    started.wait_for_peers(&[short_id(&friend)]);

    // The friend quit their Helper.
    friend.shutdown();

    started.wait_for_peers(&[]);
    assert_eq!(started.status()["state"], "ready");
}

#[test]
fn test_two_friends_give_two_entries_in_the_peer_list() {
    let Some(started) = start(false) else {
        return;
    };
    let (Some(first), Some(second)) = (friend(), friend()) else {
        return;
    };

    dial_the_helper(&first, &started);
    dial_the_helper(&second, &started);

    // Sorted, so that the page lists them the same way on every poll.
    let mut both = [short_id(&first), short_id(&second)];
    both.sort();
    started.wait_for_peers(&both);

    // One friend leaves: the other is still connected, and still hosted.
    first.shutdown();
    started.wait_for_peers(&[short_id(&second)]);
    assert_eq!(started.status()["state"], "hosting");
}

/// How long one status request may take. The page asks once a second, so a
/// slower answer leaves it behind; an answer that waited on the Transport
/// would be slower than this, or would not come at all.
const STATUS_DEADLINE: std::time::Duration = std::time::Duration::from_secs(1);

#[test]
fn test_status_is_answered_promptly_while_friends_connect_and_disconnect() {
    let Some(started) = start(false) else {
        return;
    };
    let (Some(first), Some(second), Some(third)) = (friend(), friend(), friend()) else {
        return;
    };
    let friends = [first, second, third];
    // Each friend's arrival and each departure may take this long to show.
    let coming_and_going = PEER_NOTICE_DEADLINE * (2 * friends.len() as u32 + 1);

    let (slowest, seen_hosting) = std::thread::scope(|scope| {
        // The friends arrive one by one, then leave one by one.
        let friends_come_and_go = scope.spawn(|| {
            let mut connected = Vec::new();
            for friend in &friends {
                dial_the_helper(friend, &started);
                connected.push(short_id(friend));
                connected.sort();
                started.wait_for_peers(&connected);
            }
            for friend in &friends {
                friend.shutdown();
                connected.retain(|id| *id != short_id(friend));
                started.wait_for_peers(&connected);
            }
        });

        // Meanwhile the page keeps asking, as it does about once a second;
        // here much more often.
        let mut slowest = std::time::Duration::ZERO;
        let mut seen_hosting = false;
        common::poll_until(coming_and_going, || {
            let asked = std::time::Instant::now();
            let status = started.status();
            slowest = slowest.max(asked.elapsed());
            seen_hosting |= status["state"] == "hosting";
            friends_come_and_go.is_finished().then_some(())
        })
        .expect("the friends should have come and gone by now");
        friends_come_and_go.join().expect("the friends should come and go as status says");
        (slowest, seen_hosting)
    });

    assert!(seen_hosting, "the page should have asked while the friends were connected");
    assert!(
        slowest <= STATUS_DEADLINE,
        "a status request took {slowest:?} while friends connected and disconnected"
    );
    assert_eq!(started.status()["state"], "ready");
}

// ---------------------------------------------------------------------------
// Ticket 15: single instance
//
// Three routes work without the token so that a second Helper can find the
// first, ask it to make way, or ask it to show its page. The rest of the
// rules still apply.
// ---------------------------------------------------------------------------

/// POST to `/api/show` the way a second Helper does: no token, a JSON content
/// type, and `extra` headers.
fn post_show(ui_port: u16, extra: &[(&str, &str)]) -> common::HttpResponse {
    post_public(ui_port, "/api/show", extra)
}

/// POST to `/api/replace` the way a second Helper does.
fn post_replace(ui_port: u16, extra: &[(&str, &str)]) -> common::HttpResponse {
    post_public(ui_port, "/api/replace", extra)
}

fn post_public(ui_port: u16, path: &str, extra: &[(&str, &str)]) -> common::HttpResponse {
    let mut headers = vec![("Content-Type", "application/json")];
    headers.extend_from_slice(extra);
    common::http_request(ui_port, "POST", path, &headers)
}

#[test]
fn test_instance_needs_no_token_and_names_the_application_release_and_ipc_port() {
    let Some(started) = start(false) else {
        return;
    };

    let response = http_get(started.ui_port(), "/api/instance", &[]);

    assert_eq!(response.status, 200, "{}", response.body);
    let instance = response.json();
    assert_eq!(instance["app"], "datalink-mp");
    assert_eq!(instance["release_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(instance["ipc_port"], started.helper.ipc_port());
    assert!(!response.body.contains(TOKEN), "the route must not reveal the token");
}

#[test]
fn test_instance_and_status_name_the_same_build() {
    let Some(started) = start(false) else {
        return;
    };

    let instance = http_get(started.ui_port(), "/api/instance", &[]).json();
    let status = started.status();

    let build = instance["build_id"].as_str().expect("the instance names its build");
    assert!(!build.is_empty());
    assert_eq!(status["build_id"], build, "the page shows the build the instance names");
}

#[test]
fn test_show_needs_no_token_and_opens_the_browser_at_the_launch_url() {
    let Some(started) = start(true) else {
        return;
    };
    let startup_url = started.helper.launch_url().unwrap();
    // The opener was called once at startup.
    assert_eq!(started.opened.lock().unwrap().len(), 1);

    let response = post_show(started.ui_port(), &[]);

    assert_eq!(response.status, 204, "{}", response.body);
    // The opener runs off the request's thread: wait for it.
    wait_for_opened(&started, 2);
    let reopened = started.opened.lock().unwrap()[1].clone();
    assert_eq!(port_of(&reopened), port_of(&startup_url), "the same Helper, shown again");
    assert_ne!(reopened, startup_url, "a fresh tab gets a fresh token, so an old tab is left behind");
    assert!(!response.body.contains(TOKEN), "the route must not reveal the token");

    assert_eq!(
        started.status_with_token(token_of(&startup_url)).status,
        403,
        "the tab open before this show is no longer admitted"
    );
    assert_eq!(
        started.status_with_token(token_of(&reopened)).status,
        200,
        "the freshly shown tab works"
    );
}

#[test]
fn test_show_whose_browser_did_not_open_keeps_the_tab_the_player_already_has() {
    let opened = OpenedUrls::default();
    let recorded = opened.clone();
    let mut config = ui_config_from(common::free_port());
    config.ui.as_mut().unwrap().browser_opener = Some(Box::new(move |url: &str| {
        recorded.lock().unwrap().push(url.to_string());
        false
    }));
    let Some(started) = start_with(config, opened) else {
        return;
    };
    let startup_url = started.helper.launch_url().unwrap();

    let response = post_show(started.ui_port(), &[]);

    assert_eq!(response.status, 204, "{}", response.body);
    wait_for_opened(&started, 2);
    let not_shown = started.opened.lock().unwrap()[1].clone();
    // The opener answers before the token is put back: wait for that.
    common::poll_until(SHOW_DEADLINE, || {
        (started.status_with_token(token_of(&not_shown)).status == 403).then_some(())
    })
    .expect("a tab the browser never opened should not be the one admitted");
    assert_eq!(
        started.status_with_token(token_of(&startup_url)).status,
        200,
        "with no new tab, the player's existing tab must keep working"
    );
    assert_eq!(started.helper.launch_url().unwrap(), startup_url, "the printed address still works");
}

#[test]
fn test_show_asked_again_within_the_limit_does_not_open_the_browser_again() {
    let Some(started) = start(true) else {
        return;
    };

    let first = post_show(started.ui_port(), &[]);
    let second = post_show(started.ui_port(), &[]);

    assert_eq!(first.status, 204);
    assert_eq!(second.status, 429, "a second request within the limit is refused");
    wait_for_opened(&started, 2);
    assert_eq!(started.opened.lock().unwrap().len(), 2, "startup and the first show only");
}

#[test]
fn test_show_without_a_browser_opener_changes_nothing_and_does_not_fail() {
    let Some(started) = start(false) else {
        return;
    };

    let response = post_show(started.ui_port(), &[]);

    assert_eq!(response.status, 204);
    assert!(started.opened.lock().unwrap().is_empty());
}

#[test]
fn test_the_single_instance_routes_still_refuse_another_host_name() {
    let Some(started) = start(true) else {
        return;
    };
    let port = started.ui_port();
    let host = format!("rebind.example:{port}");

    let instance = http_get(port, "/api/instance", &[("Host", &host)]);
    let show = post_show(port, &[("Host", &host)]);
    let replace = post_replace(port, &[("Host", &host)]);

    assert_eq!(instance.status, 403);
    assert_eq!(show.status, 403);
    assert_eq!(replace.status, 403);
    assert_eq!(started.opened.lock().unwrap().len(), 1, "a refused request opens nothing");
    assert_eq!(started.status()["state"], "ready", "a refused replace quits nothing");
}

#[test]
fn test_show_still_refuses_a_foreign_origin_and_needs_a_json_content_type() {
    let Some(started) = start(true) else {
        return;
    };
    let port = started.ui_port();

    let foreign = post_show(port, &[("Origin", "http://rebind.example")]);
    let not_json = common::http_request(port, "POST", "/api/show", &[("Content-Type", "text/plain")]);
    let by_get = http_get(port, "/api/show", &[]);

    assert_eq!(foreign.status, 403);
    assert_eq!(not_json.status, 415);
    assert_eq!(by_get.status, 405);
    assert_eq!(started.opened.lock().unwrap().len(), 1, "a refused request opens nothing");
}

#[test]
fn test_replace_still_refuses_a_foreign_origin_and_needs_a_json_content_type() {
    let Some(started) = start(false) else {
        return;
    };
    let port = started.ui_port();

    let foreign = post_replace(port, &[("Origin", "http://rebind.example")]);
    let not_json = common::http_request(port, "POST", "/api/replace", &[("Content-Type", "text/plain")]);
    let by_get = http_get(port, "/api/replace", &[]);

    assert_eq!(foreign.status, 403);
    assert_eq!(not_json.status, 415);
    assert_eq!(by_get.status, 405);
    assert_eq!(started.status()["state"], "ready", "a refused replace quits nothing");
}

#[test]
fn test_instance_is_read_only_and_other_api_routes_still_need_the_token() {
    let Some(started) = start(false) else {
        return;
    };
    let port = started.ui_port();

    let instance_by_post =
        common::http_request(port, "POST", "/api/instance", &[("Content-Type", "application/json")]);
    let status = http_get(port, "/api/status", &[]);

    assert_eq!(instance_by_post.status, 405);
    assert_eq!(status.status, 403, "only the single-instance routes are public");
}

/// Start a Helper on the IPC port `ipc_port`, serving the page from `ui_port`
/// upwards, with a recording stand-in for the browser opener. Fails with
/// whatever `start` fails with; None when a Transport cannot be created.
fn try_start_on(ipc_port: u16, ui_port: u16) -> Option<Result<Started, StartError>> {
    let opened = OpenedUrls::default();
    let recorded = opened.clone();
    let mut config = ui_config_from(ui_port);
    config.ipc_port = ipc_port;
    config.ui.as_mut().unwrap().browser_opener = Some(Box::new(move |url: &str| {
        recorded.lock().unwrap().push(url.to_string());
        true
    }));
    match datalink_mp::start(config) {
        Ok(helper) => Some(Ok(Started { helper, opened })),
        Err(StartError::Transport(e)) => {
            note_transport_unavailable(&e);
            None
        }
        Err(e) => Some(Err(e)),
    }
}

/// How long the first Helper may take to be asked to open its page.
const SHOW_DEADLINE: std::time::Duration = std::time::Duration::from_secs(10);

fn wait_for_opened(started: &Started, count: usize) {
    common::poll_until(SHOW_DEADLINE, || (started.opened.lock().unwrap().len() >= count).then_some(()))
        .unwrap_or_else(|| panic!("the opener should have been called {count} times"));
}

#[test]
fn test_second_start_on_the_same_ipc_port_replaces_an_idle_first_helper() {
    let ipc_port = common::free_port();
    let Some(first) = try_start_on(ipc_port, common::free_port()) else {
        return;
    };
    let first = first.expect("the first Helper should start");
    let ui_port = first.ui_port();
    let first_opened = first.opened.clone();
    // As the binary's main does: whatever waits on a Helper ends it once it quits.
    let first_waiting = wait_in_the_background(first.helper);

    let Some(second) = try_start_on(ipc_port, ui_port) else {
        return;
    };

    let second = second.expect("an idle Helper makes way for the one started after it");
    assert_finishes(first_waiting);
    assert_eq!(second.helper.ipc_port(), ipc_port, "the second Helper serves the game now");
    assert_eq!(second.opened.lock().unwrap().len(), 1, "the second opens its own page");
    assert_eq!(first_opened.lock().unwrap().len(), 1, "the first was not asked to show its page");
}

#[test]
fn test_second_start_leaves_a_first_helper_the_game_is_connected_to_and_its_page_says_why() {
    let ipc_port = common::free_port();
    let Some(first) = try_start_on(ipc_port, common::free_port()) else {
        return;
    };
    let first = first.expect("the first Helper should start");
    let original = first.helper.launch_url().unwrap();
    let mut dll = first.connect_fake_dll();
    dll.handshake();

    let Some(second) = try_start_on(ipc_port, first.ui_port()) else {
        return;
    };

    assert!(matches!(second, Err(StartError::AlreadyRunning)), "got {:?}", second.err());
    wait_for_opened(&first, 2);
    let reopened = first.opened.lock().unwrap()[1].clone();
    assert_eq!(port_of(&reopened), port_of(&original), "the same Helper, shown again");
    assert_ne!(reopened, original, "a fresh tab for the refused second start gets a fresh token");
    assert_eq!(
        first.status_with_token(token_of(&original)).status,
        403,
        "the tab the player had open is no longer admitted"
    );
    let status = first.status_with_token(token_of(&reopened)).json();
    assert_eq!(status["banners"], serde_json::json!(["restart_refused"]));
    assert_eq!(status["game_connected"], true, "the game keeps its Helper");
}

#[test]
fn test_second_start_leaves_a_first_helper_a_friend_is_connected_to() {
    let ipc_port = common::free_port();
    let Some(first) = try_start_on(ipc_port, common::free_port()) else {
        return;
    };
    let first = first.expect("the first Helper should start");
    let Some(friend) = friend() else {
        return;
    };
    dial_the_helper(&friend, &first);
    first.wait_for_state("hosting");

    let Some(second) = try_start_on(ipc_port, first.ui_port()) else {
        return;
    };

    assert!(matches!(second, Err(StartError::AlreadyRunning)), "got {:?}", second.err());
    wait_for_opened(&first, 2);
    let reopened = first.opened.lock().unwrap()[1].clone();
    let status = first.status_with_token(token_of(&reopened)).json();
    assert_eq!(status["banners"], serde_json::json!(["restart_refused"]));
    assert_eq!(status["state"], "hosting", "the friend keeps their connection");
}

#[test]
fn test_first_helper_is_found_when_it_is_on_a_later_port_of_the_ui_range() {
    let ipc_port = common::free_port();
    let (_held, base) = ports_with_only_the_second_free();
    let Some(first) = try_start_on(ipc_port, base) else {
        return;
    };
    let first = first.expect("the first Helper should start");
    assert_eq!(first.ui_port(), base + 1);
    // In use, so that it stays and shows its page.
    let mut dll = first.connect_fake_dll();
    dll.handshake();

    let Some(second) = try_start_on(ipc_port, base) else {
        return;
    };

    assert!(matches!(second, Err(StartError::AlreadyRunning)), "got {:?}", second.err());
    wait_for_opened(&first, 2);
}

#[test]
fn test_second_start_on_a_different_ipc_port_runs_and_takes_the_next_ui_port() {
    let (mut held, base) = ports_with_only_the_second_free();
    let Some(first) = try_start_on(common::free_port(), base) else {
        return;
    };
    let first = first.expect("the first Helper should start");
    assert_eq!(first.ui_port(), base + 1);
    // Free the next port too, for the second Helper to walk to.
    drop(held.remove(1));

    let Some(second) = try_start_on(common::free_port(), base) else {
        return;
    };

    let second = second.expect("a Helper on another IPC port is a separate instance");
    assert_eq!(second.ui_port(), first.ui_port() + 1);
    assert_eq!(first.opened.lock().unwrap().len(), 1, "the first Helper was not asked to show");
    assert_eq!(second.opened.lock().unwrap().len(), 1, "the second opens its own page");
    assert_ne!(first.status()["ticket"], second.status()["ticket"]);
}

#[test]
fn test_a_program_holding_the_ipc_port_is_not_mistaken_for_a_running_helper() {
    // A Helper runs on one IPC port, and another program holds a different one.
    let Some(first) = try_start_on(common::free_port(), common::free_port()) else {
        return;
    };
    let first = first.expect("the first Helper should start");
    let (holder, taken_port) = common::hold_port();

    let Some(second) = try_start_on(taken_port, first.ui_port()) else {
        return;
    };

    let second = second.expect("a held IPC port is not a running Helper: the UI still starts");
    assert_eq!(second.banners(), ["ipc_port_in_use"]);
    assert_eq!(first.opened.lock().unwrap().len(), 1, "the first Helper was not asked to show");
    drop(holder);
}

#[test]
fn test_the_helper_writes_no_lock_or_state_file() {
    let folder = GameFolder::holding(&[]);
    let ipc_port = common::free_port();
    let config = |folder: &GameFolder| Config {
        game_folder: folder.path.clone(),
        ipc_port,
        ..ui_config_from(0)
    };
    let Some(first) = start_with(config(&folder), OpenedUrls::default()) else {
        return;
    };
    let mut dll = first.connect_fake_dll();
    dll.handshake();
    let second = datalink_mp::start(Config { ui: first.helper.ui_port().map(|port| UiConfig { port, ..ui_config_from(0).ui.unwrap() }), ..config(&folder) });
    assert!(matches!(second, Err(StartError::AlreadyRunning)));

    let files = std::fs::read_dir(&folder.path).unwrap().count();
    assert_eq!(files, 0, "the Game folder should hold nothing but what the player put there");
}

#[test]
fn test_binary_started_twice_on_the_same_ipc_port_says_already_running_and_exits_with_status_0() {
    let ipc_port = common::free_port().to_string();
    let ui_port = common::free_port();
    let args = |ui_port: u16| {
        vec!["--ui-port".to_string(), ui_port.to_string(), "--port".into(), ipc_port.clone(), "--no-browser".into()]
    };
    let mut first = UiProcess::spawn(&args(ui_port), &[], std::path::Path::new(""));
    let Some(_) = first.launch_url() else {
        return;
    };
    // The game is connected to the first, so it stays.
    let mut dll = common::FakeDll::connect(ipc_port.parse().unwrap()).unwrap();
    dll.handshake();
    let mut second = UiProcess::spawn(&args(ui_port), &[], std::path::Path::new(""));

    let exit = second.wait_for_exit();

    assert!(exit.status.success(), "expected status 0, got {:?}: {}", exit.status, exit.stderr);
    assert!(
        exit.stdout.contains("datalink-mp is already running"),
        "stdout was {:?}, stderr {:?}",
        exit.stdout,
        exit.stderr
    );
    assert!(!exit.stdout.contains("?t="), "no token is printed by the second process");
}

#[test]
fn test_binary_started_again_replaces_an_idle_running_one() {
    let ipc_port = common::free_port().to_string();
    let ui_port = common::free_port();
    let args = vec![
        "--ui-port".to_string(),
        ui_port.to_string(),
        "--port".into(),
        ipc_port.clone(),
        "--no-browser".into(),
    ];
    let mut first = UiProcess::spawn(&args, &[], std::path::Path::new(""));
    let Some(_) = first.launch_url() else {
        return;
    };

    let mut second = UiProcess::spawn(&args, &[], std::path::Path::new(""));

    assert!(second.launch_url().is_some(), "the second Helper starts and prints its page");
    let exit = first.wait_for_exit();
    assert!(exit.status.success(), "the first made way and exited with {:?}: {}", exit.status, exit.stderr);
    common::FakeDll::connect(ipc_port.parse().unwrap())
        .expect("the second Helper serves the game")
        .handshake();
}

#[test]
fn test_helper_that_could_not_bind_the_ipc_port_does_not_claim_it_and_is_not_found_by_a_second_start() {
    let Some((first, holder)) = start_with_the_ipc_port_taken() else {
        return;
    };
    let taken_port = holder.local_addr().unwrap().port();

    let instance = http_get(first.ui_port(), "/api/instance", &[]).json();
    assert_eq!(instance["ipc_port"], serde_json::Value::Null, "it does not hold the port");

    let Some(second) = try_start_on(taken_port, first.ui_port()) else {
        return;
    };
    let second = second.expect("a Helper that does not hold the port is not 'already running'");
    assert_eq!(second.banners(), ["ipc_port_in_use"]);
    assert_eq!(first.opened.lock().unwrap().len(), 0, "the first Helper was not asked to show");
}

// Ticket 11: step 2, joining a friend's Ticket from the page
//
// The player pastes a friend's Ticket and presses Connect: the page sends it
// to `POST /api/join`. The friend is a second real Transport on loopback; what
// the page would show is read from the join response and from status.
// ---------------------------------------------------------------------------

impl Started {
    /// Send `ticket` to `POST /api/join` the way the page's Connect button does.
    fn join(&self, ticket: &str) -> common::HttpResponse {
        let body = serde_json::json!({ "ticket": ticket }).to_string();
        common::http_request_with_body(
            self.ui_port(),
            "POST",
            "/api/join",
            &[("X-Token", TOKEN), ("Content-Type", "application/json")],
            &body,
        )
    }
}

#[test]
fn test_join_with_text_that_is_not_a_ticket_is_refused_with_the_invalid_ticket_banner() {
    let Some(started) = start(false) else {
        return;
    };

    let response = started.join("hello, this is my ticket");

    assert_eq!(response.status, 400, "a bad Ticket should be refused: {}", response.body);
    assert_eq!(response.json()["banner"], "invalid_ticket");
    assert_eq!(response.json()["reason"], "not_a_ticket");
    assert_eq!(started.banners(), ["invalid_ticket"]);
    let status = started.status();
    assert_eq!(status["invalid_ticket"], "not_a_ticket", "the page words the banner by this");
    assert_eq!(status["state"], "ready");
}

#[test]
fn test_join_with_our_own_ticket_is_refused_as_our_own() {
    let Some(started) = start(false) else {
        return;
    };

    let response = started.join(&started.ticket());

    assert_eq!(response.status, 400, "our own Ticket should be refused: {}", response.body);
    assert_eq!(response.json()["banner"], "invalid_ticket");
    assert_eq!(response.json()["reason"], "own_ticket");
    assert_eq!(started.banners(), ["invalid_ticket"]);
    let status = started.status();
    assert_eq!(status["invalid_ticket"], "own_ticket", "the page words the banner by this");
    assert_eq!(status["state"], "ready");
}

/// Assert that a join was taken up: the dial has started, and the response
/// carries these warnings.
fn assert_dial_started(response: &common::HttpResponse, warnings: &[&str]) {
    assert_eq!(response.status, 202, "the dial should start: {}", response.body);
    assert_eq!(response.json()["warnings"], serde_json::json!(warnings));
}

#[test]
fn test_joining_a_friend_makes_the_state_joined_and_each_lists_the_other() {
    let Some(started) = start(false) else {
        return;
    };
    let Some(friend) = friend() else {
        return;
    };

    let response = started.join(friend.our_ticket());

    // The dial runs off the HTTP server's async threads: made on one of them,
    // it would panic there (a runtime inside a runtime), and a release build
    // would abort. The Helper would then not be answering at all.
    assert_dial_started(&response, &[]);
    started.wait_for_state("joined");
    started.wait_for_peers(&[short_id(&friend)]);
    wait_until_the_friend_lists_the_helper(&friend, &started);
    assert_eq!(started.banners(), Vec::<String>::new());
}

/// The dial timeout of a Helper whose dials are meant to fail: short, to keep
/// the tests short, and long enough to see the dial in progress.
const SHORT_DIAL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

/// Start a Helper whose dials give up after [`SHORT_DIAL_TIMEOUT`].
fn start_with_a_short_dial_timeout() -> Option<Started> {
    start_with_transport_options(TransportOptions {
        dial_timeout: SHORT_DIAL_TIMEOUT,
        ..TransportOptions::default()
    })
}

/// Start a Helper on free ports whose Transport is built with `transport_options`.
fn start_with_transport_options(transport_options: TransportOptions) -> Option<Started> {
    let config = Config {
        game_folder: game_folder_that_passes(),
        ipc_port: 0,
        transport_options,
        ui: Some(UiConfig {
            port: 0,
            token: TOKEN.to_string(),
            browser_opener: None,
        }),
    };
    start_with(config, OpenedUrls::default())
}

/// A Ticket for a Helper that never answers: a new endpoint ID, at a loopback
/// address where nothing speaks QUIC. A dial to it lasts until its timeout.
/// The address stays silent for as long as the socket is kept.
fn silent_ticket() -> (UdpSocket, String) {
    let silent = UdpSocket::bind("127.0.0.1:0").expect("should bind a loopback UDP port");
    let addr = silent.local_addr().expect("bound socket should have an address");
    let id = iroh::SecretKey::generate().public();
    let ticket = Ticket::new(iroh::EndpointAddr::new(id).with_ip_addr(addr));
    (silent, ticket.serialize())
}

#[test]
fn test_state_is_joining_while_the_dial_is_in_progress_and_a_second_join_is_refused() {
    let Some(started) = start_with_a_short_dial_timeout() else {
        return;
    };
    let (_silent, ticket) = silent_ticket();

    assert_dial_started(&started.join(&ticket), &[]);
    assert_eq!(started.status()["state"], "joining");

    // Only one dial from the page at a time.
    let second = started.join(&ticket);
    assert_eq!(second.status, 409, "a second join should be refused: {}", second.body);
    assert_eq!(started.status()["state"], "joining");
}

#[test]
fn test_state_returns_to_ready_when_the_dial_fails() {
    let Some(started) = start_with_a_short_dial_timeout() else {
        return;
    };
    let (_silent, ticket) = silent_ticket();

    assert_dial_started(&started.join(&ticket), &[]);

    // Past the dial timeout, with room to spare.
    common::poll_until(SHORT_DIAL_TIMEOUT * 3, || (started.status()["state"] == "ready").then_some(()))
        .unwrap_or_else(|| panic!("the failed dial should leave the state {}", started.status()["state"]));
    assert_eq!(started.status()["peers"], serde_json::json!([]));
}

#[test]
fn test_ticket_padded_with_whitespace_and_a_newline_is_accepted() {
    let Some(started) = start(false) else {
        return;
    };
    let Some(friend) = friend() else {
        return;
    };

    let response = started.join(&format!("  \t{}\r\n", friend.our_ticket()));

    assert_dial_started(&response, &[]);
    started.wait_for_state("joined");
}

#[test]
fn test_ticket_with_no_addresses_gets_a_warning_and_is_dialled_all_the_same() {
    let Some(started) = start_with_a_short_dial_timeout() else {
        return;
    };
    // A Helper that exists nowhere, named without an address.
    let id = iroh::SecretKey::generate().public();
    let ticket = Ticket::new(iroh::EndpointAddr::new(id)).serialize();

    let response = started.join(&ticket);

    assert_dial_started(&response, &["ticket_no_addresses"]);
    assert_eq!(started.banners(), Vec::<String>::new(), "the Ticket is not an invalid one");
    // The dial fails, the Helper having nowhere to find it, and the state
    // returns to ready. (How the page is told the dial failed is not this
    // test's business.)
    common::poll_until(SHORT_DIAL_TIMEOUT * 3, || (started.status()["state"] == "ready").then_some(()))
        .unwrap_or_else(|| panic!("the dial should end, the state is {}", started.status()["state"]));
}

#[test]
fn test_next_join_attempt_clears_the_invalid_ticket_banner() {
    let Some(started) = start(false) else {
        return;
    };
    let Some(friend) = friend() else {
        return;
    };
    assert_eq!(started.join("not a ticket").status, 400);
    assert_eq!(started.banners(), ["invalid_ticket"]);

    assert_dial_started(&started.join(friend.our_ticket()), &[]);

    assert_eq!(started.banners(), Vec::<String>::new());
    assert!(started.status()["invalid_ticket"].is_null());
}

#[test]
fn test_invalid_ticket_banner_says_which_case_the_latest_join_was() {
    let Some(started) = start(false) else {
        return;
    };
    assert_eq!(started.join("not a ticket").status, 400);

    assert_eq!(started.join(&started.ticket()).status, 400);

    assert_eq!(started.banners(), ["invalid_ticket"]);
    assert_eq!(started.status()["invalid_ticket"], "own_ticket");
}

#[test]
fn test_state_returns_to_ready_when_the_joined_friend_goes_away() {
    let Some(started) = start(false) else {
        return;
    };
    let Some(friend) = friend() else {
        return;
    };
    started.join(friend.our_ticket());
    started.wait_for_state("joined");

    // The friend quit their Helper.
    friend.shutdown();

    started.wait_for_peers(&[]);
    assert_eq!(started.status()["state"], "ready");
}

#[test]
fn test_friend_dialling_in_after_a_joined_friend_went_away_makes_the_state_hosting() {
    let Some(started) = start(false) else {
        return;
    };
    let (Some(joined), Some(dialling_in)) = (friend(), friend()) else {
        return;
    };
    started.join(joined.our_ticket());
    started.wait_for_state("joined");
    joined.shutdown();
    started.wait_for_state("ready");

    dial_the_helper(&dialling_in, &started);

    // Our dial was to the friend who left: this one dialled us.
    started.wait_for_state("hosting");
}

#[test]
fn test_join_is_refused_without_the_token_and_from_a_foreign_origin() {
    let Some(started) = start(false) else {
        return;
    };
    let Some(friend) = friend() else {
        return;
    };
    let port = started.ui_port();
    let body = serde_json::json!({ "ticket": friend.our_ticket() }).to_string();
    let json = ("Content-Type", "application/json");
    let send = |headers: &[(&str, &str)]| {
        common::http_request_with_body(port, "POST", "/api/join", headers, &body)
    };

    let no_token = send(&[json]);
    assert_eq!(no_token.status, 403, "a join without the token should be refused");
    let wrong_token = send(&[json, ("X-Token", "0000")]);
    assert_eq!(wrong_token.status, 403, "a join with a wrong token should be refused");
    let foreign = send(&[json, ("X-Token", TOKEN), ("Origin", "http://rebind.example")]);
    assert_eq!(foreign.status, 403, "a join from a foreign Origin should be refused");

    // None of them started a dial.
    assert_eq!(started.status()["state"], "ready");
    assert_eq!(friend.connected_peers(), Vec::new());
}

#[test]
fn test_fresh_helper_has_no_invalid_ticket_reason() {
    let Some(started) = start(false) else {
        return;
    };

    assert!(started.status()["invalid_ticket"].is_null());
}

// ---------------------------------------------------------------------------
// Ticket 12: dial failure banners
//
// A join fails because the friend's Helper can't be reached, or because it
// runs another release: either way the joiner gets a banner saying which,
// whether the join came from the page or from the game's DLL. The friends are
// real Transports on loopback, one of them built with another Peer protocol
// version; what the page would show is read from status.
// ---------------------------------------------------------------------------

/// The options of a Transport from another release: it speaks another Peer
/// protocol version, so this build's Helpers refuse it, and it them.
fn options_of_another_release() -> TransportOptions {
    TransportOptions {
        peer_protocol_version: iroh_transport::STREAM_PROTO_VERSION + 1,
        ..TransportOptions::default()
    }
}

/// A friend's Transport from another release. None when one cannot be
/// created (sandboxed environments); the test then returns early.
fn friend_of_another_release() -> Option<iroh_transport::Transport> {
    match iroh_transport::Transport::with_options(options_of_another_release()) {
        Ok(friend) => Some(friend),
        Err(e) => {
            note_transport_unavailable(&e);
            None
        }
    }
}

/// The Ticket of a friend whose Helper has since shut down, the way a Ticket
/// goes stale when its Helper quits. None when no friend can be created.
fn ticket_of_a_friend_who_shut_down() -> Option<String> {
    let friend = friend()?;
    let ticket = friend.our_ticket().to_string();
    friend.shutdown();
    Some(ticket)
}

impl Started {
    /// Wait for the dial a join started to end, whichever way it went, and
    /// return the banners status reports then. The dial gives up after
    /// [`SHORT_DIAL_TIMEOUT`] at the latest.
    fn wait_for_the_dial_to_end(&self) -> Vec<String> {
        common::poll_until(SHORT_DIAL_TIMEOUT * 3, || (self.status()["state"] != "joining").then_some(()))
            .expect("the dial should end within its timeout");
        self.banners()
    }

    /// Ask the Helper to join `ticket` from a fake DLL, the way the game does
    /// when the player picks Join Game, and return the reply.
    fn join_from_the_game(&self, ticket: &str) -> ipc_protocol::IpcResponse {
        let mut dll = self.connect_fake_dll();
        dll.handshake();
        dll.request(&ipc_protocol::IpcRequest::JoinSessionByTicket {
            host_ticket: ticket.to_string(),
        })
    }
}

/// Assert that the game's join was answered with an error, and return its message.
fn join_failed(reply: ipc_protocol::IpcResponse) -> String {
    match reply {
        ipc_protocol::IpcResponse::Error { message } => message,
        other => panic!("the join should fail, got {other:?}"),
    }
}

#[test]
fn test_join_to_a_ticket_whose_helper_shut_down_gives_cant_reach_host_and_ready() {
    let Some(started) = start_with_a_short_dial_timeout() else {
        return;
    };
    let Some(ticket) = ticket_of_a_friend_who_shut_down() else {
        return;
    };

    assert_dial_started(&started.join(&ticket), &[]);

    // Within the shortened timeout: the deadline is well below the 15 s default.
    assert_eq!(started.wait_for_the_dial_to_end(), ["cant_reach_host"]);
    assert_eq!(started.status()["state"], "ready");
}

#[test]
fn test_join_whose_dial_times_out_gives_cant_reach_host() {
    let Some(started) = start_with_a_short_dial_timeout() else {
        return;
    };
    // Nothing answers there, so the dial lasts until its timeout instead of
    // failing at once.
    let (_silent, ticket) = silent_ticket();

    assert_dial_started(&started.join(&ticket), &[]);

    assert_eq!(started.wait_for_the_dial_to_end(), ["cant_reach_host"]);
    assert_eq!(started.status()["state"], "ready");
}

#[test]
fn test_join_to_a_friend_of_another_release_gives_peer_version_mismatch() {
    let Some(started) = start_with_a_short_dial_timeout() else {
        return;
    };
    let Some(friend) = friend_of_another_release() else {
        return;
    };

    assert_dial_started(&started.join(friend.our_ticket()), &[]);

    // Not cant_reach_host: the friend was reached, and refused us.
    assert_eq!(started.wait_for_the_dial_to_end(), ["peer_version_mismatch"]);
    assert_eq!(started.status()["state"], "ready");
}

#[test]
fn test_game_join_to_an_unreachable_ticket_sets_cant_reach_host() {
    let Some(started) = start_with_a_short_dial_timeout() else {
        return;
    };
    let Some(ticket) = ticket_of_a_friend_who_shut_down() else {
        return;
    };

    join_failed(started.join_from_the_game(&ticket));

    // The reply comes once the join is over, so its banner is up by now.
    assert_eq!(started.banners(), ["cant_reach_host"]);
}

#[test]
fn test_game_join_to_a_friend_of_another_release_sets_peer_version_mismatch() {
    let Some(started) = start_with_a_short_dial_timeout() else {
        return;
    };
    let Some(friend) = friend_of_another_release() else {
        return;
    };

    join_failed(started.join_from_the_game(friend.our_ticket()));

    assert_eq!(started.banners(), ["peer_version_mismatch"]);
}

#[test]
fn test_game_join_to_a_host_with_no_session_sets_no_banner() {
    let Some(started) = start_with_a_short_dial_timeout() else {
        return;
    };
    // Reachable and of this release, but hosting no game.
    let Some(friend) = friend() else {
        return;
    };

    let message = join_failed(started.join_from_the_game(friend.our_ticket()));

    assert!(message.contains("SessionNotFound"), "the join should fail for the missing session: {message}");
    assert_eq!(started.banners(), Vec::<String>::new());
}

#[test]
fn test_later_join_attempt_clears_both_banners() {
    let Some(started) = start_with_a_short_dial_timeout() else {
        return;
    };
    let (Some(of_another_release), Some(ticket_that_is_stale), Some(friend)) =
        (friend_of_another_release(), ticket_of_a_friend_who_shut_down(), friend())
    else {
        return;
    };
    started.join(of_another_release.our_ticket());
    assert_eq!(started.wait_for_the_dial_to_end(), ["peer_version_mismatch"]);

    assert_dial_started(&started.join(&ticket_that_is_stale), &[]);
    assert_eq!(started.banners(), Vec::<String>::new(), "the join attempt clears the banner at once");
    assert_eq!(started.wait_for_the_dial_to_end(), ["cant_reach_host"]);

    assert_dial_started(&started.join(friend.our_ticket()), &[]);
    assert_eq!(started.banners(), Vec::<String>::new());
    started.wait_for_state("joined");
    assert_eq!(started.banners(), Vec::<String>::new());
}

#[test]
fn test_game_join_attempt_clears_the_banner_of_the_join_before_it() {
    let Some(started) = start_with_a_short_dial_timeout() else {
        return;
    };
    let (Some(of_another_release), Some(ticket_that_is_stale), Some(friend)) =
        (friend_of_another_release(), ticket_of_a_friend_who_shut_down(), friend())
    else {
        return;
    };
    started.join(of_another_release.our_ticket());
    assert_eq!(started.wait_for_the_dial_to_end(), ["peer_version_mismatch"]);

    join_failed(started.join_from_the_game(&ticket_that_is_stale));
    assert_eq!(started.banners(), ["cant_reach_host"]);

    // Fails too, for no reason a banner speaks of.
    join_failed(started.join_from_the_game(friend.our_ticket()));
    assert_eq!(started.banners(), Vec::<String>::new());
}

#[test]
fn test_helper_that_refused_a_mismatched_dial_reports_no_banner_of_its_own() {
    let Some(joiner) = start_with_a_short_dial_timeout() else {
        return;
    };
    let Some(refusing) = start_with_transport_options(options_of_another_release()) else {
        return;
    };

    joiner.join(&refusing.ticket());
    assert_eq!(joiner.wait_for_the_dial_to_end(), ["peer_version_mismatch"]);

    assert_eq!(refusing.banners(), Vec::<String>::new());
    let status = refusing.status();
    assert_eq!(status["state"], "ready");
    assert_eq!(status["peers"], serde_json::json!([]));
}

/// The words of the banner element for `code` on the page: its text with the
/// markup taken out and runs of white space made one space.
fn banner_words(page: &str, code: &str) -> String {
    let attribute = format!("data-banner=\"{code}\"");
    let start = page.find(&attribute).unwrap_or_else(|| panic!("the page should have the {code} banner"));
    let element = &page[start..];
    let element = &element[element.find('>').expect("the banner's tag should close") + 1..];
    let element = &element[..element.find("</div>").expect("the banner should end")];
    let mut text = String::new();
    let mut in_tag = false;
    for c in element.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => text.push(c),
            _ => {}
        }
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn test_page_has_the_words_of_both_dial_failure_banners_under_the_header() {
    let Some(started) = start(false) else {
        return;
    };
    let page = http_get(started.ui_port(), "/", &[]).body;

    assert_eq!(
        banner_words(&page, "cant_reach_host"),
        "Couldn't reach your friend's Helper. Ask them for their current Ticket: it changes \
         every time they start the Helper or press Stop."
    );
    assert_eq!(
        banner_words(&page, "peer_version_mismatch"),
        "Your friend has a different release of datalink-mp. You both need the same one."
    );
    // Under the header, with the page's other banners, before the steps.
    let header_end = page.find("</h1>").expect("the page should have its header");
    let steps = page.find("class=\"card\"").expect("the page should have its steps");
    for code in ["cant_reach_host", "peer_version_mismatch"] {
        let at = page.find(&format!("data-banner=\"{code}\"")).unwrap();
        assert!(header_end < at && at < steps, "the {code} banner should sit under the header");
    }
}

// ---------------------------------------------------------------------------
// Ticket 13: Stop
//
// A player in a session presses Stop: the page sends `POST /api/stop`, and the
// Helper ends its connections and comes back with a new Ticket, without
// quitting. The friends are real Transports on loopback and the game is a fake
// DLL; what the page would show is read from the Stop response and status.
// ---------------------------------------------------------------------------

impl Started {
    /// Press Stop the way the page does, and assert it was answered with the
    /// new Ticket in place. Returns the new Ticket's sequence number.
    fn stop(&self) -> u64 {
        let response = post(self, "/api/stop", &[]);
        assert_eq!(response.status, 200, "Stop should be answered once it is done: {}", response.body);
        let ticket_seq = response.json()["ticket_seq"]
            .as_u64()
            .unwrap_or_else(|| panic!("the answer should carry the new sequence number: {}", response.body));
        assert_eq!(
            self.status()["ticket_seq"], ticket_seq,
            "the new Ticket should be in place when Stop is answered"
        );
        ticket_seq
    }
}

/// Start a Helper with a friend's Helper connected to it: a player hosting.
/// None when a Transport cannot be created.
fn start_hosting_a_friend() -> Option<(Started, iroh_transport::Transport)> {
    let started = start(false)?;
    let friend = friend()?;
    dial_the_helper(&friend, &started);
    wait_until_the_friend_lists_the_helper(&friend, &started);
    started.wait_for_state("hosting");
    Some((started, friend))
}

#[test]
fn test_stop_gives_a_new_ticket_with_the_next_sequence_number_and_the_state_ready() {
    let Some((started, _friend)) = start_hosting_a_friend() else {
        return;
    };
    let before = started.ticket();

    // Answered at all: Stop made a Transport, shut one down and dropped it,
    // each of which panics on the HTTP server's async threads.
    assert_eq!(started.stop(), 2);

    let status = started.status();
    let after = status["ticket"].as_str().expect("status should carry a Ticket");
    assert_ne!(after, before, "Stop should give a new Ticket");
    Ticket::parse(after).expect("the new Ticket should parse");
    assert_eq!(status["state"], "ready");
    assert_eq!(status["peers"], serde_json::json!([]));
}

#[test]
fn test_each_stop_puts_the_sequence_number_up_by_one() {
    let Some(started) = start(false) else {
        return;
    };
    let first = started.ticket();

    assert_eq!(started.stop(), 2);
    let second = started.ticket();
    assert_eq!(started.stop(), 3);

    let third = started.ticket();
    assert!(first != second && second != third && first != third, "each Stop should give a new Ticket");
}

#[test]
fn test_connected_friend_sees_the_connection_close_promptly_after_stop() {
    let Some((started, friend)) = start_hosting_a_friend() else {
        return;
    };

    started.stop();

    // The old Transport was closed for the friend, not left to time out.
    let closed = common::poll_until(PEER_NOTICE_DEADLINE, || {
        friend.connected_peers().is_empty().then_some(())
    });
    assert!(
        closed.is_some(),
        "the friend should see the close within {PEER_NOTICE_DEADLINE:?}, still lists {:?}",
        friend.connected_peers()
    );
}

#[test]
fn test_stop_clears_a_dial_failure_banner() {
    let Some(started) = start_with_a_short_dial_timeout() else {
        return;
    };
    let Some(of_another_release) = friend_of_another_release() else {
        return;
    };
    started.join(of_another_release.our_ticket());
    assert_eq!(started.wait_for_the_dial_to_end(), ["peer_version_mismatch"]);

    started.stop();

    assert_eq!(started.banners(), Vec::<String>::new());
}

#[test]
fn test_stop_clears_the_invalid_ticket_banner() {
    let Some(started) = start(false) else {
        return;
    };
    assert_eq!(started.join("not a ticket").status, 400);
    assert_eq!(started.banners(), ["invalid_ticket"]);

    started.stop();

    assert_eq!(started.banners(), Vec::<String>::new());
    assert!(started.status()["invalid_ticket"].is_null());
}

#[test]
fn test_stop_clears_the_banner_of_the_games_join() {
    let Some(started) = start_with_a_short_dial_timeout() else {
        return;
    };
    let Some(ticket) = ticket_of_a_friend_who_shut_down() else {
        return;
    };
    join_failed(started.join_from_the_game(&ticket));
    assert_eq!(started.banners(), ["cant_reach_host"]);

    started.stop();

    assert_eq!(started.banners(), Vec::<String>::new());
}

#[test]
fn test_stop_leaves_the_condition_banners_alone() {
    let Some(started) = start(false) else {
        return;
    };
    let mut dll = started.connect_fake_dll();
    dll.handshake_with_version(ipc_protocol::PROTOCOL_VERSION + 1);
    started.wait_for_banners(&["ipc_version_mismatch"]);

    started.stop();

    assert_eq!(started.banners(), ["ipc_version_mismatch"]);
}

#[test]
fn test_stop_during_a_dial_ends_it_with_no_banner_and_the_state_ready() {
    let Some(started) = start_with_a_short_dial_timeout() else {
        return;
    };
    // The dial would last until its timeout and then put up cant_reach_host.
    let (_silent, ticket) = silent_ticket();
    assert_dial_started(&started.join(&ticket), &[]);

    started.stop();

    // The dial was on the Transport Stop replaced: how it ended is no news
    // to the player, who pressed Stop.
    assert_eq!(started.wait_for_the_dial_to_end(), Vec::<String>::new());
    assert_eq!(started.status()["state"], "ready");
}

#[test]
fn test_games_link_survives_stop_and_its_requests_are_answered_by_the_new_transport() {
    let Some((started, _friend)) = start_hosting_a_friend() else {
        return;
    };
    // The game is open, and its DLL connected before Stop.
    let mut dll = started.connect_fake_dll();
    let (_, old_ticket) = dll.handshake();
    assert_eq!(started.status()["game_connected"], true);

    started.stop();

    let answer = dll.request(&ipc_protocol::IpcRequest::GetOurTicket);
    let ipc_protocol::IpcResponse::StringValue { value: ticket } = answer else {
        panic!("the game's request after Stop should be answered, got {answer:?}");
    };
    assert_ne!(ticket, old_ticket, "the game should be talking to the new Transport");
    assert_eq!(ticket, started.ticket());
    assert_eq!(started.status()["game_connected"], true);
}

#[test]
fn test_friend_can_dial_the_new_ticket_after_stop() {
    let Some((started, _old_friend)) = start_hosting_a_friend() else {
        return;
    };
    let Some(friend) = friend() else {
        return;
    };

    started.stop();
    dial_the_helper(&friend, &started);

    started.wait_for_state("hosting");
    started.wait_for_peers(&[short_id(&friend)]);
    wait_until_the_friend_lists_the_helper(&friend, &started);
}

#[test]
fn test_join_from_the_page_after_stop_works() {
    let Some((started, _hosted)) = start_hosting_a_friend() else {
        return;
    };
    let Some(friend) = friend() else {
        return;
    };

    started.stop();
    assert_dial_started(&started.join(friend.our_ticket()), &[]);

    started.wait_for_state("joined");
    started.wait_for_peers(&[short_id(&friend)]);
}

#[test]
fn test_stop_from_the_page_leaves_the_helper_running_and_it_still_quits_cleanly() {
    let Some((started, _friend)) = start_hosting_a_friend() else {
        return;
    };
    let mut dll = started.connect_fake_dll();
    dll.handshake();

    started.stop();

    // A panic on one of the HTTP server's async threads would abort a release
    // build. Here it would have cut off the answer, or the server: the
    // Helper still answers the page and the game, and still quits cleanly.
    assert_eq!(started.status()["state"], "ready");
    dll.handshake();
    quit(&started);
    wait_for_the_helper_to_finish(started.helper);
}

#[test]
fn test_stop_is_refused_without_the_token_from_a_foreign_origin_and_by_get() {
    let Some((started, friend)) = start_hosting_a_friend() else {
        return;
    };
    let port = started.ui_port();
    let ticket = started.ticket();
    let json = ("Content-Type", "application/json");

    let no_token = common::http_request(port, "POST", "/api/stop", &[json]);
    let wrong_token =
        common::http_request(port, "POST", "/api/stop", &[json, ("X-Token", "not-the-token")]);
    let foreign_origin = post(&started, "/api/stop", &[("Origin", "http://rebind.example")]);
    let by_get = started.get_api("/api/stop");

    assert_eq!(no_token.status, 403);
    assert_eq!(wrong_token.status, 403);
    assert_eq!(foreign_origin.status, 403);
    assert_eq!(by_get.status, 405);
    // None of them reached Stop.
    let status = started.status();
    assert_eq!(status["ticket"], ticket.as_str());
    assert_eq!(status["ticket_seq"], 1);
    assert_eq!(status["state"], "hosting");
    assert_eq!(friend.connected_peers().len(), 1);
}

#[test]
fn test_page_has_both_wordings_of_the_not_game_folder_banner() {
    let Some(started) = start(false) else {
        return;
    };
    let page = http_get(started.ui_port(), "/", &[]).body;

    // The Helper only says the banner is active; the page picks the wording
    // from the self-check, like the invalid-Ticket words.
    let banner = &page[page.find("data-banner=\"not_game_folder\"").expect("the page should have the banner")..];
    let banner = &banner[..banner.find("datalink-mp is running from").expect("the banner should say where it runs")];
    let wrong = banner.find("data-folder-words=\"wrong_folder\"").expect("a wrong-folder wording");
    let dll = banner.find("data-folder-words=\"dll_missing\"").expect("a DLL-missing wording");
    let (wrong_words, dll_words) = if wrong < dll {
        (&banner[wrong..dll], &banner[dll..])
    } else {
        (&banner[wrong..], &banner[dll..wrong])
    };
    assert!(wrong_words.contains("This isn't your Game folder."));
    assert!(dll_words.contains("This looks like your Game folder"));
    assert!(dll_words.contains("only <code>dplayx.dll</code> is missing"));
    assert!(dll_words.contains("antivirus"));
    assert!(
        !dll_words.contains("This isn't your Game folder"),
        "the DLL-missing wording should not say the folder is wrong"
    );
}

#[test]
fn test_page_has_stop_hidden_as_it_loads_and_the_words_for_after_stop() {
    let Some(started) = start(false) else {
        return;
    };
    let page = http_get(started.ui_port(), "/", &[]).body;

    // In the header, beside Quit, and hidden until status says the Helper is
    // in a session. Which states show it is page logic, checked by hand.
    let header = &page[..page.find("data-banner").expect("the page should have its banners")];
    let stop = header.find("id=\"stop\"").expect("the header should have a Stop button");
    let tag = &header[header[..stop].rfind('<').unwrap()..];
    let tag = &tag[..=tag.find('>').unwrap()];
    assert!(tag.contains(" hidden"), "Stop should be hidden as the page loads: {tag}");
    assert!(header.contains("id=\"quit\""));
    assert!(page.contains("Your Ticket changed. Share it again."));
    assert!(
        page.contains("Return to the game\\'s main menu first"),
        "Stop should ask the player to return to the game's main menu first"
    );
    assert!(
        page.contains("can crash their game"),
        "Stop should say what stopping during a game can do"
    );
}

#[test]
fn test_page_has_a_hidden_connected_helpers_line_under_the_hosts_ticket() {
    let Some(started) = start(false) else {
        return;
    };
    let page = http_get(started.ui_port(), "/", &[]).body;

    // In the Hosting column, under Copy Ticket, and hidden until status says
    // the Helper is hosting with friends connected. Which states show it is
    // page logic, checked by hand.
    let copy = page.find("id=\"copy\"").expect("step 2 should have the Copy Ticket button");
    let joining = page.find("<b>Joining:</b>").expect("step 2 should have the Joining column");
    let hosting_column = &page[copy..joining];
    let line = hosting_column
        .find("id=\"hosting-peers\"")
        .expect("the Hosting column should have a connected Helpers line after Copy Ticket");
    let tag = &hosting_column[hosting_column[..line].rfind('<').unwrap()..];
    let tag = &tag[..=tag.find('>').unwrap()];
    assert!(tag.contains(" hidden"), "the line should be hidden as the page loads: {tag}");
}

#[test]
fn test_page_tells_joiners_to_click_make_ready_for_simultaneous_moves() {
    let Some(started) = start(false) else {
        return;
    };
    let page = http_get(started.ui_port(), "/", &[]).body;

    let hint = &page[page.find("id=\"menu-hint-joined\"").expect("step 3 should have the Joiner's hint")..];
    let hint = &hint[..hint.find("</div>").unwrap()];
    assert!(
        hint.contains("Simultaneous Moves") && hint.contains("<b>Make Ready</b>"),
        "the Joiner's hint should say to click Make Ready for Simultaneous Moves: {hint}"
    );
}
