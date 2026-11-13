//! The JSON API on port 8080, against a real Redis.

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};

fn empty_board() -> Value {
    Value::Array(vec![Value::Null; 9])
}

#[tokio::test]
async fn creating_a_lobby_returns_a_code_and_a_token() {
    let app = miniarcade::app(state());
    let reply = call(&app, "POST", "/api/lobbies", None, None).await;
    assert_eq!(reply.status, StatusCode::CREATED);
    let code = reply.body["code"].as_str().unwrap();
    assert_eq!(code.len(), 6);
    assert!(
        code.chars()
            .all(|c| "ABCDEFGHJKMNPQRSTUVWXYZ23456789".contains(c)),
        "{code}"
    );
    let token = reply.body["token"].as_str().unwrap();
    assert_eq!(token.len(), 32, "128 bits as hex");
    assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
}

#[tokio::test]
async fn a_new_lobby_waits_for_seat_two_and_never_shows_tokens() {
    let app = miniarcade::app(state());
    let (code, token) = create(&app).await;
    let view = lobby(&app, &code).await;
    assert_eq!(
        view,
        json!({ "board": empty_board(), "status": "waiting", "turn": null, "x_seat": 1, "version": 1 })
    );
    assert!(!view.to_string().contains(&token));
}

#[tokio::test]
async fn joining_takes_seat_two_and_starts_the_game() {
    let app = miniarcade::app(state());
    let (code, one) = create(&app).await;
    let two = join(&app, &code).await;
    assert_eq!(two.len(), 32);
    assert_ne!(one, two);
    let view = lobby(&app, &code).await;
    assert_eq!(
        (&view["status"], &view["turn"], &view["version"]),
        (&json!("playing"), &json!("X"), &json!(2))
    );
    assert!(!view.to_string().contains(&two));
}

#[tokio::test]
async fn a_third_player_finds_the_lobby_full() {
    let app = miniarcade::app(state());
    let (code, _, _) = playing_lobby(&app).await;
    let reply = call(
        &app,
        "POST",
        &format!("/api/lobbies/{code}/join"),
        None,
        None,
    )
    .await;
    assert_eq!(
        (reply.status, reply.body),
        (StatusCode::CONFLICT, json!({ "error": "lobby_full" }))
    );
}

#[tokio::test]
async fn players_alternate_and_every_move_bumps_the_version() {
    let app = miniarcade::app(state());
    let (code, x, o) = playing_lobby(&app).await;
    let reply = play(&app, &code, &x, 4, 2).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(
        (
            &reply.body["board"][4],
            &reply.body["turn"],
            &reply.body["version"]
        ),
        (&json!("X"), &json!("O"), &json!(3))
    );
    let reply = play(&app, &code, &o, 0, 3).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(
        (&reply.body["board"][0], &reply.body["version"]),
        (&json!("O"), &json!(4))
    );
    assert_eq!(
        lobby(&app, &code).await,
        reply.body,
        "a move returns the new state"
    );
}

#[tokio::test]
async fn illegal_moves_are_400s() {
    let app = miniarcade::app(state());
    let (code, x, o) = playing_lobby(&app).await;
    let cases = [
        (&o, json!({ "cell": 0, "version": 2 }), "not_your_turn"),
        (&x, json!({ "cell": 9, "version": 2 }), "invalid_cell"),
        (&x, json!({ "cell": -1, "version": 2 }), "invalid_body"),
        (&x, json!({ "cell": "4", "version": 2 }), "invalid_body"),
        (&x, json!({ "version": 2 }), "invalid_body"),
    ];
    let uri = format!("/api/lobbies/{code}/move");
    for (token, body, error) in cases {
        let reply = call(&app, "POST", &uri, Some(token), Some(body.clone())).await;
        assert_eq!(
            (reply.status, reply.body),
            (StatusCode::BAD_REQUEST, json!({ "error": error })),
            "{body}"
        );
    }
    let reply = send(&app, "POST", &uri, Some(&x), Some("{not json".to_owned())).await;
    assert_eq!(reply.body, json!({ "error": "invalid_body" }));

    play(&app, &code, &x, 4, 2).await;
    let reply = play(&app, &code, &o, 4, 3).await;
    assert_eq!(
        (reply.status, reply.body),
        (StatusCode::BAD_REQUEST, json!({ "error": "cell_taken" }))
    );
}

#[tokio::test]
async fn moves_need_a_seat_token() {
    let app = miniarcade::app(state());
    let (code, _, _) = playing_lobby(&app).await;
    let (_, other_lobbys_token) = create(&app).await;
    let uri = format!("/api/lobbies/{code}/move");
    for token in [None, Some("nope"), Some(other_lobbys_token.as_str())] {
        let body = json!({ "cell": 4, "version": 2 });
        let reply = call(&app, "POST", &uri, token, Some(body)).await;
        assert_eq!(
            (reply.status, reply.body),
            (StatusCode::FORBIDDEN, json!({ "error": "bad_token" })),
            "{token:?}"
        );
    }
}

#[tokio::test]
async fn a_stale_version_is_a_conflict() {
    let app = miniarcade::app(state());
    let (code, x, o) = playing_lobby(&app).await;
    play(&app, &code, &x, 4, 2).await;
    let reply = play(&app, &code, &o, 0, 2).await;
    assert_eq!(
        (reply.status, reply.body),
        (StatusCode::CONFLICT, json!({ "error": "stale_version" }))
    );
}

#[tokio::test]
async fn nobody_moves_before_seat_two_is_taken() {
    let app = miniarcade::app(state());
    let (code, x) = create(&app).await;
    let reply = play(&app, &code, &x, 4, 1).await;
    assert_eq!(
        (reply.status, reply.body),
        (
            StatusCode::BAD_REQUEST,
            json!({ "error": "waiting_for_opponent" })
        )
    );
}

#[tokio::test]
async fn a_finished_game_takes_no_moves_but_either_seat_can_start_a_rematch() {
    let app = miniarcade::app(state());
    let (code, x, o) = playing_lobby(&app).await;
    let version = x_wins(&app, &code, &x, &o, 2).await;
    let view = lobby(&app, &code).await;
    assert_eq!(
        (&view["status"], &view["turn"]),
        (&json!("x_won"), &Value::Null)
    );
    let reply = play(&app, &code, &o, 8, version).await;
    assert_eq!(
        (reply.status, reply.body),
        (StatusCode::BAD_REQUEST, json!({ "error": "game_over" }))
    );

    // The loser asks for the rematch. X and O swap: seat 2 now plays X and starts.
    let reply = rematch(&app, &code, &o, version).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(
        reply.body,
        json!({ "board": empty_board(), "status": "playing", "turn": "X", "x_seat": 2, "version": version + 1 })
    );
    let reply = play(&app, &code, &x, 4, version + 1).await;
    assert_eq!(reply.body, json!({ "error": "not_your_turn" }));
    assert_eq!(
        play(&app, &code, &o, 4, version + 1).await.status,
        StatusCode::OK
    );
}

#[tokio::test]
async fn a_rematch_needs_a_finished_game_the_current_version_and_a_token() {
    let app = miniarcade::app(state());
    let (code, x, o) = playing_lobby(&app).await;
    let reply = rematch(&app, &code, &x, 2).await;
    assert_eq!(
        (reply.status, reply.body),
        (StatusCode::BAD_REQUEST, json!({ "error": "game_not_over" }))
    );
    let version = x_wins(&app, &code, &x, &o, 2).await;
    let reply = rematch(&app, &code, &x, version - 1).await;
    assert_eq!(
        (reply.status, reply.body),
        (StatusCode::CONFLICT, json!({ "error": "stale_version" }))
    );
    let reply = rematch(&app, &code, "nope", version).await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);
    assert_eq!(
        rematch(&app, &code, &x, version).await.status,
        StatusCode::OK
    );
}

#[tokio::test]
async fn unknown_and_malformed_codes_are_404s() {
    let app = miniarcade::app(state());
    let uris = [
        "/api/lobbies/ZZZZZZ",
        "/api/lobbies/abc",
        "/api/lobbies/ABC23O",
        "/api/lobbies/ABCDEFGH",
        "/api/lobbies/%FF%FE",
    ];
    for uri in uris {
        let reply = call(&app, "GET", uri, None, None).await;
        assert_eq!(
            (reply.status, reply.body),
            (StatusCode::NOT_FOUND, json!({ "error": "not_found" })),
            "{uri}"
        );
    }
}

#[tokio::test]
async fn codes_are_checked_before_redis_is_touched() {
    // With Redis unreachable, a well-formed code fails with 503 while a
    // malformed one is still a 404: it never got as far as Redis.
    let app = miniarcade::app(state_with(UNREACHABLE_REDIS));
    let reply = call(&app, "GET", "/api/lobbies/ABC23O", None, None).await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    let reply = call(&app, "GET", "/api/lobbies/ABC234", None, None).await;
    assert_eq!(
        (reply.status, reply.body),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            json!({ "error": "unavailable" })
        )
    );
}

#[tokio::test]
async fn an_expired_lobby_is_a_404() {
    let app = miniarcade::app(state());
    let (code, x, _) = playing_lobby(&app).await;
    let mut redis = redis().await;
    let _: () = redis::cmd("DEL")
        .arg(format!("lobby:{code}"))
        .query_async(&mut redis)
        .await
        .unwrap();
    let reply = call(&app, "GET", &format!("/api/lobbies/{code}"), None, None).await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    assert_eq!(
        play(&app, &code, &x, 4, 2).await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn every_write_sets_a_one_hour_ttl() {
    let app = miniarcade::app(state());
    let (code, x, _) = playing_lobby(&app).await;
    let mut redis = redis().await;
    let key = format!("lobby:{code}");
    let ttl: i64 = redis::cmd("TTL")
        .arg(&key)
        .query_async(&mut redis)
        .await
        .unwrap();
    assert!((3590..=3600).contains(&ttl), "after the join: {ttl}");

    let _: () = redis::cmd("EXPIRE")
        .arg(&key)
        .arg(60)
        .query_async(&mut redis)
        .await
        .unwrap();
    assert_eq!(play(&app, &code, &x, 4, 2).await.status, StatusCode::OK);
    let ttl: i64 = redis::cmd("TTL")
        .arg(&key)
        .query_async(&mut redis)
        .await
        .unwrap();
    assert!((3590..=3600).contains(&ttl), "after a move: {ttl}");
}

#[tokio::test]
async fn bodies_over_1_kib_are_refused() {
    let app = miniarcade::app(state());
    let (code, x, _) = playing_lobby(&app).await;
    let padded = json!({ "cell": 4, "version": 2, "pad": "x".repeat(1024) });
    let uri = format!("/api/lobbies/{code}/move");
    let reply = call(&app, "POST", &uri, Some(&x), Some(padded)).await;
    assert_eq!(
        (reply.status, reply.body),
        (
            StatusCode::BAD_REQUEST,
            json!({ "error": "body_too_large" })
        )
    );
    assert_eq!(play(&app, &code, &x, 4, 2).await.status, StatusCode::OK);
}

#[tokio::test]
async fn unknown_paths_and_methods_get_json_errors() {
    let app = miniarcade::app(state());
    let reply = call(&app, "GET", "/nope", None, None).await;
    assert_eq!(
        (reply.status, reply.body),
        (StatusCode::NOT_FOUND, json!({ "error": "not_found" }))
    );
    let reply = call(&app, "DELETE", "/api/lobbies", None, None).await;
    assert_eq!(
        (reply.status, reply.body),
        (
            StatusCode::METHOD_NOT_ALLOWED,
            json!({ "error": "method_not_allowed" })
        )
    );
}

#[tokio::test]
async fn every_response_carries_the_security_headers() {
    let app = miniarcade::app(state());
    let (code, _) = create(&app).await;
    let replies = [
        call(&app, "POST", "/api/lobbies", None, None).await,
        call(&app, "GET", &format!("/api/lobbies/{code}"), None, None).await,
        call(&app, "GET", "/api/lobbies/ZZZZZZ", None, None).await,
        call(&app, "GET", "/nope", None, None).await,
        call(&app, "DELETE", "/api/lobbies", None, None).await,
        call(
            &app,
            "POST",
            &format!("/api/lobbies/{code}/move"),
            None,
            Some(json!({})),
        )
        .await,
    ];
    for reply in replies {
        assert_security_headers(&reply.headers);
    }
}

/// Two app instances, each with its own Redis connection, stand in for the
/// two replicas.
fn two_replicas() -> [axum::Router; 2] {
    [miniarcade::app(state()), miniarcade::app(state())]
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn racing_moves_on_two_replicas_let_exactly_one_through() {
    let replicas = two_replicas();
    let (code, x, _) = playing_lobby(&replicas[0]).await;
    let racers: Vec<_> = (0..8)
        .map(|cell| {
            let (app, code, x) = (replicas[cell % 2].clone(), code.clone(), x.clone());
            tokio::spawn(async move { play(&app, &code, &x, cell, 2).await.status })
        })
        .collect();
    let mut statuses = Vec::new();
    for racer in racers {
        statuses.push(racer.await.unwrap());
    }
    let wins = statuses.iter().filter(|s| **s == StatusCode::OK).count();
    let conflicts = statuses
        .iter()
        .filter(|s| **s == StatusCode::CONFLICT)
        .count();
    assert_eq!((wins, conflicts), (1, 7), "{statuses:?}");

    let view = lobby(&replicas[1], &code).await;
    let xs = view["board"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|cell| **cell == "X")
        .count();
    assert_eq!((xs, &view["version"]), (1, &json!(3)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn racing_joins_on_two_replicas_fill_seat_two_once() {
    let replicas = two_replicas();
    let (code, _) = create(&replicas[0]).await;
    let racers: Vec<_> = (0..8)
        .map(|i| {
            let (app, code) = (replicas[i % 2].clone(), code.clone());
            let uri = format!("/api/lobbies/{code}/join");
            tokio::spawn(async move { call(&app, "POST", &uri, None, None).await })
        })
        .collect();
    let mut replies = Vec::new();
    for racer in racers {
        replies.push(racer.await.unwrap());
    }
    let joined = replies
        .iter()
        .filter(|r| r.status == StatusCode::OK)
        .count();
    let full = replies
        .iter()
        .filter(|r| r.status == StatusCode::CONFLICT && r.body == json!({ "error": "lobby_full" }))
        .count();
    assert_eq!((joined, full), (1, 7));
}
