//! The browser page: the three files in app/static/, compiled into the
//! binary so the image needs no files beside it.

use axum::{Router, http::header, response::IntoResponse, routing::get};

const INDEX_HTML: &str = include_str!("../static/index.html");
const APP_JS: &str = include_str!("../static/app.js");
const STYLE_CSS: &str = include_str!("../static/style.css");

/// Explicit routes rather than a static-file directory: each file gets its
/// own route label in the metrics, and there's no filesystem path to abuse.
pub fn router<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route(
            "/",
            get(|| async { file("text/html; charset=utf-8", INDEX_HTML) }),
        )
        .route(
            "/app.js",
            get(|| async { file("text/javascript; charset=utf-8", APP_JS) }),
        )
        .route(
            "/style.css",
            get(|| async { file("text/css; charset=utf-8", STYLE_CSS) }),
        )
}

/// `no-cache` makes the browser check back on every load, so a release
/// reaches an open page at its next reload.
fn file(content_type: &'static str, body: &'static str) -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        body,
    )
}
