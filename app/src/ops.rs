//! Port 9090: probes for Kubernetes and metrics for Prometheus. Never
//! exposed outside the cluster.

use std::time::Duration;

use axum::{
    Router,
    extract::State,
    http::{StatusCode, header},
    response::IntoResponse,
    routing::get,
};

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/livez", get(livez))
        .route("/readyz", get(readyz))
        .route("/metrics", get(metrics))
}

/// The process is up. Ignores Redis on purpose: restarting the app wouldn't
/// bring Redis back.
async fn livez() -> &'static str {
    "ok"
}

/// Ready only when Redis answers PING, so the Service sends players only to
/// pods that can serve them.
async fn readyz(State(state): State<AppState>) -> (StatusCode, &'static str) {
    match tokio::time::timeout(Duration::from_secs(1), state.store.ping()).await {
        Ok(Ok(())) => (StatusCode::OK, "ok"),
        _ => (StatusCode::SERVICE_UNAVAILABLE, "redis unavailable"),
    }
}

async fn metrics(State(state): State<AppState>) -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, prometheus::TEXT_FORMAT)],
        state.metrics.render(),
    )
}
