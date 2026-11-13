//! Settings from environment variables. The default ports are the spec's.

use std::{net::SocketAddr, str::FromStr, time::Duration};

#[derive(Clone, Debug)]
pub struct Config {
    pub redis_url: String,
    /// The page and /api; the only port behind the Ingress.
    pub app_addr: SocketAddr,
    /// /livez, /readyz and /metrics; never exposed outside the cluster.
    pub ops_addr: SocketAddr,
    /// How long to keep serving after SIGTERM.
    pub drain: Duration,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// Reads each setting through `get`, so tests needn't change the real
    /// environment.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        Ok(Self {
            redis_url: get("REDIS_URL").unwrap_or_else(|| "redis://127.0.0.1:6379".to_owned()),
            app_addr: parse(&get, "APP_ADDR", "0.0.0.0:8080")?,
            ops_addr: parse(&get, "OPS_ADDR", "0.0.0.0:9090")?,
            drain: Duration::from_secs(parse(&get, "DRAIN_SECONDS", "5")?),
        })
    }
}

fn parse<T: FromStr>(
    get: &impl Fn(&str) -> Option<String>,
    name: &str,
    default: &str,
) -> Result<T, String> {
    let value = get(name).unwrap_or_else(|| default.to_owned());
    value
        .parse()
        .map_err(|_| format!("{name} is not valid: {value:?}"))
}
