//! Lobbies in Redis. Stub for the failing tests.

#[derive(Clone)]
pub struct Store;

#[derive(Debug)]
pub enum StoreError {}

impl Store {
    pub fn new(_redis_url: &str) -> Result<Self, StoreError> {
        todo!()
    }
}
