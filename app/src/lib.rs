//! MiniArcade: two-player games in the browser.

pub mod api;
pub mod config;
pub mod games;
pub mod lobby;
pub mod logging;
pub mod metrics;
pub mod ops;
pub mod store;

use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::{HeaderValue, header},
    middleware,
    response::Response,
};

use crate::{metrics::Metrics, store::Store};

/// Request bodies above this are refused.
pub const BODY_LIMIT_BYTES: usize = 1024;

/// What every request handler shares.
#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub metrics: Metrics,
}

impl AppState {
    pub fn new(store: Store, metrics: Metrics) -> Self {
        Self { store, metrics }
    }
}

/// Port 8080: the API, and from phase 3 the page.
pub fn app(state: AppState) -> Router {
    api::router()
        .layer(DefaultBodyLimit::max(BODY_LIMIT_BYTES))
        .layer(middleware::from_fn_with_state(
            state.metrics.clone(),
            metrics::track,
        ))
        .layer(middleware::map_response(security_headers))
        .with_state(state)
}

/// Port 9090: probes and metrics.
pub fn ops(state: AppState) -> Router {
    ops::router()
        .layer(middleware::map_response(security_headers))
        .with_state(state)
}

/// Added to every response on both ports, errors included.
async fn security_headers(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'self'; frame-ancestors 'none'"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response
}
