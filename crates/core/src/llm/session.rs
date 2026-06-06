use std::sync::Arc;

use anyhow::Result;

use super::tools::create_decision_tool;
use super::{agent_loop, DynTool, LlmClient};

/// Builder for a single agent invocation.
///
/// Wraps `agent_loop` + decision-tool injection + result extraction into one
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
    /// `extract_decision_output` (one extra LLM call), then raw text.
    pub async fn process_decision(
        &self,
        user_message: &str,
    ) -> Result<crate::roles::prompt::DecisionOutput> {
        let tools = self.build_tools();
        let result = agent_loop(self.llm, tools, self.system_prompt, user_message).await?;

        // 1) Decision-tool path (preferred — no extra LLM call)
        if let Some(ref tv) = result.tool_output {
            if let Ok(output) =
                serde_json::from_value::<crate::roles::prompt::DecisionOutput>(tv.clone())
            {
                return Ok(output);
            }
        }

        // 2) Extractor fallback (one extra LLM call)
        if let Ok(output) =
            crate::roles::prompt::extract_decision_output(self.llm, &result.final_text).await
        {
            return Ok(output);
        }

        // 3) Last resort — wrap raw text
        Ok(crate::roles::prompt::DecisionOutput {
            analysis: result.final_text,
            trades: Vec::new(),
        })
    }

    /// Run the agent and return the final text reply.
    pub async fn process_chat(&self, user_message: &str) -> Result<String> {
        let tools = self.build_tools();
        let result = agent_loop(self.llm, tools, self.system_prompt, user_message).await?;
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
