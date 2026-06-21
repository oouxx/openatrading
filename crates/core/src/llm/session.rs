use std::sync::Arc;

use anyhow::Result;

use openatrading_agent::pi_ai_types::ContentBlock;
use openatrading_agent::types::{AgentMessage, DynTool};

use super::tools::create_decision_tool;
use super::LlmClient;

// ── Helper: one-shot agent invoke ────────────────────────────────────────

struct AgentResult {
    final_text: String,
    tool_output: Option<serde_json::Value>,
}

async fn invoke_agent(
    llm: &LlmClient,
    tools: Vec<Arc<DynTool>>,
    system_prompt: &str,
    user_message: &str,
) -> Result<AgentResult> {
    let agent = llm.create_agent(tools, system_prompt);

    let messages = vec![AgentMessage::User {
        content: vec![ContentBlock::text(user_message)],
        timestamp: 0,
    }];

    let result = agent
        .process(messages)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let final_text: String = result
        .iter()
        .rev()
        .find_map(|msg| match msg {
            AgentMessage::Assistant { content, .. } => {
                let text: String = content
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text, .. } => Some(text.as_str()),
                        ContentBlock::Thinking { thinking, .. } => Some(thinking.as_str()),
                        _ => None,
                    })
                    .collect();
                if text.is_empty() {
                    None
                } else {
                    Some(text)
                }
            },
            _ => None,
        })
        .unwrap_or_default();

    let tool_output = result.iter().rev().find_map(|msg| match msg {
        AgentMessage::Assistant { content, .. } => content.iter().find_map(|b| {
            if let ContentBlock::ToolCall { arguments, .. } = b {
                Some(arguments.clone())
            } else {
                None
            }
        }),
        _ => None,
    });

    Ok(AgentResult {
        final_text,
        tool_output,
    })
}

// ── AgentSession ─────────────────────────────────────────────────────────

/// Builder for a single agent invocation.
///
/// Wraps upstream `Agent` + decision-tool injection + result extraction into one
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
                    tracing::warn!(
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
                tracing::warn!(
                    "[session] attempt {}/{}: extractor returned empty trades, retrying",
                    attempt + 1,
                    max_retries
                );
                continue;
            }

            tracing::warn!(
                "[session] attempt {}/{}: no decision output, retrying",
                attempt + 1,
                max_retries
            );
        }

        // 3) Last resort — wrap raw text with empty trades
        tracing::error!(
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

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::tools;

    #[test]
    fn test_builder_defaults() {
        let llm = LlmClient::with_config("openrouter".into(), "test".into());
        let s = AgentSession::new(&llm, "test");
        assert!(s.tools.is_empty());
        assert!(!s.include_decision_tool);
    }

    #[test]
    fn test_builder_with_tools() {
        let llm = LlmClient::with_config("openrouter".into(), "test".into());
        let tool = tools::get_limit_up();
        let s = AgentSession::new(&llm, "test").with_tools(&[tool]);
        assert_eq!(s.tools.len(), 1);
        assert!(!s.include_decision_tool);
    }

    #[test]
    fn test_builder_with_decision_tool() {
        let llm = LlmClient::with_config("openrouter".into(), "test".into());
        let t = tools::get_limit_up();
        let s = AgentSession::new(&llm, "test")
            .with_tools(&[t])
            .with_decision_tool();
        assert_eq!(s.tools.len(), 1);
        assert!(s.include_decision_tool);
        // build_tools should include the decision tool
        let all = s.build_tools();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].name, "get_limit_up");
        assert_eq!(all[1].name, "submit_trading_decision");
    }
}
