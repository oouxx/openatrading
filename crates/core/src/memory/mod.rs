pub mod sqlite;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketSnapshot {
    pub id: i64,
    pub captured_at: String,
    pub data_source: String,
    pub raw_json: String,
}

pub trait MemoryStore: Send + Sync {
    fn save_snapshot(&self, source: &str, json: &str) -> Result<i64>;
    fn query_recent(&self, days: u32) -> Result<Vec<MarketSnapshot>>;
    fn query_by_source(&self, source: &str, days: u32) -> Result<Vec<MarketSnapshot>>;
    fn cleanup_old(&self) -> Result<()>;
    fn get_historical_context(&self, days: u32) -> Result<String>;
}
