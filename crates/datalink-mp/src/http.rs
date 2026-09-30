//! HTTP server: the embedded page and the JSON API, on 127.0.0.1 only.
//!
//! It runs on its own tokio runtime, owned by the [`HttpServer`], so the rest
//! of the Helper stays synchronous. Handlers use only non-blocking reads of
//! the Session controller; its blocking calls go through `spawn_blocking`.

mod guard;

use crate::controller::{SessionController, Status};
use crate::BrowserOpener;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;
use axum::routing::{get, post};
use axum::{Json, Router, ServiceExt};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::runtime::{Builder, Runtime};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tracing::info;

/// The page, embedded in the binary.
const PAGE: &str = include_str!("page.html");

/// How long each part of stopping the HTTP server may take: answering the
/// requests it has taken up, then ending its runtime.
const STOP_TIMEOUT: Duration = Duration::from_secs(1);

/// The least time between two `POST /api/show` requests that are answered by
/// opening the browser. The route needs no token, so this keeps a local
/// program from opening tabs in a loop.
const SHOW_INTERVAL: Duration = Duration::from_secs(3);

/// Make a token: 32 bytes from the OS random source, hex-encoded.
pub fn generate_token() -> std::io::Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(std::io::Error::other)?;
    Ok(hex::encode(bytes))
}

/// How many ports the UI port walk tries: the chosen port and the nine after it.
pub(crate) const PORT_WALK_LEN: u16 = 10;

/// Why the whole UI port walk failed.
#[derive(Debug)]
pub(crate) struct PortWalkFailed {
    pub first: u16,
    pub last: u16,
    /// Why the last port could not be bound.
    pub source: std::io::Error,
}

/// Bind the listener for the page, walking up from `port` to the next free
/// one: `port` and the nine after it are tried in turn. Port 0 asks the OS
/// for a free port and needs no walk. Read the port actually bound from the
/// listener.
pub(crate) fn bind_walking(port: u16) -> Result<TcpListener, PortWalkFailed> {
    let last = port.saturating_add(PORT_WALK_LEN - 1);
    let mut failure = None;
    for candidate in port..=last {
        match TcpListener::bind(("127.0.0.1", candidate)) {
            Ok(listener) => return Ok(listener),
            // Not only AddrInUse: Windows refuses ports it has reserved with
            // a permission error, and the next port may well be fine.
            Err(e) => failure = Some(e),
        }
    }
    Err(PortWalkFailed {
        first: port,
        last,
        source: failure.expect("the walk tries at least one port"),
    })
}

/// A running HTTP server. Stops when dropped, which blocks: drop it on a
/// plain thread.
pub(crate) struct HttpServer {
    port: u16,
    token: String,
    stop: Option<oneshot::Sender<()>>,
    /// The task serving requests. It ends once the server was told to stop
    /// and has answered every request it had taken up.
    serve_task: Option<JoinHandle<()>>,
    runtime: Option<Runtime>,
}

struct AppState {
    controller: Arc<SessionController>,
    /// The application name, the Release version and the IPC port, as
    /// `GET /api/instance` answers them.
    instance: serde_json::Value,
    launch_url: String,
    browser_opener: Option<Arc<BrowserOpener>>,
    /// When the browser was last opened because a request asked for it.
    last_shown: Mutex<Option<Instant>>,
}

/// Serve the page and the API on `listener`.
pub(crate) fn spawn(
    listener: TcpListener,
    controller: Arc<SessionController>,
    ipc_port: u16,
    token: String,
    browser_opener: Option<Arc<BrowserOpener>>,
) -> std::io::Result<HttpServer> {
    let port = listener.local_addr()?.port();
    listener.set_nonblocking(true)?;

    let runtime = Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("datalink-mp-http")
        .enable_all()
        .build()?;

    let state = Arc::new(AppState {
        instance: serde_json::json!({
            "app": crate::APP_NAME,
            "release_version": env!("CARGO_PKG_VERSION"),
            "ipc_port": ipc_port,
        }),
        launch_url: launch_url(port, &token),
        browser_opener,
        last_shown: Mutex::new(None),
        controller,
    });
    let api = Router::new()
        .route("/status", get(status))
        .route("/instance", get(instance))
        .route("/show", post(show))
        .route("/quit", post(quit))
        .with_state(state);
    let app = Router::new().route("/", get(page)).nest("/api", api);
    // Every request passes the security rules, the token among them, before
    // it reaches any of the routes above. See `guard`.
    let app = guard::protect(app, guard::Guard::new(port, &token, PAGE));

    let (stop, stopped) = oneshot::channel::<()>();
    let listener = {
        let _enter = runtime.enter();
        tokio::net::TcpListener::from_std(listener)?
    };
    let serve_task = runtime.spawn(async move {
        let served = axum::serve(listener, app.into_make_service())
            .with_graceful_shutdown(async {
                let _ = stopped.await;
            })
            .await;
        if let Err(e) = served {
            tracing::error!("HTTP server error: {:?}", e);
        }
    });
    info!("Serving the page on 127.0.0.1:{}", port);

    Ok(HttpServer {
        port,
        token,
        stop: Some(stop),
        serve_task: Some(serve_task),
        runtime: Some(runtime),
    })
}

fn launch_url(port: u16, token: &str) -> String {
    format!("http://127.0.0.1:{port}/?t={token}")
}

impl HttpServer {
    pub(crate) fn port(&self) -> u16 {
        self.port
    }

    pub(crate) fn launch_url(&self) -> String {
        launch_url(self.port, &self.token)
    }
}

impl Drop for HttpServer {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        let Some(runtime) = self.runtime.take() else {
            return;
        };
        if let Some(serve_task) = self.serve_task.take() {
            // Ending the runtime cuts off a reply that is still on its way
            // out, and the reply to Quit is on its way out about now.
            let _ =
                runtime.block_on(async { tokio::time::timeout(STOP_TIMEOUT, serve_task).await });
        }
        runtime.shutdown_timeout(STOP_TIMEOUT);
    }
}

async fn page() -> Html<&'static str> {
    Html(PAGE)
}

async fn status(State(state): State<Arc<AppState>>) -> Result<Json<Status>, StatusCode> {
    let controller = state.controller.clone();
    // The snapshot reads the Game folder's list of files. A slow folder (a
    // network drive) must not hold up an async worker thread.
    tokio::task::spawn_blocking(move || controller.status())
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Quit: answer, and shut the Helper down. Whoever waits on the Helper (the
/// binary's `main`) ends the process once that is done.
async fn quit(State(state): State<Arc<AppState>>) -> StatusCode {
    let controller = state.controller.clone();
    // The shutdown blocks on the Transport's runtime, which panics on an
    // async worker thread like this one.
    tokio::task::spawn_blocking(move || controller.shutdown());
    StatusCode::NO_CONTENT
}

/// Who is answering: the way a second Helper on the same IPC port finds this one.
async fn instance(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(state.instance.clone())
}

/// Open the browser at this Helper's own launch URL, for a second Helper that
/// was started on the same IPC port. The token never leaves this process.
/// Answers 429 when it opened the browser less than [`SHOW_INTERVAL`] ago.
async fn show(State(state): State<Arc<AppState>>) -> StatusCode {
    {
        let mut last_shown = state.last_shown.lock().unwrap();
        let now = Instant::now();
        if last_shown.is_some_and(|last| now.duration_since(last) < SHOW_INTERVAL) {
            return StatusCode::TOO_MANY_REQUESTS;
        }
        *last_shown = Some(now);
    }
    if let Some(open) = state.browser_opener.clone() {
        // The opener may start a program: keep it off the async worker.
        let url = state.launch_url.clone();
        tokio::task::spawn_blocking(move || open(&url));
    }
    StatusCode::NO_CONTENT
}
