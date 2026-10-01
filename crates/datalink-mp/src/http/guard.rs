//! The security rules, applied to every request before routing.
//!
//! [`protect`] wraps the whole router, so a request reaches a route, the
//! fallback (404) or axum's 405 only after it has passed [`Guard::admit`], and
//! every response on the way out, rejections included, gets the same headers.
//! Routes added later are covered without doing anything: there is no
//! per-route check to forget. A route that must work without the token is
//! listed in [`PUBLIC_PATHS`]; every other path needs it.

use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::Router;
use data_encoding::BASE64;
use sha2::{Digest, Sha256};
use std::convert::Infallible;
use std::sync::Arc;
use subtle::ConstantTimeEq;
use tower::{Layer, Service};

/// The paths served without the token. They still get every other rule.
/// `/` is the page, which holds no secrets. The others are how a second
/// Helper on the same IPC port finds the running one and asks it to make way
/// or to show its page. They reveal no secret. At worst they open a browser
/// tab, or quit a Helper that nothing is using, which a program on this
/// computer could do anyway by ending its process.
const PUBLIC_PATHS: &[&str] = &["/", "/api/instance", "/api/show", "/api/replace"];

/// The header that carries the token.
const TOKEN_HEADER: &str = "x-token";

/// The only methods any route may answer. State-changing routes are POST; a
/// route that takes other methods still never sees them.
const ALLOWED_METHODS: [Method; 3] = [Method::GET, Method::HEAD, Method::POST];

/// Why a request was refused. The response says which rule it broke and
/// nothing more.
enum Rejection {
    /// Wrong `Host`, foreign `Origin`, or no token.
    Forbidden,
    /// A method outside [`ALLOWED_METHODS`].
    MethodNotAllowed,
    /// A POST whose body is not declared as JSON.
    NotJson,
}

impl IntoResponse for Rejection {
    fn into_response(self) -> Response {
        match self {
            Rejection::Forbidden => StatusCode::FORBIDDEN.into_response(),
            Rejection::MethodNotAllowed => {
                let allow = ALLOWED_METHODS.map(|m| m.to_string()).join(", ");
                (StatusCode::METHOD_NOT_ALLOWED, [(header::ALLOW, allow)]).into_response()
            }
            Rejection::NotJson => StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response(),
        }
    }
}

/// What the rules need to know about the running server.
pub(super) struct Guard {
    /// The UI port actually bound: the only port `Host` may name.
    port: u16,
    /// The secret every request outside [`PUBLIC_PATHS`] must carry.
    token: String,
    /// The Content-Security-Policy for every response.
    csp: HeaderValue,
}

impl Guard {
    /// Rules for a server bound to `port`, whose secret is `token`, and that
    /// serves `page`, whose inline scripts are the only scripts allowed to run.
    pub(super) fn new(port: u16, token: &str, page: &str) -> Self {
        let csp = content_security_policy(page);
        Self {
            port,
            token: token.to_string(),
            csp: HeaderValue::from_str(&csp).expect("the policy is plain ASCII"),
        }
    }

    /// Whether `request` may go on to routing. The rules are checked in this
    /// order, so a request from a foreign `Host` learns nothing else.
    fn admit(&self, request: &Request) -> Result<(), Rejection> {
        let headers = request.headers();
        if !self.host_is_ours(request) || !self.origin_is_absent_or_ours(headers) {
            return Err(Rejection::Forbidden);
        }
        let method = request.method();
        if !ALLOWED_METHODS.contains(method) {
            return Err(Rejection::MethodNotAllowed);
        }
        if method == Method::POST && !is_json(headers) {
            return Err(Rejection::NotJson);
        }
        if !PUBLIC_PATHS.contains(&request.uri().path()) && !self.carries_token(headers) {
            return Err(Rejection::Forbidden);
        }
        Ok(())
    }

    /// Exactly one token header, equal to the token. Compared in constant
    /// time in the token's bytes; a length difference is not a secret.
    fn carries_token(&self, headers: &HeaderMap) -> bool {
        let mut values = headers.get_all(TOKEN_HEADER).iter();
        match (values.next(), values.next()) {
            (Some(given), None) => bool::from(given.as_bytes().ct_eq(self.token.as_bytes())),
            _ => false,
        }
    }

    /// A browser names the page that sent a request in `Origin`. Only our own
    /// page may send requests here; `null` (a sandboxed or opaque origin) is
    /// not ours. No `Origin` at all is a navigation or a non-browser client,
    /// which the token and `Host` rules deal with.
    ///
    /// Checked on every method, not just POST: our own page never needs
    /// another origin, so nothing is lost by the stricter rule.
    fn origin_is_absent_or_ours(&self, headers: &HeaderMap) -> bool {
        if !headers.contains_key(header::ORIGIN) {
            return true;
        }
        single(headers, header::ORIGIN)
            .and_then(|origin| origin.strip_prefix("http://"))
            .is_some_and(|authority| self.is_our_authority(authority))
    }

    /// `Host` must name this server as `127.0.0.1:<port>` or
    /// `localhost:<port>`. Anything else is a name the attacker chose, which is
    /// how DNS rebinding reaches a loopback server. An absolute request target
    /// (`GET http://name:port/…`) names a host too, and is held to the same rule.
    fn host_is_ours(&self, request: &Request) -> bool {
        let header_ok =
            single(request.headers(), header::HOST).is_some_and(|host| self.is_our_authority(host));
        let target_ok = request
            .uri()
            .authority()
            .is_none_or(|authority| self.is_our_authority(authority.as_str()));
        header_ok && target_ok
    }

    fn is_our_authority(&self, authority: &str) -> bool {
        let Some((name, port)) = authority.rsplit_once(':') else {
            return false;
        };
        let name_ok = name == "127.0.0.1" || name.eq_ignore_ascii_case("localhost");
        name_ok && port == self.port.to_string()
    }

    /// Give a response, whatever produced it, the headers every response carries.
    ///
    /// These overwrite anything a handler set, and any `Access-Control-*`
    /// header is removed: this server never grants cross-origin access.
    fn secure(&self, headers: &mut HeaderMap) {
        let cors = headers
            .keys()
            .filter(|name| name.as_str().starts_with("access-control-"))
            .cloned()
            .collect::<Vec<_>>();
        for name in cors {
            headers.remove(name);
        }
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        headers.insert(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        );
        headers.insert(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        );
        headers.insert(header::CONTENT_SECURITY_POLICY, self.csp.clone());
    }
}

/// The policy for every response: the page's own inline scripts (by hash),
/// inline style and the fonts the page carries as data URLs, requests to this
/// server only, nothing else loaded, and no framing. Style is allowed inline
/// rather than by hash so that `style` attributes in the page keep working.
fn content_security_policy(page: &str) -> String {
    let scripts = inline_scripts(page)
        .map(|script| format!("'sha256-{}'", BASE64.encode(&Sha256::digest(script))))
        .collect::<Vec<_>>();
    let scripts = if scripts.is_empty() {
        "'none'".to_string()
    } else {
        scripts.join(" ")
    };
    format!(
        "default-src 'none'; script-src {scripts}; style-src 'unsafe-inline'; \
         font-src data:; connect-src 'self'; base-uri 'none'; form-action 'none'; \
         frame-ancestors 'none'"
    )
}

/// The text of each `<script>…</script>` element in `page`, exactly as a
/// browser hashes it. A `<script>` tag with attributes is not matched, so its
/// script is not allowed to run.
fn inline_scripts(page: &str) -> impl Iterator<Item = &str> {
    const OPEN: &str = "<script>";
    const CLOSE: &str = "</script>";
    let mut rest = page;
    std::iter::from_fn(move || {
        let start = rest.find(OPEN)? + OPEN.len();
        let len = rest[start..].find(CLOSE)?;
        let script = &rest[start..start + len];
        rest = &rest[start + len + CLOSE.len()..];
        Some(script)
    })
}

/// Whether the body is declared as JSON (parameters such as `charset` allowed).
///
/// A page on another site can POST `text/plain`, a form encoding or
/// `multipart/form-data` without asking first. `application/json` makes the
/// browser send a CORS preflight, which this server never grants.
fn is_json(headers: &HeaderMap) -> bool {
    single(headers, header::CONTENT_TYPE)
        .and_then(|value| value.split(';').next())
        .is_some_and(|essence| essence.trim().eq_ignore_ascii_case("application/json"))
}

/// The value of a header that must appear exactly once, as text.
fn single(headers: &HeaderMap, name: header::HeaderName) -> Option<&str> {
    let mut values = headers.get_all(name).iter();
    match (values.next(), values.next()) {
        (Some(value), None) => value.to_str().ok(),
        _ => None,
    }
}

/// Put `router` behind the guard. Serve what this returns, never the router.
pub(super) fn protect(
    router: Router,
    guard: Guard,
) -> impl Service<Request, Response = Response, Error = Infallible, Future: Send> + Clone + Send + 'static
{
    middleware::from_fn_with_state(Arc::new(guard), check).layer(router)
}

async fn check(State(guard): State<Arc<Guard>>, request: Request, next: Next) -> Response {
    let mut response = match guard.admit(&request) {
        Ok(()) => next.run(request).await,
        Err(rejection) => rejection.into_response(),
    };
    guard.secure(response.headers_mut());
    response
}

/// The rules on routes of the kinds later tickets add, which the real server
/// does not have yet. Assertions are on responses only.
#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::routing::{any, get, post};
    use tower::ServiceExt;

    const PORT: u16 = 47700;
    const TOKEN: &str = "0123456789abcdef";

    /// A page route, a POST route and a route that takes any method (and
    /// tries to grant CORS and caching), all behind the guard.
    async fn send(method: &str, path: &str, headers: &[(&str, &str)]) -> Response {
        let router = Router::new()
            .route("/", get(|| async { "page" }))
            .route("/api/action", post(|| async { "done" }))
            .route(
                "/api/anything",
                any(|| async {
                    (
                        [
                            ("access-control-allow-origin", "*"),
                            ("access-control-allow-credentials", "true"),
                            ("cache-control", "public, max-age=3600"),
                        ],
                        "anything",
                    )
                }),
            );
        let app = protect(router, Guard::new(PORT, TOKEN, "<script>go()</script>"));

        let mut request = Request::builder().method(method).uri(path);
        if !headers.iter().any(|(n, _)| n.eq_ignore_ascii_case("host")) {
            request = request.header("host", format!("127.0.0.1:{PORT}"));
        }
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        app.oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    const JSON: (&str, &str) = ("content-type", "application/json");
    const WITH_TOKEN: (&str, &str) = ("x-token", TOKEN);

    #[tokio::test]
    async fn test_post_following_every_rule_reaches_its_route() {
        let own = format!("http://localhost:{PORT}");

        let plain = send("POST", "/api/action", &[WITH_TOKEN, JSON]).await;
        let with_origin = send("POST", "/api/action", &[WITH_TOKEN, JSON, ("origin", &own)]).await;

        assert_eq!(plain.status(), StatusCode::OK);
        assert_eq!(with_origin.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_on_a_post_route_is_refused() {
        let response = send("GET", "/api/action", &[WITH_TOKEN]).await;

        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    }

    #[tokio::test]
    async fn test_post_route_refuses_a_foreign_origin_and_a_body_that_is_not_json() {
        let foreign = send(
            "POST",
            "/api/action",
            &[WITH_TOKEN, JSON, ("origin", "http://rebind.example")],
        )
        .await;
        let form = send(
            "POST",
            "/api/action",
            &[
                WITH_TOKEN,
                ("content-type", "application/x-www-form-urlencoded"),
            ],
        )
        .await;

        assert_eq!(foreign.status(), StatusCode::FORBIDDEN);
        assert_eq!(form.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }

    #[tokio::test]
    async fn test_a_new_route_needs_the_token_without_asking_for_it() {
        let missing = send("POST", "/api/action", &[JSON]).await;
        let wrong = send(
            "POST",
            "/api/action",
            &[JSON, ("x-token", "0123456789abcdeX")],
        )
        .await;
        let twice = send("POST", "/api/action", &[JSON, WITH_TOKEN, WITH_TOKEN]).await;
        let unknown = send("GET", "/api/not-yet", &[]).await;
        let page = send("GET", "/", &[]).await;

        assert_eq!(missing.status(), StatusCode::FORBIDDEN);
        assert_eq!(wrong.status(), StatusCode::FORBIDDEN);
        assert_eq!(twice.status(), StatusCode::FORBIDDEN);
        assert_eq!(unknown.status(), StatusCode::FORBIDDEN);
        assert_eq!(page.status(), StatusCode::OK, "the page needs no token");
    }

    #[tokio::test]
    async fn test_only_get_head_and_post_reach_a_route_that_takes_any_method() {
        for method in ["GET", "HEAD", "POST"] {
            let response = send(method, "/api/anything", &[WITH_TOKEN, JSON]).await;
            assert_eq!(response.status(), StatusCode::OK, "{method}");
        }
        for method in ["OPTIONS", "PUT", "DELETE", "PATCH", "TRACE"] {
            let response = send(method, "/api/anything", &[WITH_TOKEN, JSON]).await;
            assert_eq!(
                response.status(),
                StatusCode::METHOD_NOT_ALLOWED,
                "{method}"
            );
        }
    }

    #[tokio::test]
    async fn test_a_route_cannot_grant_cors_or_caching() {
        let response = send("GET", "/api/anything", &[WITH_TOKEN]).await;

        assert_eq!(response.status(), StatusCode::OK);
        let cors = response
            .headers()
            .keys()
            .filter(|name| name.as_str().starts_with("access-control-"))
            .collect::<Vec<_>>();
        assert!(cors.is_empty(), "CORS headers sent: {cors:?}");
        let caching = response
            .headers()
            .get_all(header::CACHE_CONTROL)
            .iter()
            .collect::<Vec<_>>();
        assert_eq!(caching, ["no-store"]);
    }

    /// Expected hashes come from
    /// `printf '<script text>' | openssl dgst -sha256 -binary | base64`.
    #[test]
    fn test_policy_allows_each_plain_inline_script_by_its_hash() {
        let policy = content_security_policy(
            "<p>x</p><script>alert(1)</script><p>y</p><script>\n  go();\n</script>",
        );

        assert!(
            policy.contains(
                "script-src 'sha256-bhHHL3z2vDgxUt0W3dWQOrprscmda2Y5pLsLg4GF+pI=' \
                 'sha256-bMJjpehY8EHYTEPyRlK8345NPB2b9eh4N0kqsRPiDq4=';"
            ),
            "{policy}"
        );
    }

    #[test]
    fn test_policy_allows_no_script_that_is_not_a_plain_inline_one() {
        let policy = content_security_policy(
            r#"<script src="app.js"></script><script type="module">go()</script>"#,
        );

        assert!(policy.contains("script-src 'none';"), "{policy}");
    }
}
