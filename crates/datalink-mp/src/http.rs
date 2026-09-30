//! HTTP server: the embedded page and the JSON API, on 127.0.0.1 only.
//!
//! It runs on its own tokio runtime, owned by the [`HttpServer`], so the rest
//! of the Helper stays synchronous. Handlers use only non-blocking reads of
//! the Session controller; its blocking calls go through `spawn_blocking`.

mod guard;

use crate::controller::{Banner, SessionController, Status};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router, ServiceExt};
use serde::Deserialize;
use serde_json::json;
use std::net::TcpListener;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::{Builder, Runtime};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tracing::info;

/// The page, embedded in the binary.
const PAGE: &str = include_str!("page.html");

/// How long each part of stopping the HTTP server may take: answering the
/// requests it has taken up, then ending its runtime.
const STOP_TIMEOUT: Duration = Duration::from_secs(1);

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
}

/// Serve the page and the API on `listener`.
pub(crate) fn spawn(
    listener: TcpListener,
    controller: Arc<SessionController>,
    token: String,
) -> std::io::Result<HttpServer> {
    let port = listener.local_addr()?.port();
    listener.set_nonblocking(true)?;

    let runtime = Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("datalink-mp-http")
        .enable_all()
        .build()?;

    let state = Arc::new(AppState { controller });
    let api = Router::new()
        .route("/status", get(status))
        .route("/join", post(join))
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

impl HttpServer {
    pub(crate) fn port(&self) -> u16 {
        self.port
    }

    pub(crate) fn launch_url(&self) -> String {
        format!("http://127.0.0.1:{}/?t={}", self.port, self.token)
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

/// The body of `POST /api/join`: the text the player pasted as a friend's Ticket.
#[derive(Deserialize)]
struct JoinRequest {
    ticket: String,
}

/// Join a friend's Ticket: check it, start the dial and answer at once. How
/// the dial ends is for status to say.
///
/// - 202 with the list of `warnings` when the dial has started;
/// - 400 with the `invalid_ticket` banner and why, when the text is not a
///   Ticket or is our own, which also puts the banner up;
/// - 409 when a dial is already in progress.
async fn join(State(state): State<Arc<AppState>>, Json(request): Json<JoinRequest>) -> Response {
    // Parsing and checking the Ticket waits on nothing, so it is done here.
    let pending = match state.controller.begin_join(&request.ticket) {
        Ok(pending) => pending,
        Err(refused) => {
            let (status, body) = match refused.invalid_ticket() {
                Some(reason) => (
                    StatusCode::BAD_REQUEST,
                    json!({ "banner": Banner::InvalidTicket, "reason": reason }),
                ),
                // The only refusal that puts no banner up.
                None => (StatusCode::CONFLICT, json!({ "error": "dial_in_progress" })),
            };
            return (status, Json(body)).into_response();
        }
    };
    let body = json!({ "warnings": pending.warnings() });
    let controller = state.controller.clone();
    // The dial blocks on the Transport's runtime, which panics on an async
    // worker thread like this one. The controller goes with it, so the
    // blocking thread is where this reference to it is dropped.
    tokio::task::spawn_blocking(move || {
        // Status reports how the dial went.
        let _ = controller.dial(pending);
    });
    (StatusCode::ACCEPTED, Json(body)).into_response()
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
