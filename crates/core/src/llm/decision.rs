use anyhow::Result;

use super::agent_runtime::AgentRuntime;
use super::LlmClient;
use crate::roles::prompt::{extract_decision_output, DecisionOutput};

/// Run a trading decision with retry logic.
///
/// This is a convenience function on top of `AgentRuntime::run()`.
/// It handles:
/// - Decision tool extraction (preferred path)
/// - Extractor fallback (second LLM call)
/// - Retry with modified prompt (up to 3 attempts)
/// - Last-resort fallback (raw text + empty trades)
pub async fn run_decision(
    runtime: &AgentRuntime,
    session_id: &str,
    llm: &LlmClient,
    user_message: &str,
) -> Result<DecisionOutput> {
    let max_retries = 3;
    let mut last_text = String::new();

    for attempt in 0..max_retries {
        let msg = if attempt == 0 {
            user_message.to_string()
        } else {
            format!(
                "{}\n\n⚠️ 你没有给出具体的交易指令（买入/卖出/持有）。请立即基于当前市场数据和持仓情况，\
                给出至少一条具体的交易指令。必须是 submit_trading_decision，不得只说分析不操作。",
                last_text
            )
        };

        let result = runtime.run(session_id, &msg).await?;
        last_text = result.final_text.clone();

        // 1) Decision-tool path (preferred — no extra LLM call)
        for tv in &result.tool_outputs {
            if let Ok(output) = serde_json::from_value::<DecisionOutput>(tv.clone()) {
                if !output.trades.is_empty() {
                    return Ok(output);
                }
                tracing::warn!(
                    "[decision] attempt {}/{}: decision tool returned empty trades, retrying",
                    attempt + 1,
                    max_retries
                );
            }
        }

        // 2) Extractor fallback (one extra LLM call)
        if let Ok(output) = extract_decision_output(llm, &result.final_text).await {
            if !output.trades.is_empty() {
                return Ok(output);
            }
            tracing::warn!(
                "[decision] attempt {}/{}: extractor returned empty trades, retrying",
                attempt + 1,
                max_retries
            );
        }

        tracing::warn!(
            "[decision] attempt {}/{}: no decision output, retrying",
            attempt + 1,
            max_retries
        );
    }

    // 3) Last resort — wrap raw text with empty trades
    tracing::error!(
        "[decision] all {} attempts exhausted, returning raw text",
        max_retries
    );
    Ok(DecisionOutput {
        analysis: last_text,
        trades: Vec::new(),
    })
}
