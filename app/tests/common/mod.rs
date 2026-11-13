//! Helpers for the integration tests. Tests that touch Redis expect it at
//! REDIS_URL, by default redis://127.0.0.1:6379 (see the README).

// Each test binary uses a different subset of these helpers.
#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Request, StatusCode},
};
use http_body_util::BodyExt;
use miniarcade::{AppState, metrics::Metrics, store::Store};
use serde_json::{Value, json};
use tower::ServiceExt;
use tracing_subscriber::fmt::MakeWriter;

pub fn redis_url() -> String {
    std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_owned())
}

/// Nothing listens on port 1, so every Redis call fails straight away.
pub const UNREACHABLE_REDIS: &str = "redis://127.0.0.1:1";

pub fn state() -> AppState {
    state_with(&redis_url())
}

pub fn state_with(redis_url: &str) -> AppState {
    AppState::new(
        Store::new(redis_url).expect("valid Redis URL"),
        Metrics::new(),
    )
}

/// A direct Redis connection, for looking behind the API.
pub async fn redis() -> redis::aio::MultiplexedConnection {
    redis::Client::open(redis_url())
        .expect("valid Redis URL")
        .get_multiplexed_async_connection()
        .await
        .expect("Redis is running")
}

pub struct Reply {
    pub status: StatusCode,
    pub headers: HeaderMap,
    /// The body as JSON, or as a JSON string when it isn't JSON.
    pub body: Value,
}

pub async fn send(
    router: &Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<String>,
) -> Reply {
    let mut request = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        request = request.header("x-player-token", token);
    }
    let request = match body {
        Some(body) => request
            .header("content-type", "application/json")
            .body(Body::from(body)),
        None => request.body(Body::empty()),
    }
    .expect("valid request");
    let response = router.clone().oneshot(request).await.expect("infallible");
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let body = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    Reply {
        status,
        headers,
        body,
    }
}

pub async fn call(
    router: &Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> Reply {
    send(
        router,
        method,
        uri,
        token,
        body.map(|body| body.to_string()),
    )
    .await
}

/// Creates a lobby and returns its code and seat 1's token.
pub async fn create(app: &Router) -> (String, String) {
    let reply = call(app, "POST", "/api/lobbies", None, None).await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
    (text(&reply.body["code"]), text(&reply.body["token"]))
}

/// Takes seat 2 and returns its token.
pub async fn join(app: &Router, code: &str) -> String {
    let reply = call(
        app,
        "POST",
        &format!("/api/lobbies/{code}/join"),
        None,
        None,
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    text(&reply.body["token"])
}

/// The lobby as anyone with the link sees it.
pub async fn lobby(app: &Router, code: &str) -> Value {
    let reply = call(app, "GET", &format!("/api/lobbies/{code}"), None, None).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    reply.body
}

/// A lobby with both seats taken: its code, then the tokens of seat 1 (X)
/// and seat 2 (O). It is at version 2 with X to move.
pub async fn playing_lobby(app: &Router) -> (String, String, String) {
    let (code, x) = create(app).await;
    let o = join(app, &code).await;
    (code, x, o)
}

pub async fn play(app: &Router, code: &str, token: &str, cell: usize, version: u64) -> Reply {
    let body = json!({ "cell": cell, "version": version });
    call(
        app,
        "POST",
        &format!("/api/lobbies/{code}/move"),
        Some(token),
        Some(body),
    )
    .await
}

pub async fn rematch(app: &Router, code: &str, token: &str, version: u64) -> Reply {
    let body = json!({ "version": version });
    call(
        app,
        "POST",
        &format!("/api/lobbies/{code}/rematch"),
        Some(token),
        Some(body),
    )
    .await
}

/// From a fresh game at `version`, X takes the top row. Returns the version
/// afterwards.
pub async fn x_wins(app: &Router, code: &str, x: &str, o: &str, version: u64) -> u64 {
    let moves = [(x, 0), (o, 3), (x, 1), (o, 4), (x, 2)];
    for (offset, (token, cell)) in (0..).zip(moves) {
        let reply = play(app, code, token, cell, version + offset).await;
        assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    }
    version + 5
}

pub fn assert_security_headers(headers: &HeaderMap) {
    assert_eq!(
        headers["content-security-policy"],
        "default-src 'self'; frame-ancestors 'none'"
    );
    assert_eq!(headers["x-content-type-options"], "nosniff");
    assert_eq!(headers["referrer-policy"], "no-referrer");
}

/// Collects what `tracing` writes, one log line per entry.
#[derive(Clone, Default)]
pub struct CapturedLogs(Arc<Mutex<Vec<u8>>>);

impl CapturedLogs {
    pub fn lines(&self) -> Vec<String> {
        let bytes = self.0.lock().unwrap().clone();
        String::from_utf8(bytes)
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect()
    }
}

impl std::io::Write for CapturedLogs {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for CapturedLogs {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

fn text(value: &Value) -> String {
    value
        .as_str()
        .unwrap_or_else(|| panic!("expected a string, got {value}"))
        .to_owned()
}
