//! Trading-state bridge traits.
//!
//! The extension crate must not depend on `openatrading-core` (core depends on
//! extension, so a reverse dependency would cycle). To let the paper-trading
//! tools read positions / memory and submit decisions, core implements these
//! traits and hands a [`TradingContext`] to [`crate::create_registry`].
//!
//! `DecisionSink::submit` takes raw tool args (`serde_json::Value`) instead of
//! a core domain type — core's `PaperExecutor` deserializes them into its own
//! `DecisionOutput`, keeping the type boundary clean.

use async_trait::async_trait;
use std::sync::Arc;

/// Read formatted positions for a trading account (role).
#[async_trait]
pub trait PositionReader: Send + Sync {
    /// Return a human-readable position summary, or an error message.
    async fn format_positions(&self, role: &str) -> Result<String, String>;
}

/// Read historical market context from memory.
#[async_trait]
pub trait MemoryReader: Send + Sync {
    /// Return a historical-context summary for the last `days` days.
    async fn get_historical_context(&self, days: u32) -> Result<String, String>;
}

/// Receive a structured trading decision and execute it (paper account).
///
/// `args` is the raw JSON the agent passed to `submit_trading_decision`; the
/// implementation is responsible for deserializing it into its domain type.
#[async_trait]
pub trait DecisionSink: Send + Sync {
    /// Execute the decision and return a summary string.
    async fn submit(&self, role: &str, args: serde_json::Value) -> Result<String, String>;
}

/// Bundled trading state handed to the extension registry.
///
/// All sessions created from a factory built with `Some(TradingContext)` get
/// the paper-trading tools; `None` yields a market-data-only registry.
#[derive(Clone)]
pub struct TradingContext {
    /// Trading account label (e.g. `"paper"`).
    pub role: String,
    pub positions: Arc<dyn PositionReader>,
    pub memory: Arc<dyn MemoryReader>,
    pub sink: Arc<dyn DecisionSink>,
}
