//! Prometheus metrics, and the middleware that counts, times and logs each
//! request.

use std::time::{Duration, Instant};

use axum::{
    extract::{MatchedPath, Request, State},
    http::Method,
    middleware::Next,
    response::Response,
};
use prometheus::{
    HistogramOpts, HistogramVec, IntCounterVec, Opts, Registry, TextEncoder, core::Collector,
};

use crate::lobby::LobbyStatus;

/// The `route` label for requests no route matched, so random paths can't
/// create new label values.
pub const UNMATCHED_ROUTE: &str = "unmatched";

/// Any other method (HTTP allows made-up ones) is labelled OTHER, for the
/// same reason.
const METHODS: [&str; 9] = [
    "GET", "HEAD", "POST", "PUT", "DELETE", "CONNECT", "OPTIONS", "TRACE", "PATCH",
];

/// Each instance has its own registry, so tests don't share counters.
#[derive(Clone)]
pub struct Metrics {
    registry: Registry,
    requests: IntCounterVec,
    duration: HistogramVec,
    games_finished: IntCounterVec,
}

impl Metrics {
    pub fn new() -> Self {
        let requests = IntCounterVec::new(
            Opts::new(
                "http_requests_total",
                "HTTP requests by method, route and status.",
            ),
            &["method", "route", "status"],
        )
        .expect("valid metric");
        let duration = HistogramVec::new(
            HistogramOpts::new(
                "http_request_duration_seconds",
                "HTTP request latency by route.",
            ),
            &["route"],
        )
        .expect("valid metric");
        let games_finished = IntCounterVec::new(
            Opts::new(
                "miniarcade_games_finished_total",
                "Finished games by result.",
            ),
            &["result"],
        )
        .expect("valid metric");

        let registry = Registry::new();
        let collectors: [Box<dyn Collector>; 3] = [
            Box::new(requests.clone()),
            Box::new(duration.clone()),
            Box::new(games_finished.clone()),
        ];
        for collector in collectors {
            registry
                .register(collector)
                .expect("metric names are unique");
        }
        // Export every result from the start, at 0, so dashboards and rate()
        // see the series before the first game ends.
        for result in [LobbyStatus::XWon, LobbyStatus::OWon, LobbyStatus::Draw] {
            games_finished.with_label_values(&[result.as_str()]);
        }

        Self {
            registry,
            requests,
            duration,
            games_finished,
        }
    }

    pub fn game_finished(&self, result: LobbyStatus) {
        self.games_finished
            .with_label_values(&[result.as_str()])
            .inc();
    }

    /// Everything, in the Prometheus text format.
    pub fn render(&self) -> String {
        TextEncoder::new()
            .encode_to_string(&self.registry.gather())
            .expect("metrics encode as text")
    }

    fn observe(&self, method: &str, route: &str, status: u16, elapsed: Duration) {
        self.requests
            .with_label_values(&[method, route, &status.to_string()])
            .inc();
        self.duration
            .with_label_values(&[route])
            .observe(elapsed.as_secs_f64());
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Counts, times and logs one request. It runs on the app port only: probes
/// and scrapes on the ops port would drown out player traffic, and probe
/// 503s while Redis is down would count towards the 5xx alert.
pub async fn track(State(metrics): State<Metrics>, request: Request, next: Next) -> Response {
    let started = Instant::now();
    let method = method_label(request.method());
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map_or(UNMATCHED_ROUTE, MatchedPath::as_str)
        .to_owned();
    let response = next.run(request).await;
    let elapsed = started.elapsed();
    let status = response.status().as_u16();
    metrics.observe(method, &route, status, elapsed);
    // Only the route template: the real path holds the lobby code, and
    // headers and bodies hold tokens.
    tracing::info!(
        method,
        route = route.as_str(),
        status,
        latency_ms = elapsed.as_secs_f64() * 1000.0,
        "request"
    );
    response
}

fn method_label(method: &Method) -> &'static str {
    METHODS
        .into_iter()
        .find(|known| *known == method.as_str())
        .unwrap_or("OTHER")
}
