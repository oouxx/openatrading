use anyhow::Result;

use pi_coding_agent::core::sdk::{self, CreateAgentSessionOptions, NoToolsMode};
use pi_coding_agent::pi_ai_types::ContentBlock;
use pi_extension_api::ToolDefinition;

// ── Helper: one-shot agent invoke ────────────────────────────────────────

pub struct AgentResult {
    pub final_text: String,
    pub tool_output: Option<serde_json::Value>,
}

/// One-shot agent invocation using pi-coding-agent SDK + extension registry.
///
/// Creates an in-memory session (no persistence), runs the agent loop,
/// and returns the final text and any tool call output.
///
/// `custom_tools` are additional `ToolDefinition`s with execute callbacks
/// (e.g. per-role positions/memory tools) that are injected alongside
/// the extension-provided tools.
pub async fn invoke_agent(
    system_prompt: &str,
    user_message: &str,
    custom_tools: Vec<ToolDefinition>,
) -> Result<AgentResult> {
    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let ext_registry = openatrading_extension::create_registry();

    let custom_tools_opt = if custom_tools.is_empty() {
        None
    } else {
        Some(custom_tools)
    };

    let (mut session, _result) = sdk::create_agent_session(CreateAgentSessionOptions {
        cwd,
        agent_dir: None,
        model: None,
        thinking_level: None,
        scoped_models: None,
        no_tools: Some(NoToolsMode::All),
        tools: None,
        exclude_tools: None,
        custom_prompt: Some(system_prompt.to_string()),
        append_system_prompt: None,
        session_name: None,
        stream_fn: None,
        convert_to_llm: None,
        custom_tools: custom_tools_opt,
        extension_paths: Vec::new(),
        enable_extensions: true,
        extension_registry: Some(ext_registry),
        cli_provider: None,
        cli_model: None,
        persist_session: false,
        session_file: None,
        fork_from: None,
        session_dir: None,
    })
    .await
    .map_err(|e| anyhow::anyhow!("create_agent_session failed: {e}"))?;

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
