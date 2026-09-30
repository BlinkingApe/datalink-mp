//! HTTP server: the embedded page and the JSON API, on 127.0.0.1 only.
//!
//! It runs on its own tokio runtime, owned by the [`HttpServer`], so the rest
//! of the Helper stays synchronous. Handlers use only non-blocking reads of
//! the Session controller.

use crate::controller::{SessionController, Status};
use axum::extract::{Request, State};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{Html, Response};
use axum::routing::get;
use axum::{Json, Router};
use std::net::TcpListener;
use std::sync::Arc;
use std::time::Duration;
use subtle::ConstantTimeEq;
use tokio::runtime::{Builder, Runtime};
use tokio::sync::oneshot;
use tracing::info;

/// The page, embedded in the binary.
const PAGE: &str = include_str!("page.html");

/// The header that carries the token on API requests.
const TOKEN_HEADER: &str = "x-token";

/// How long the HTTP server may take to stop.
const STOP_TIMEOUT: Duration = Duration::from_secs(1);

/// Make a token: 32 bytes from the OS random source, hex-encoded.
pub fn generate_token() -> std::io::Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(std::io::Error::other)?;
    Ok(hex::encode(bytes))
}

/// Bind the listener for the page. Port 0 picks a free port.
pub(crate) fn bind(port: u16) -> std::io::Result<TcpListener> {
    TcpListener::bind(("127.0.0.1", port))
}

/// A running HTTP server. Stops when dropped.
pub(crate) struct HttpServer {
    port: u16,
    token: String,
    stop: Option<oneshot::Sender<()>>,
    runtime: Option<Runtime>,
}

struct AppState {
    controller: Arc<SessionController>,
    token: String,
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

    let state = Arc::new(AppState {
        controller,
        token: token.clone(),
    });
    let api = Router::new()
        .route("/status", get(status))
        .layer(middleware::from_fn_with_state(state.clone(), require_token))
        .with_state(state);
    let app = Router::new().route("/", get(page)).nest("/api", api);

    let (stop, stopped) = oneshot::channel::<()>();
    let listener = {
        let _enter = runtime.enter();
        tokio::net::TcpListener::from_std(listener)?
    };
    runtime.spawn(async move {
        let served = axum::serve(listener, app)
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
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_timeout(STOP_TIMEOUT);
        }
    }
}

/// Refuse API requests that do not carry the token.
async fn require_token(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let given = headers
        .get(TOKEN_HEADER)
        .map(|v| v.as_bytes())
        .unwrap_or_default();
    // Constant time in the token's bytes; a length difference is not a secret.
    if bool::from(given.ct_eq(state.token.as_bytes())) {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

async fn page() -> Html<&'static str> {
    Html(PAGE)
}

async fn status(State(state): State<Arc<AppState>>) -> Json<Status> {
    Json(state.controller.status())
}
