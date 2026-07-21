use super::types::DesktopState;
use anyhow::Result;
// ── AgentSession (trading-specific) ─────────────────────────────────────

/// Builder for a single agent invocation with trading decision support.
///
/// Wraps the unified `AgentRuntime` (create_session → subscribe → send →
/// collect) plus retry logic into one call, so entry points (cron, CLI,
/// Telegram) don't repeat the assemble → call → extract pattern.
///
/// All tools (market data, `submit_trading_decision`, etc.) are provided by
/// `MarketDataExtension` via the `ExtensionRegistry` — no manual tool injection.
///
/// # Example
///
/// ```rust,ignore
/// let output = AgentSession::new(&ctx.runtime, &role.strategy)
///     .process_decision(user_msg)
///     .await?;
/// ```
pub struct AgentSession<'a> {
    runtime: &'a crate::llm::AgentRuntime,
    system_prompt: &'a str,
}

impl<'a> AgentSession<'a> {
    pub fn new(runtime: &'a crate::llm::AgentRuntime, system_prompt: &'a str) -> Self {
        Self {
            runtime,
            system_prompt,
        }
    }

    /// Run the agent and extract structured `DecisionOutput`.
    ///
    /// Prefers the decision-tool path (single LLM turn).  Falls back to
    /// `extract_decision_output` (one extra LLM call).  If no trades are
    /// produced, re-prompts the agent (max 3 attempts) to force a decision.
    pub async fn process_decision(
        &self,
        user_message: &str,
    ) -> Result<crate::roles::prompt::DecisionOutput> {
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

            let mut handle = self
                .runtime
                .create_session(self.system_prompt, Vec::new(), false, None, None)
                .await?;
            let result = handle.run_with_timeout(&msg, 300).await?;
            last_text = result.final_text.clone();

            // 1) Decision-tool path (preferred — no extra LLM call)
            if let Some(ref tv) = result.tool_outputs.into_iter().next() {
                if let Ok(output) =
                    serde_json::from_value::<crate::roles::prompt::DecisionOutput>(tv.clone())
                {
                    if !output.trades.is_empty() {
                        return Ok(output);
                    }
                    crate::log::warn!(
                        "[session] attempt {}/{}: decision tool returned empty trades, retrying",
                        attempt + 1,
                        max_retries
                    );
                    continue;
                }
            }

            // 2) Extractor fallback (one extra LLM call)
            if let Ok(output) =
                crate::roles::prompt::extract_decision_output(&result.final_text).await
            {
                if !output.trades.is_empty() {
                    return Ok(output);
                }
                crate::log::warn!(
                    "[session] attempt {}/{}: extractor returned empty trades, retrying",
                    attempt + 1,
                    max_retries
                );
                continue;
            }

            crate::log::warn!(
                "[session] attempt {}/{}: no decision output, retrying",
                attempt + 1,
                max_retries
            );
        }

        // 3) Last resort — wrap raw text with empty trades
        crate::log::error!(
            "[session] all {} attempts exhausted, returning raw text",
            max_retries
        );
        Ok(crate::roles::prompt::DecisionOutput {
            analysis: last_text,
            trades: Vec::new(),
        })
    }

    /// Run the agent and return the final text reply.
    pub async fn process_chat(&self, user_message: &str) -> Result<String> {
        let mut handle = self
            .runtime
            .create_session(self.system_prompt, Vec::new(), false, None, None)
            .await?;
        let result = handle.run_with_timeout(user_message, 300).await?;
        Ok(result.final_text)
    }
}
/// Select a session by ID.
pub fn select_session_by_id(state: &mut DesktopState, session_id: &str) {
    state.selected_session_id = session_id.to_string();
}
