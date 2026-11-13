//! The per-request log line. This test has a test binary to itself on
//! purpose: tracing caches per call site whether any subscriber listens, and
//! a test on another thread reaching the same call site with no subscriber
//! can race that cache and hide these events.

mod common;

use common::*;
use serde_json::Value;

#[tokio::test]
async fn each_request_logs_one_json_line_without_tokens() {
    let logs = CapturedLogs::default();
    let _guard =
        tracing::subscriber::set_default(miniarcade::logging::json_subscriber(logs.clone()));
    let app = miniarcade::app(state());
    let (code, token) = create(&app).await;
    play(&app, &code, &token, 4, 1).await;

    let lines = logs.lines();
    let requests: Vec<Value> = lines
        .iter()
        .map(|line| serde_json::from_str::<Value>(line).expect("every line is JSON"))
        .filter(|entry| entry["message"] == "request")
        .collect();
    assert_eq!(requests.len(), 2, "{lines:#?}");
    let fields = |entry: &Value| {
        (
            entry["method"].clone(),
            entry["route"].clone(),
            entry["status"].clone(),
        )
    };
    assert_eq!(
        fields(&requests[0]),
        ("POST".into(), "/api/lobbies".into(), 201.into())
    );
    assert_eq!(
        fields(&requests[1]),
        ("POST".into(), "/api/lobbies/{code}/move".into(), 400.into())
    );
    assert!(requests.iter().all(|entry| entry["latency_ms"].is_f64()));
    assert!(
        lines.iter().all(|line| !line.contains(&token)),
        "{lines:#?}"
    );
}
