use crate::trade::TradeAction;
use crate::types::StockCode;
use openatrading_agent::extraction::Extractor;
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

pub async fn extract_decision_output(text: &str) -> anyhow::Result<DecisionOutput> {
    let llm = crate::llm::LlmClient::from_config(None, None);
    let extractor = Extractor::<DecisionOutput>::new(llm.model.clone())
        .with_api_key(&llm.api_key)
        .with_system_prompt(crate::prompts::EXTRACTION_SYSTEM_PROMPT)
        .with_tool_name("extract_trading_decision")
        .with_tool_description("Extract structured trading decision data")
        .with_retries(3);

    let result = extractor
        .extract(text)
        .await
        .map_err(|e| anyhow::anyhow!("[decision] extraction failed: {e}"))?;

    Ok(result)
}
