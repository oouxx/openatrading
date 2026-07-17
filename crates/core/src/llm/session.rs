use std::sync::Arc;

use anyhow::Result;

use openatrading_agent::llm::LlmClient;
use openatrading_agent::session::invoke_agent;
use openatrading_agent::types::DynTool;

use super::tools::create_decision_tool;

// ── AgentSession (trading-specific) ─────────────────────────────────────

/// Builder for a single agent invocation with trading decision support.
///
/// Wraps upstream `invoke_agent` + decision-tool injection + retry logic into one
/// call, so entry points (cron, CLI, Telegram) don't repeat the
/// assemble → call → extract pattern.
///
/// # Example
///
/// ```rust,ignore
/// let output = AgentSession::new(&llm, &role.strategy)
///     .with_tools(registry.market_tools())
///     .with_decision_tool()
///     .process_decision(user_msg)
///     .await?;
/// ```
pub struct AgentSession<'a> {
    llm: &'a LlmClient,
    system_prompt: &'a str,
    tools: Vec<Arc<DynTool>>,
    include_decision_tool: bool,
}

impl<'a> AgentSession<'a> {
    pub fn new(llm: &'a LlmClient, system_prompt: &'a str) -> Self {
        Self {
            llm,
            system_prompt,
            tools: Vec::new(),
            include_decision_tool: false,
        }
    }

    /// Attach agent tools (market, positions, memory, …).
    pub fn with_tools(mut self, tools: &[Arc<DynTool>]) -> Self {
        self.tools = tools.to_vec();
        self
    }

    /// Also inject `submit_trading_decision` so the agent can return
    /// structured `DecisionOutput` in a single turn.
    pub fn with_decision_tool(mut self) -> Self {
        self.include_decision_tool = true;
        self
    }

    fn build_tools(&self) -> Vec<Arc<DynTool>> {
        let mut t = self.tools.clone();
        if self.include_decision_tool {
            t.push(create_decision_tool());
        }
        t
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
        let tools = self.build_tools();
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

            let result = invoke_agent(self.llm, tools.clone(), self.system_prompt, &msg).await?;
            last_text = result.final_text.clone();

            // 1) Decision-tool path (preferred — no extra LLM call)
            if let Some(ref tv) = result.tool_output {
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
                crate::roles::prompt::extract_decision_output(self.llm, &result.final_text).await
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
        let tools = self.build_tools();
        let result = invoke_agent(self.llm, tools, self.system_prompt, user_message).await?;
        Ok(result.final_text)
    }
}
