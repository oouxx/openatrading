//! Shared chat-session logic for the WebSocket (web) and Tauri IPC (desktop)
//! backends.
//!
//! Both `agent_ws::ws_handler` and `src-tauri`'s `send_prompt` drive the same
//! agent loop (`openatrading_core::llm::AgentRuntime`) and translate its
//! events into a frontend wire protocol. This module centralizes the
//! two pieces that were previously copy-pasted with subtle drift:
//!
//! 1. Command resolution — `role` / `command_type` / `command_name` /
//!    `skill_name` → `(system_prompt, tools)`.
//! 2. Event serialization — `AgentEvent` → a protocol-neutral frame that
//!    each transport renders into its own wire format. The frame type is
//!    `pi_agent_core::types::AgentEvent` itself — no custom `ChatFrame`
//!    wrapper. Consumers decompose `MessageUpdate` via
//!    the `AssistantMessageEvent` variants directly.
//!
//! This mirrors the centralization pattern from
//! the desktop `Store`: one source of truth, transports stay thin.

use std::sync::Arc;

use openatrading_core::commands::CommandType;
use openatrading_core::llm::DynTool;
use openatrading_core::skills::SkillRegistry;

use crate::RoleEntry;

/// Resolved agent configuration: the system prompt and the tool set the
/// agent loop runs with.
///
/// Returned by [`resolve_chat_config`] so callers don't reconstruct the
/// command→config mapping inline (the previous duplication lived in both
/// the web and Tauri handlers).
pub struct ChatConfig {
    pub system_prompt: String,
    pub tools: Vec<Arc<DynTool>>,
}

/// Inputs needed to resolve a chat configuration.
///
/// Bundled into a struct because the resolved form depends on four
/// caller-supplied fields plus the role list — more than three args, so the
/// project's function-signature rules require a struct.
///
/// `all_tools` is the live tool set (e.g. from `AppContext::tool_registry`),
/// so runtime-loaded MCP tools are included. The caller owns resolution of
/// the registry; this module only filters it.
pub struct ChatConfigInput<'a> {
    pub role_name: &'a str,
    pub command_type: Option<CommandType>,
    pub command_name: Option<&'a str>,
    pub skill_name: Option<&'a str>,
    pub roles: &'a [RoleEntry],
    pub all_tools: Vec<Arc<DynTool>>,
}

/// Resolve the effective command type/name and build the `(system_prompt,
/// tools)` pair.
///
/// `command_type`/`command_name` take precedence; if absent, a present
/// `skill_name` routes to the `Skill` command. The `Skill` branch applies the
/// skill's `allowed_tools` filter (the Tauri copy previously skipped this).
#[must_use]
pub fn resolve_chat_config(input: ChatConfigInput<'_>) -> ChatConfig {
    let default_strategy = openatrading_core::prompts::DEFAULT_CHAT_SYSTEM_PROMPT;

    let effective_command_type = input
        .command_type
        .clone()
        .or_else(|| input.skill_name.map(|_| CommandType::Skill));
    let effective_command_name = input.command_name.or(input.skill_name).unwrap_or("");

    let all_tools = input.all_tools;

    let (system_prompt, tools) = match effective_command_type {
        Some(CommandType::Tool) => {
            let strategy_text = input
                .roles
                .iter()
                .find(|r| r.name == input.role_name)
                .map_or(default_strategy, |r| r.strategy.as_str());
            let sp = crate::chat::build_system_prompt(strategy_text);
            let filtered: Vec<Arc<DynTool>> = all_tools
                .iter()
                .filter(|t| t.name == effective_command_name)
                .cloned()
                .collect();
            (sp, filtered)
        }
        Some(CommandType::Skill) => {
            let skills = SkillRegistry::from_disk();
            let skill = skills
                .skills()
                .iter()
                .find(|s| s.name == effective_command_name);
            let prompt = skill.map_or(default_strategy, |s| s.content.as_str());
            let allowed = skill
                .and_then(|s| s.allowed_tools.as_ref())
                .map(|s| s.split_whitespace().map(String::from).collect::<Vec<_>>())
                .filter(|v| !v.is_empty());
            let filtered: Vec<Arc<DynTool>> = match allowed {
                Some(names) => all_tools
                    .iter()
                    .filter(|t| names.contains(&t.name))
                    .cloned()
                    .collect(),
                None => all_tools,
            };
            (crate::chat::build_system_prompt(prompt), filtered)
        }
        Some(CommandType::Slash) => {
            // Slash commands are handled by the pi-coding-agent runtime directly.
            // Use the default system prompt and pass all tools (the runtime will
            // decide which to invoke based on the slash command handler).
            let sp = crate::chat::build_system_prompt(default_strategy);
            (sp, all_tools)
        }
        None => {
            let strategy_text = input
                .roles
                .iter()
                .find(|r| r.name == input.role_name)
                .map_or(default_strategy, |r| r.strategy.as_str());
            (crate::chat::build_system_prompt(strategy_text), all_tools)
        }
    };

    ChatConfig {
        system_prompt,
        tools,
    }
}

/// Extract the human-readable text from a serialized `AgentToolResult`.
///
/// The `ToolExecutionEnd` event carries `result` as the full
/// `AgentToolResult` JSON (`{ content: [...], details, terminate }`).
/// The frontend tool-call block expects a plain string, so we concatenate
/// the `text` of every `content` block. Falls back to the raw JSON string
/// when the shape is unexpected.
///
/// Single source of truth — previously duplicated in `chat.rs` and
/// `src-tauri/src/main.rs`.
#[must_use]
pub fn extract_tool_result_text(result: &serde_json::Value) -> String {
    let Some(serde_json::Value::Array(content)) = result.get("content") else {
        return result.to_string();
    };
    let mut texts: Vec<String> = Vec::new();
    for item in content {
        if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
            texts.push(text.to_string());
        }
    }
    if texts.is_empty() {
        return result.to_string();
    }
    texts.join("\n")
}
