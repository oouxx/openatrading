use crate::types::StockCode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum TradeAction {
    Buy,
    Sell,
    Hold,
    Open,
    Close,
}

impl TradeAction {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Buy => "buy",
            Self::Sell => "sell",
            Self::Hold => "hold",
            Self::Open => "open",
            Self::Close => "close",
        }
    }
}

impl std::fmt::Display for TradeAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub id: i64,
    pub role_name: String,
    pub symbol: StockCode,
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
    pub action: TradeAction,
    pub price: f64,
    pub shares: i64,
    pub reason: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TradeLogEntry {
    pub id: i64,
    pub position_id: i64,
    pub action: TradeAction,
    pub price: f64,
    pub shares: i64,
    pub reason: String,
    pub created_at: String,
    pub symbol: StockCode,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClosedPosition {
    pub position_id: i64,
    pub symbol: StockCode,
    pub stock_name: String,
    pub entry_date: String,
    pub exit_date: String,
    pub entry_price: f64,
    pub exit_price: f64,
    pub shares: i64,
    pub pnl_pct: f64,
    pub entry_reason: String,
    pub exit_reason: String,
}

pub mod broker;
pub mod paper;
pub mod paper_executor;
pub mod sqlite;

pub use broker::execute_buy;
pub use broker::execute_sell;
pub use paper_executor::PaperExecutor;
pub use sqlite::TradeManager;
