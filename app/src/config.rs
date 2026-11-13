//! Settings from environment variables. Stub for the failing tests.

use std::{net::SocketAddr, time::Duration};

#[derive(Clone, Debug)]
pub struct Config {
    pub redis_url: String,
    pub app_addr: SocketAddr,
    pub ops_addr: SocketAddr,
    pub drain: Duration,
}

impl Config {
    pub fn from_lookup(_get: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        todo!()
    }
}
