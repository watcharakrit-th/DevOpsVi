//! MiniArcade: two-player games in the browser.

pub mod config;
pub mod games;
pub mod lobby;
pub mod logging;
pub mod metrics;
pub mod store;

use axum::Router;

use crate::{metrics::Metrics, store::Store};

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
pub fn app(_state: AppState) -> Router {
    Router::new()
}

/// Port 9090: probes and metrics.
pub fn ops(_state: AppState) -> Router {
    Router::new()
}
