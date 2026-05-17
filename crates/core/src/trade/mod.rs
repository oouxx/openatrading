use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub id: i64,
    pub role_name: String,
    pub symbol: String,
    pub stock_name: String,
    pub entry_date: String,
    pub cost_price: f64,
    pub shares: i64,
    pub current_price: Option<f64>,
    pub stop_loss: Option<f64>,
    pub target_price: Option<f64>,
    pub entry_reason: String,
    pub sector: String,
    pub position_pct: f64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeLog {
    pub id: i64,
    pub position_id: i64,
    pub action: String,
    pub price: f64,
    pub shares: i64,
    pub reason: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TradeLogEntry {
    pub id: i64,
    pub position_id: i64,
    pub action: String,
    pub price: f64,
    pub shares: i64,
    pub reason: String,
    pub created_at: String,
    pub symbol: String,
}

pub mod sqlite;

pub trait TradeManager: Send + Sync {
    fn list_positions(&self, role_name: &str) -> Result<Vec<Position>>;
    fn add_position(&self, pos: Position) -> Result<i64>;
    fn update_position(&self, pos: Position) -> Result<()>;
    fn close_position(&self, id: i64, price: f64, reason: &str) -> Result<()>;
    fn get_trade_logs(&self, position_id: i64) -> Result<Vec<TradeLog>>;
    fn get_trade_logs_by_role(&self, role_name: &str, limit: usize) -> Result<Vec<TradeLogEntry>>;
    fn format_positions(&self, role_name: &str) -> Result<String>;
    fn delete_positions_by_role(&self, role_name: &str) -> Result<usize>;
    fn delete_trade_logs_by_role(&self, role_name: &str) -> Result<usize>;
    fn get_all_trade_logs_by_role(&self, role_name: &str) -> Result<Vec<TradeLogEntry>>;
}
