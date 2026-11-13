//! The JSON API on port 8080.

use axum::{
    Json, Router,
    extract::{FromRequest, FromRequestParts, Path, Request, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode, request::Parts},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    AppState,
    games::xo::MoveError,
    lobby::{Lobby, LobbyCode, LobbyError, LobbyView, Seat, Token},
    store::{Save, StoreError},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/lobbies", post(create))
        .route("/api/lobbies/{code}", get(show))
        .route("/api/lobbies/{code}/join", post(join))
        .route("/api/lobbies/{code}/move", post(play))
        .route("/api/lobbies/{code}/rematch", post(rematch))
        .fallback(|| async { ApiError::NotFound })
        .method_not_allowed_fallback(|| async { ApiError::MethodNotAllowed })
}

#[derive(Serialize)]
struct Created {
    code: String,
    token: String,
}

#[derive(Serialize)]
struct Joined {
    token: String,
}

#[derive(Deserialize)]
struct MoveRequest {
    cell: usize,
    version: u64,
}

#[derive(Deserialize)]
struct RematchRequest {
    version: u64,
}

async fn create(State(state): State<AppState>) -> Result<(StatusCode, Json<Created>), ApiError> {
    let token = Token::random();
    let code = state.store.create(&Lobby::new(token.clone())).await?;
    let created = Created {
        code: code.as_str().to_owned(),
        token: token.as_str().to_owned(),
    };
    Ok((StatusCode::CREATED, Json(created)))
}

async fn show(
    State(state): State<AppState>,
    Code(code): Code,
) -> Result<Json<LobbyView>, ApiError> {
    Ok(Json(load(&state, &code).await?.view()))
}

async fn join(State(state): State<AppState>, Code(code): Code) -> Result<Json<Joined>, ApiError> {
    // Losing a race means someone else wrote first. Reading again shows what
    // changed, which before seat 2 is taken can only be another join.
    for _ in 0..3 {
        let mut lobby = load(&state, &code).await?;
        let version = lobby.version();
        let token = Token::random();
        lobby.join(token.clone())?;
        match state.store.save(&code, &lobby, version).await? {
            Save::Written => {
                return Ok(Json(Joined {
                    token: token.as_str().to_owned(),
                }));
            }
            Save::Conflict => continue,
            Save::Expired => return Err(ApiError::NotFound),
        }
    }
    Err(ApiError::Conflict("stale_version"))
}

async fn play(
    State(state): State<AppState>,
    Code(code): Code,
    headers: HeaderMap,
    JsonBody(request): JsonBody<MoveRequest>,
) -> Result<Json<LobbyView>, ApiError> {
    let mut lobby = load(&state, &code).await?;
    let seat = seat(&lobby, &headers)?;
    check_version(&lobby, request.version)?;
    let status = lobby.play(seat, request.cell)?;
    save(&state, &code, &lobby, request.version).await?;
    if status.is_finished() {
        state.metrics.game_finished(status);
    }
    Ok(Json(lobby.view()))
}

async fn rematch(
    State(state): State<AppState>,
    Code(code): Code,
    headers: HeaderMap,
    JsonBody(request): JsonBody<RematchRequest>,
) -> Result<Json<LobbyView>, ApiError> {
    let mut lobby = load(&state, &code).await?;
    seat(&lobby, &headers)?;
    check_version(&lobby, request.version)?;
    lobby.rematch()?;
    save(&state, &code, &lobby, request.version).await?;
    Ok(Json(lobby.view()))
}

async fn load(state: &AppState, code: &LobbyCode) -> Result<Lobby, ApiError> {
    state.store.load(code).await?.ok_or(ApiError::NotFound)
}

async fn save(
    state: &AppState,
    code: &LobbyCode,
    lobby: &Lobby,
    expected_version: u64,
) -> Result<(), ApiError> {
    match state.store.save(code, lobby, expected_version).await? {
        Save::Written => Ok(()),
        Save::Conflict => Err(ApiError::Conflict("stale_version")),
        Save::Expired => Err(ApiError::NotFound),
    }
}

/// The seat whose token is in the X-Player-Token header.
fn seat(lobby: &Lobby, headers: &HeaderMap) -> Result<Seat, ApiError> {
    headers
        .get("x-player-token")
        .and_then(|token| token.to_str().ok())
        .and_then(|token| lobby.seat_of(token))
        .ok_or(ApiError::BadToken)
}

fn check_version(lobby: &Lobby, version: u64) -> Result<(), ApiError> {
    if lobby.version() == version {
        Ok(())
    } else {
        Err(ApiError::Conflict("stale_version"))
    }
}

/// The `{code}` path segment, format-checked before the handler (and so any
/// Redis call) runs. A malformed code is a 404: no lobby can have it.
struct Code(LobbyCode);

impl<S: Send + Sync> FromRequestParts<S> for Code {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path(code) = Path::<String>::from_request_parts(parts, state)
            .await
            .map_err(|_| ApiError::NotFound)?;
        LobbyCode::parse(&code).map(Self).ok_or(ApiError::NotFound)
    }
}

/// Like `Json`, but a bad body becomes our JSON 400 rather than axum's plain
/// text, including bodies over the 1 KiB limit.
struct JsonBody<T>(T);

impl<S, T> FromRequest<S> for JsonBody<T>
where
    Json<T>: FromRequest<S, Rejection = JsonRejection>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(request, state).await {
            Ok(Json(value)) => Ok(Self(value)),
            Err(rejection) if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE => {
                Err(ApiError::BadRequest("body_too_large"))
            }
            Err(_) => Err(ApiError::BadRequest("invalid_body")),
        }
    }
}

/// Every error is `{"error": "<code>"}`.
#[derive(Debug)]
pub enum ApiError {
    /// 400: bad input or an illegal move.
    BadRequest(&'static str),
    /// 403: the X-Player-Token header is missing or holds no seat's token.
    BadToken,
    /// 404: no such lobby (or it expired), or no such route.
    NotFound,
    /// 405
    MethodNotAllowed,
    /// 409: `lobby_full` or `stale_version`.
    Conflict(&'static str),
    /// 503: Redis didn't answer.
    Unavailable,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error) = match self {
            ApiError::BadRequest(error) => (StatusCode::BAD_REQUEST, error),
            ApiError::BadToken => (StatusCode::FORBIDDEN, "bad_token"),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not_found"),
            ApiError::MethodNotAllowed => (StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed"),
            ApiError::Conflict(error) => (StatusCode::CONFLICT, error),
            ApiError::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
        };
        (status, Json(json!({ "error": error }))).into_response()
    }
}

impl From<LobbyError> for ApiError {
    fn from(error: LobbyError) -> Self {
        match error {
            LobbyError::LobbyFull => ApiError::Conflict("lobby_full"),
            LobbyError::WaitingForOpponent => ApiError::BadRequest("waiting_for_opponent"),
            LobbyError::GameNotOver => ApiError::BadRequest("game_not_over"),
            LobbyError::Move(MoveError::GameOver) => ApiError::BadRequest("game_over"),
            LobbyError::Move(MoveError::NotYourTurn) => ApiError::BadRequest("not_your_turn"),
            LobbyError::Move(MoveError::InvalidCell) => ApiError::BadRequest("invalid_cell"),
            LobbyError::Move(MoveError::CellTaken) => ApiError::BadRequest("cell_taken"),
        }
    }
}

impl From<StoreError> for ApiError {
    fn from(error: StoreError) -> Self {
        tracing::warn!(%error, "lobby store failed");
        ApiError::Unavailable
    }
}
