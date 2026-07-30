//! Paper-account execution glue.
//!
//! [`PaperExecutor`] implements the extension crate's [`DecisionSink`] trait:
//! it deserializes the agent's `submit_trading_decision` args into the core
//! [`DecisionOutput`] domain type and executes each trade instruction on the
//! paper account via [`super::broker::execute_buy`] / [`execute_sell`].
//!
//! This module also implements [`PositionReader`] for [`TradeManager`] and
//! [`MemoryReader`] for [`MemoryStore`], bridging core's domain storage to the
//! extension traits without the extension crate depending on core.

use std::sync::Arc;

use async_trait::async_trait;

use openatrading_extension::{DecisionSink, MemoryReader, PositionReader};

use crate::api::Client as ApiClient;
use crate::memory::sqlite::MemoryStore;
use crate::roles::prompt::DecisionOutput;
use crate::trade::broker::{execute_buy, execute_sell};
use crate::trade::sqlite::TradeManager;
use crate::trade::TradeAction;

/// Paper-account decision executor.
///
/// Holds a reference to the shared [`TradeManager`] and a market-data
/// [`ApiClient`] (for realtime prices during simulated fills).
pub struct PaperExecutor {
    trade: Arc<TradeManager>,
    api: ApiClient,
}

impl PaperExecutor {
    #[must_use]
    pub fn new(trade: Arc<TradeManager>, api: ApiClient) -> Self {
        Self { trade, api }
    }
}

#[async_trait]
impl DecisionSink for PaperExecutor {
    async fn submit(&self, role: &str, args: serde_json::Value) -> Result<String, String> {
        let decision: DecisionOutput =
            serde_json::from_value(args).map_err(|e| format!("解析决策参数失败: {e}"))?;

        let mut lines: Vec<String> = Vec::new();
        for instr in &decision.trades {
            match instr.action {
                TradeAction::Buy => match execute_buy(&self.trade, &self.api, role, instr).await {
                    Ok(()) => lines.push(format!(
                        "✅ 买入 {} {} {}股 — {}",
                        instr.symbol, instr.name, instr.shares, instr.reason
                    )),
                    Err(e) => {
                        lines.push(format!("❌ 买入 {} {} 失败: {e}", instr.symbol, instr.name))
                    }
                },
                TradeAction::Sell => {
                    match execute_sell(&self.trade, &self.api, role, instr).await {
                        Ok(()) => lines.push(format!(
                            "✅ 卖出 {} {} — {}",
                            instr.symbol, instr.name, instr.reason
                        )),
                        Err(e) => {
                            lines.push(format!("❌ 卖出 {} {} 失败: {e}", instr.symbol, instr.name))
                        }
                    }
                }
                TradeAction::Hold => lines.push(format!(
                    "⏸ 持有 {} {} — {}",
                    instr.symbol, instr.name, instr.reason
                )),
                TradeAction::Open | TradeAction::Close => {
                    crate::log::warn!(
                        "[paper] unexpected action '{}' for {} {}",
                        instr.action,
                        instr.symbol,
                        instr.name
                    );
                }
            }
        }

        let trade_section = if lines.is_empty() {
            "无交易操作".to_string()
        } else {
            lines.join("\n")
        };
        Ok(format!(
            "📊 分析：\n{}\n\n💹 操作：\n{}",
            decision.analysis, trade_section
        ))
    }
}

#[async_trait]
impl PositionReader for TradeManager {
    async fn format_positions(&self, role: &str) -> Result<String, String> {
        TradeManager::format_positions(self, role).map_err(|e| e.to_string())
    }
}

#[async_trait]
impl MemoryReader for MemoryStore {
    async fn get_historical_context(&self, days: u32) -> Result<String, String> {
        MemoryStore::get_historical_context(self, days).map_err(|e| e.to_string())
    }
}
