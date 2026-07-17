use std::sync::Arc;

use anyhow::Result;

use pi_agent_core::pi_ai_types::ContentBlock;
use pi_agent_core::types::{AgentMessage, DynTool};

use crate::llm::LlmClient;

// ── Helper: one-shot agent invoke ────────────────────────────────────────

pub struct AgentResult {
    pub final_text: String,
    pub tool_output: Option<serde_json::Value>,
}

pub async fn invoke_agent(
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

// ── AgentSession (generic, no trading-specific logic) ────────────────────

/// Builder for a single agent invocation.
///
/// Wraps upstream `Agent` + result extraction into one call.
pub struct AgentSession<'a> {
    llm: &'a LlmClient,
    system_prompt: &'a str,
    tools: Vec<Arc<DynTool>>,
}

impl<'a> AgentSession<'a> {
    pub fn new(llm: &'a LlmClient, system_prompt: &'a str) -> Self {
        Self {
            llm,
            system_prompt,
            tools: Vec::new(),
        }
    }

    /// Attach agent tools.
    pub fn with_tools(mut self, tools: &[Arc<DynTool>]) -> Self {
        self.tools = tools.to_vec();
        self
    }

    /// Run the agent and return the final text reply.
    pub async fn process_chat(&self, user_message: &str) -> Result<String> {
        let result = invoke_agent(
            self.llm,
            self.tools.clone(),
            self.system_prompt,
            user_message,
        )
        .await?;
        Ok(result.final_text)
    }
}
