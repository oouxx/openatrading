use serde::{Deserialize, Serialize};

pub mod sqlite;

pub use sqlite::MemoryStore;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketSnapshot {
    pub id: i64,
    pub captured_at: String,
    pub data_source: String,
    pub raw_json: String,
}
