use std::sync::Arc;

use anyhow::Result;

use pi_coding_agent::core::sdk::{self, CreateAgentSessionOptions, NoToolsMode};
use pi_coding_agent::pi_agent_core::types::DynTool;
use pi_coding_agent::pi_ai_types::ContentBlock;

use crate::llm::LlmClient;

// ── Helper: one-shot agent invoke ────────────────────────────────────────

pub struct AgentResult {
    pub final_text: String,
    pub tool_output: Option<serde_json::Value>,
}

pub async fn invoke_agent(
    llm: &LlmClient,
    trading_tools: Vec<Arc<DynTool>>,
    system_prompt: &str,
    user_message: &str,
) -> Result<AgentResult> {
    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let (mut session, _result) = sdk::create_agent_session(CreateAgentSessionOptions {
        cwd,
        model: Some(llm.model.clone()),
        no_tools: Some(NoToolsMode::All),
        custom_prompt: Some(system_prompt.to_string()),
        tools: Some(trading_tools.iter().map(|t| t.name.clone()).collect()),
        exclude_tools: None,
        custom_tools: None,
        persist_session: false,
        session_file: None,
        enable_extensions: true,
        extension_paths: Vec::new(),
        extension_registry: None,
        stream_fn: None,
        convert_to_llm: None,
        agent_dir: None,
        thinking_level: None,
        scoped_models: None,
        append_system_prompt: None,
        session_name: None,
        cli_provider: None,
        cli_model: None,
        fork_from: None,
        session_dir: None,
    })
    .await
    .map_err(|e| anyhow::anyhow!("create_agent_session failed: {e}"))?;

    // Tools have execute callbacks baked in — no add_tools() needed.
    // Add user message and run agent loop
    session.add_user_text(user_message).await;
    let messages = session.get_messages().await;

    let final_text: String = messages
        .iter()
        .rev()
        .filter_map(|msg| {
            if let pi_coding_agent::pi_agent_core::types::AgentMessage::Assistant {
                content, ..
            } = msg
            {
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
            } else {
                None
            }
        })
        .next()
        .unwrap_or_default();

    let tool_output = messages.iter().rev().find_map(|msg| {
        if let pi_coding_agent::pi_agent_core::types::AgentMessage::Assistant { content, .. } = msg
        {
            content.iter().find_map(|b| {
                if let ContentBlock::ToolCall { arguments, .. } = b {
                    Some(arguments.clone())
                } else {
                    None
                }
            })
        } else {
            None
        }
    });

    Ok(AgentResult {
        final_text,
        tool_output,
    })
}

// ── AgentSession wrapper (generic, kept for backward compat) ────────────

pub struct LlmAgentSession<'a> {
    llm: &'a LlmClient,
    system_prompt: &'a str,
    tools: Vec<Arc<DynTool>>,
}

impl<'a> LlmAgentSession<'a> {
    pub fn new(llm: &'a LlmClient, system_prompt: &'a str) -> Self {
        Self {
            llm,
            system_prompt,
            tools: Vec::new(),
        }
    }

    pub fn with_tools(mut self, tools: &[Arc<DynTool>]) -> Self {
        self.tools = tools.to_vec();
        self
    }

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
