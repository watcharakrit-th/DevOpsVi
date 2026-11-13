//! Port 9090: probes and metrics.

mod common;

use axum::http::StatusCode;
use common::*;

#[tokio::test]
async fn livez_is_200_even_without_redis() {
    let ops = miniarcade::ops(state_with(UNREACHABLE_REDIS));
    assert_eq!(
        call(&ops, "GET", "/livez", None, None).await.status,
        StatusCode::OK
    );
}

#[tokio::test]
async fn readyz_follows_redis() {
    let ops = miniarcade::ops(state());
    assert_eq!(
        call(&ops, "GET", "/readyz", None, None).await.status,
        StatusCode::OK
    );
    let ops = miniarcade::ops(state_with(UNREACHABLE_REDIS));
    assert_eq!(
        call(&ops, "GET", "/readyz", None, None).await.status,
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[tokio::test]
async fn ops_responses_carry_the_security_headers() {
    let ops = miniarcade::ops(state());
    for path in ["/livez", "/readyz", "/metrics"] {
        assert_security_headers(&call(&ops, "GET", path, None, None).await.headers);
    }
}

#[tokio::test]
async fn requests_are_counted_by_route_template() {
    let state = state();
    let (app, ops) = (miniarcade::app(state.clone()), miniarcade::ops(state));
    let (code, _) = create(&app).await;
    lobby(&app, &code).await;
    call(&app, "GET", "/no/such/page", None, None).await;
    call(&app, "GET", "/also/missing", None, None).await;
    call(&app, "BREW", "/api/lobbies", None, None).await;
    call(&ops, "GET", "/livez", None, None).await;

    let text = scrape(&ops).await;
    let expected = [
        r#"http_requests_total{method="POST",route="/api/lobbies",status="201"} 1"#,
        r#"http_requests_total{method="GET",route="/api/lobbies/{code}",status="200"} 1"#,
        r#"http_requests_total{method="GET",route="unmatched",status="404"} 2"#,
        r#"http_requests_total{method="OTHER",route="/api/lobbies",status="405"} 1"#,
        r#"http_request_duration_seconds_count{route="/api/lobbies/{code}"} 1"#,
    ];
    for line in expected {
        assert!(text.contains(line), "missing {line}\n{text}");
    }
    assert!(!text.contains("livez"), "probes aren't counted\n{text}");
    assert!(!text.contains(&code), "codes stay out of labels\n{text}");
}

#[tokio::test]
async fn finished_games_are_counted_by_result() {
    let state = state();
    let (app, ops) = (miniarcade::app(state.clone()), miniarcade::ops(state));
    let text = scrape(&ops).await;
    for result in ["x_won", "o_won", "draw"] {
        let line = format!(r#"miniarcade_games_finished_total{{result="{result}"}} 0"#);
        assert!(text.contains(&line), "missing {line}\n{text}");
    }

    let (code, x, o) = playing_lobby(&app).await;
    x_wins(&app, &code, &x, &o, 2).await;
    let text = scrape(&ops).await;
    assert!(
        text.contains(r#"miniarcade_games_finished_total{result="x_won"} 1"#),
        "{text}"
    );
}
