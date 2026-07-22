//! Shared chat-session logic for the web (SSE) and Tauri (IPC channel)
//! backends.
//!
//! Both `chat::ai_chat` and `src-tauri`'s `api_chat_stream` drive the same
//! agent loop (`openatrading_core::llm::AgentRuntime`) and translate its
//! `AgentEvent`s into a frontend wire protocol. This module centralizes the
//! two pieces that were previously copy-pasted with subtle drift:
//!
//! 1. Command resolution — `role` / `command_type` / `command_name` /
//!    `skill_name` → `(system_prompt, tools)`. The Tauri copy had drifted and
//!    skipped the `allowed_tools` filter that the web copy applies.
//! 2. Event serialization — `AgentEvent` → a protocol-neutral `ChatFrame`
//!    that each transport renders into its own wire format.
//!
//! This mirrors the `serialize_event` / `mutate` centralization pattern from
//! the desktop `Store`: one source of truth, transports stay thin.

use std::sync::Arc;

use openatrading_core::commands::CommandType;
use openatrading_core::llm::{assistant_event_to_stream_events, AgentEvent, DynTool, StreamEvent};
use openatrading_core::skills::SkillRegistry;

use crate::RoleEntry;

/// Resolved agent configuration: the system prompt and the tool set the
/// agent loop runs with.
///
/// Returned by [`resolve_chat_config`] so callers don't reconstruct the
/// command→config mapping inline (the previous duplication lived in both
/// `ai_chat` and `api_chat_stream`).
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
                .map(|r| r.strategy.as_str())
                .unwrap_or(default_strategy);
            let sp = crate::chat::build_system_prompt(strategy_text);
            let filtered: Vec<Arc<DynTool>> = all_tools
                .iter()
                .filter(|t| t.name == effective_command_name)
                .cloned()
                .collect();
            (sp, filtered)
        },
        Some(CommandType::Skill) => {
            let skills = SkillRegistry::from_disk();
            let skill = skills
                .skills()
                .iter()
                .find(|s| s.name == effective_command_name);
            let prompt = skill
                .map(|s| s.content.as_str())
                .unwrap_or(default_strategy);
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
        },
        None => {
            let strategy_text = input
                .roles
                .iter()
                .find(|r| r.name == input.role_name)
                .map(|r| r.strategy.as_str())
                .unwrap_or(default_strategy);
            (crate::chat::build_system_prompt(strategy_text), all_tools)
        },
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
pub fn extract_tool_result_text(result: &serde_json::Value) -> String {
    let content = match result.get("content") {
        Some(serde_json::Value::Array(items)) => items,
        _ => return result.to_string(),
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

/// A protocol-neutral rendering of an `AgentEvent`, ready to be turned into a
/// transport-specific frame (SSE event / Tauri channel message).
///
/// Centralizing this is the `serialize_event` analog from the desktop
/// `Store`: both backends consume the same `ChatFrame` stream instead of
/// each re-implementing the `AgentEvent` → wire mapping.
#[derive(Debug, Clone)]
pub enum ChatFrame {
    /// Plain text delta from the assistant.
    Text(String),
    /// Reasoning / thinking delta from the assistant.
    Thinking(String),
    /// A tool call started (id, name, raw args JSON string).
    ToolCall {
        id: String,
        name: String,
        args: String,
    },
    /// A tool call argument delta (raw args JSON string).
    ToolDelta { args: String },
    /// A tool execution started.
    ToolExecStart,
    /// A tool execution intermediate update with partial result.
    ToolExecutionUpdate {
        id: String,
        name: String,
        args: String,
        partial_result: String,
    },
    /// A tool execution finished with its (already-extracted) result text.
    ToolExecEnd {
        id: String,
        name: String,
        result: String,
        is_error: bool,
    },
    /// A conversation turn started.
    TurnStart,
    /// A conversation turn ended with tool results.
    TurnEnd { tool_results: Vec<String> },
    /// An error surfaced during streaming.
    Error(String),
    /// The whole agent turn finished — receivers should close the stream.
    Done,
}

/// Translate a single `AgentEvent` into zero or more [`ChatFrame`]s.
///
/// `MessageUpdate` may expand to several frames (e.g. a text delta followed
/// by a tool-call start), so the result is a `Vec`. `AgentEnd` maps to a
/// single [`ChatFrame::Done`]; `MessageEnd` only emits a frame when it carries
/// an error message. Events the chat protocol doesn't surface return an empty
/// `Vec`.
pub fn serialize_chat_event(event: AgentEvent) -> Vec<ChatFrame> {
    match event {
        AgentEvent::MessageUpdate {
            assistant_message_event,
            ..
        } => {
            let mut frames = Vec::new();
            for se in assistant_event_to_stream_events(&assistant_message_event) {
                match se {
                    StreamEvent::Text(t) => frames.push(ChatFrame::Text(t)),
                    StreamEvent::ReasoningText(t) => frames.push(ChatFrame::Thinking(t)),
                    StreamEvent::ToolCallStart(tc) => frames.push(ChatFrame::ToolCall {
                        id: tc.id,
                        name: tc.function_name,
                        args: tc.arguments,
                    }),
                    StreamEvent::ToolCallDelta(tc) => {
                        frames.push(ChatFrame::ToolDelta { args: tc.arguments })
                    },
                    StreamEvent::Error(e) => frames.push(ChatFrame::Error(e)),
                }
            }
            frames
        },
        AgentEvent::MessageStart { .. } => {
            // 消息开始信号，前端可据此清空上一轮累积的流式缓冲区
            Vec::new()
        },
        AgentEvent::MessageEnd { message } => {
            if let openatrading_core::llm::AgentMessage::Assistant {
                error_message: Some(err),
                ..
            } = &message
            {
                if !err.is_empty() {
                    return vec![ChatFrame::Error(format!("[ERROR] {err}"))];
                }
            }
            Vec::new()
        },
        AgentEvent::AgentStart => {
            // 对话开始信号，前端可据此初始化对话状态
            Vec::new()
        },
        AgentEvent::AgentEnd { .. } => vec![ChatFrame::Done],
        AgentEvent::TurnStart => vec![ChatFrame::TurnStart],
        AgentEvent::TurnEnd { tool_results, .. } => {
            let results: Vec<String> = tool_results
                .iter()
                .map(|m| {
                    // Extract text content from each result message
                    if let openatrading_core::llm::AgentMessage::ToolResult { content, .. } = m {
                        content
                            .iter()
                            .filter_map(|b| {
                                if let openatrading_core::llm::pi_ai_types::ContentBlock::Text {
                                    text,
                                    ..
                                } = b
                                {
                                    Some(text.clone())
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    } else {
                        String::new()
                    }
                })
                .filter(|s| !s.is_empty())
                .collect();
            vec![ChatFrame::TurnEnd {
                tool_results: results,
            }]
        },
        AgentEvent::ToolExecutionStart { .. } => vec![ChatFrame::ToolExecStart],
        AgentEvent::ToolExecutionUpdate {
            tool_call_id,
            tool_name,
            args,
            partial_result,
        } => vec![ChatFrame::ToolExecutionUpdate {
            id: tool_call_id,
            name: tool_name,
            args: args.to_string(),
            partial_result: partial_result.to_string(),
        }],
        AgentEvent::ToolExecutionEnd {
            tool_call_id,
            tool_name,
            result,
            is_error,
        } => vec![ChatFrame::ToolExecEnd {
            id: tool_call_id,
            name: tool_name,
            result: extract_tool_result_text(&result),
            is_error,
        }],
    }
}
