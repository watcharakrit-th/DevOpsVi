//! Lobbies in Redis: one key per lobby holding its JSON. Writes are a
//! compare-and-set on the version (a Lua script, atomic inside Redis), so the
//! two replicas can't overwrite each other's changes.

use std::{fmt, sync::LazyLock, time::Duration};

use redis::{
    Client, Script, Value,
    aio::{ConnectionManager, ConnectionManagerConfig},
};

use crate::lobby::{Lobby, LobbyCode};

/// Every write gives the lobby a fresh hour; Redis deletes it after that.
pub const LOBBY_TTL: Duration = Duration::from_secs(3600);

/// Writes ARGV[2] (with ARGV[3] seconds to live) only if the stored lobby
/// still has version ARGV[1]. Returns 1 when written, 0 when the version had
/// moved on and -1 when the lobby has expired.
static SAVE_IF_VERSION: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r"
        local current = redis.call('GET', KEYS[1])
        if not current then return -1 end
        if cjson.decode(current).version ~= tonumber(ARGV[1]) then return 0 end
        redis.call('SET', KEYS[1], ARGV[2], 'EX', ARGV[3])
        return 1
        ",
    )
});

#[derive(Clone)]
pub struct Store {
    redis: ConnectionManager,
}

#[derive(Debug)]
pub enum StoreError {
    Redis(redis::RedisError),
    /// A stored lobby didn't parse.
    Corrupt(serde_json::Error),
    /// Every random code tried was already taken.
    NoFreeCode,
}

/// What [`Store::save`] did.
#[derive(Debug, PartialEq, Eq)]
pub enum Save {
    Written,
    /// The stored version had moved on: someone else wrote first.
    Conflict,
    /// The lobby is gone (its hour ran out).
    Expired,
}

impl Store {
    /// Doesn't connect yet: the app starts even while Redis is down, and
    /// /readyz reports that until Redis answers.
    pub fn new(redis_url: &str) -> Result<Self, StoreError> {
        let config = ConnectionManagerConfig::new()
            .set_connection_timeout(Some(Duration::from_secs(1)))
            .set_response_timeout(Some(Duration::from_secs(1)))
            // Fail the request quickly; the next one reconnects anyway.
            .set_number_of_retries(1);
        let redis = ConnectionManager::new_lazy_with_config(Client::open(redis_url)?, config)?;
        Ok(Self { redis })
    }

    /// Stores a new lobby under a fresh random code and returns the code.
    pub async fn create(&self, lobby: &Lobby) -> Result<LobbyCode, StoreError> {
        let json = serde_json::to_string(lobby)?;
        // With 31^6 possible codes a clash is unlikely; on one, draw again.
        for _ in 0..5 {
            let code = LobbyCode::random();
            let reply: Value = redis::cmd("SET")
                .arg(key(&code))
                .arg(&json)
                .arg("NX")
                .arg("EX")
                .arg(LOBBY_TTL.as_secs())
                .query_async(&mut self.redis.clone())
                .await?;
            if reply != Value::Nil {
                return Ok(code);
            }
        }
        Err(StoreError::NoFreeCode)
    }

    pub async fn load(&self, code: &LobbyCode) -> Result<Option<Lobby>, StoreError> {
        let json: Option<String> = redis::cmd("GET")
            .arg(key(code))
            .query_async(&mut self.redis.clone())
            .await?;
        Ok(json.map(|json| serde_json::from_str(&json)).transpose()?)
    }

    /// Writes `lobby` only if the stored copy is still at `expected_version`.
    pub async fn save(
        &self,
        code: &LobbyCode,
        lobby: &Lobby,
        expected_version: u64,
    ) -> Result<Save, StoreError> {
        let outcome: i64 = SAVE_IF_VERSION
            .key(key(code))
            .arg(expected_version)
            .arg(serde_json::to_string(lobby)?)
            .arg(LOBBY_TTL.as_secs())
            .invoke_async(&mut self.redis.clone())
            .await?;
        Ok(match outcome {
            1 => Save::Written,
            0 => Save::Conflict,
            _ => Save::Expired,
        })
    }

    pub async fn ping(&self) -> Result<(), StoreError> {
        let _: String = redis::cmd("PING")
            .query_async(&mut self.redis.clone())
            .await?;
        Ok(())
    }
}

fn key(code: &LobbyCode) -> String {
    format!("lobby:{code}")
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Redis(error) => write!(f, "redis: {error}"),
            StoreError::Corrupt(error) => write!(f, "stored lobby doesn't parse: {error}"),
            StoreError::NoFreeCode => f.write_str("no free lobby code found"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<redis::RedisError> for StoreError {
    fn from(error: redis::RedisError) -> Self {
        StoreError::Redis(error)
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(error: serde_json::Error) -> Self {
        StoreError::Corrupt(error)
    }
}
