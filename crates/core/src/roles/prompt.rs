use crate::trade::TradeAction;
use crate::types::StockCode;
use schemars::JsonSchema;

#[derive(Debug, Clone, serde::Deserialize, JsonSchema)]
pub struct TradeInstruction {
    pub action: TradeAction,
    pub symbol: StockCode,
    pub name: String,
    #[serde(default)]
    pub shares: i64,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, serde::Deserialize, JsonSchema)]
pub struct DecisionOutput {
    pub analysis: String,
    #[serde(default)]
    pub trades: Vec<TradeInstruction>,
}
