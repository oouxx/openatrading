use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use openatrading_core::api::Client as ApiClient;
use openatrading_core::commands::{CommandSpec, CommandType};
use openatrading_core::skills::SkillEntry;
use openatrading_core::skills::SkillRequest;

use crate::web::OverviewResponse;
use crate::AppState;

/// Find a session JSONL file by session ID.
///
/// Session files are named `{timestamp}_{id}.jsonl`, so we cannot construct
/// the path from the ID alone. This scans the session directory for a file
/// whose name ends with `_{id}.jsonl`.
pub fn find_session_file(id: &str) -> Option<std::path::PathBuf> {
    let dir = pi_coding_agent::config::get_sessions_dir();
    let suffix = format!("_{id}.jsonl");
    let direct = dir.join(format!("{id}.jsonl"));
    if direct.exists() {
        return Some(direct);
    }
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.ends_with(&suffix) {
                    return Some(path);
                }
            }
        }
    }
    None
}

#[derive(Serialize)]
pub struct AiRoleItem {
    pub name: String,
    pub display_name: String,
}

pub async fn get_ai_commands() -> Json<Vec<CommandSpec>> {
    let data = crate::handlers::get_ai_commands_data()
        .await
        .unwrap_or_default();
    Json(serde_json::from_value(data).unwrap_or_default())
}

/// GET /api/agent/settings — pi-rs default provider/model/thinking level.
pub async fn get_agent_settings() -> Result<axum::Json<serde_json::Value>, axum::http::StatusCode> {
    crate::handlers::get_pi_agent_settings_data()
        .map(axum::Json)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /api/agent/models — pi-rs 可选模型列表（provider + models）。
pub async fn get_agent_models() -> Result<axum::Json<serde_json::Value>, axum::http::StatusCode> {
    crate::handlers::get_pi_agent_models_data()
        .map(axum::Json)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)
}

/// POST /api/agent/settings — update pi-rs defaults.
#[derive(serde::Deserialize)]
pub struct UpdateAgentSettingsRequest {
    #[serde(default)]
    default_provider: Option<String>,
    #[serde(default)]
    default_model: Option<String>,
    #[serde(default)]
    thinking_level: Option<String>,
}

pub async fn update_agent_settings(
    axum::Json(req): axum::Json<UpdateAgentSettingsRequest>,
) -> Result<axum::Json<serde_json::Value>, axum::http::StatusCode> {
    crate::handlers::update_pi_agent_settings_data(
        req.default_provider.as_deref(),
        req.default_model.as_deref(),
        req.thinking_level.as_deref(),
    )
    .map(|_| axum::Json(serde_json::json!({ "ok": true })))
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(Serialize)]
pub struct ToolResponse {
    pub name: String,
    pub description: String,
}

pub async fn get_skills(State(_state): State<Arc<AppState>>) -> Json<Vec<SkillEntry>> {
    Json(openatrading_core::skills::list_skill_entries())
}

pub async fn get_tools(State(state): State<Arc<AppState>>) -> Json<Vec<ToolResponse>> {
    let registry = &state.ctx.tool_registry;
    let tools = registry
        .load()
        .all_tools()
        .iter()
        .map(|t| ToolResponse {
            name: t.name.clone(),
            description: t.description.clone(),
        })
        .collect();
    Json(tools)
}

/// POST /api/skills — create or update a skill
pub async fn save_skill(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<SkillRequest>,
) -> impl IntoResponse {
    if let Err(e) = openatrading_core::skills::validate_skill_name(&req.name) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e})),
        )
            .into_response();
    }
    if let Err(e) = openatrading_core::skills::install_skill(&req) {
        openatrading_core::log::error!("failed to save skill: {e}");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "failed to save skill"})),
        )
            .into_response();
    }
    let saved = openatrading_core::skills::list_skill_entries()
        .into_iter()
        .find(|s| s.name == req.name);
    (StatusCode::OK, Json(saved)).into_response()
}

/// DELETE /api/skills/:name — delete a skill
pub async fn delete_skill(
    State(_state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match openatrading_core::skills::uninstall_skill(&name) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            openatrading_core::log::error!("failed to delete skill '{name}': {e}");
            StatusCode::NOT_FOUND.into_response()
        }
    }
}

/// GET /api/ai/roles — list all available AI roles.
///
/// # Errors
/// Returns `StatusCode` on internal errors (unlikely for this read-only endpoint).
pub async fn get_ai_roles(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::handlers::get_ai_roles_data(&state)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub fn build_user_message(
    overview: &OverviewResponse,
    field_items: Option<&str>,
    super_stocks: Option<&str>,
) -> String {
    let field_section = field_items
        .map(|s| format!("\n## 韭研公社异动数据\n{s}\n"))
        .unwrap_or_default();

    let super_section = super_stocks
        .map(|s| format!("\n## 强势股数据（未涨停但强）\n{s}\n"))
        .unwrap_or_default();

    openatrading_core::prompts::build_web_chat_user_message(
        &openatrading_core::prompts::WebChatMarketData {
            date: &overview.date,
            rise: overview.rise_count,
            fall: overview.fall_count,
            limit_up: overview.limit_up_count,
            limit_down: overview.limit_down_count,
            bomb_rate: overview.bomb_rate,
            risk_level: &overview.risk_level,
            summary: &overview.decision_summary,
            field_items: &field_section,
            super_stocks: &super_section,
        },
    )
}

pub async fn fetch_field_json(api: &ApiClient) -> Option<String> {
    let date = openatrading_core::utils::time::beijing_now()
        .format("%Y-%m-%d")
        .to_string();
    match api.jiuyan.fetch_field(&date, 1).await {
        Ok(plates) if !plates.is_empty() => serde_json::to_string(&plates).ok(),
        _ => None,
    }
}

pub async fn fetch_super_stock_json(api: &ApiClient) -> Option<String> {
    match api.xuangubao.fetch_super_stock().await {
        Ok(stocks) if !stocks.is_empty() => serde_json::to_string(&stocks).ok(),
        _ => None,
    }
}

/// Fetch overview data from the API client.
///
/// # Errors
/// Returns an error if the API request fails.
pub async fn fetch_overview_data(api: &ApiClient) -> anyhow::Result<OverviewResponse> {
    let overview = api.xuangubao.fetch_market_overview().await?;

    let ratio = f64::from(overview.rise_count) / f64::from(overview.fall_count.max(1));
    let risk_level = if ratio < 0.8 && overview.bomb_rate > 0.3 {
        "高"
    } else if ratio < 1.2 || overview.bomb_rate > 0.2 {
        "中"
    } else {
        "低"
    };

    let decision_summary = format!(
        "风险等级：{}；上涨{}家，下跌{}家，涨停{}只，跌停{}只",
        risk_level,
        overview.rise_count,
        overview.fall_count,
        overview.limit_up_count,
        overview.limit_down_count,
    );

    Ok(OverviewResponse {
        date: openatrading_core::utils::time::beijing_now()
            .format("%Y-%m-%d %H:%M")
            .to_string(),
        rise_count: overview.rise_count,
        fall_count: overview.fall_count,
        limit_up_count: overview.limit_up_count,
        limit_down_count: overview.limit_down_count,
        bomb_rate: overview.bomb_rate,
        risk_level: risk_level.to_string(),
        decision_summary,
        market_temperature: overview.market_temperature,
        yesterday_limit_up_avg_pcp: overview.yesterday_limit_up_avg_pcp,
    })
}

// ── Slash command execution ────────────────────────────────────────────

/// Execute a slash command on the given agent session.
///
/// Returns the response text to stream back to the client, or an error message.
pub use openatrading_core::llm::{handle_slash_command, SESSION_SLASH_COMMANDS};

/// If `text` is a session-level slash command we handle locally, return its
/// `(name, args)`. Returns `None` for `/skill:`, unknown commands, and non-slash
/// input so they reach pi-rs `prompt()` (skill expansion / normal turn).
pub fn parse_session_slash_command(text: &str) -> Option<(String, String)> {
    let (cmd, args) = pi_coding_agent::core::slash_commands::parse_slash_command(text)?;
    if SESSION_SLASH_COMMANDS.contains(&cmd) {
        Some((cmd.to_string(), args.to_string()))
    } else {
        None
    }
}

/// Run a session-level slash command against the pooled session, returning a
/// `Notice` payload. Shared by the WebSocket and Tauri paths.
pub async fn run_session_slash_command(
    pool: &openatrading_core::llm::SessionPool,
    session_id: &str,
    cmd_name: &str,
    cmd_args: &str,
) -> openatrading_core::llm::SessionEventPayload {
    if let Some(handle) = pool.get_session(session_id).await {
        match handle.slash(cmd_name, cmd_args).await {
            Ok(text) => openatrading_core::llm::SessionEventPayload::Notice {
                text,
                is_error: false,
            },
            Err(e) => openatrading_core::llm::SessionEventPayload::Notice {
                text: format!("[Slash Error] {e}"),
                is_error: true,
            },
        }
    } else {
        openatrading_core::llm::SessionEventPayload::Notice {
            text: "session not found".to_string(),
            is_error: true,
        }
    }
}

// ── Session listing (pi-coding-agent JSONL sessions) ────────────────────

/// GET /api/ai/sessions — list all saved sessions
pub async fn list_sessions(State(state): State<Arc<AppState>>) -> Json<Vec<serde_json::Value>> {
    let data = crate::handlers::list_sessions_data(&state)
        .await
        .unwrap_or_default();
    Json(serde_json::from_value(data).unwrap_or_default())
}

#[derive(Serialize)]
pub struct SessionMessage {
    pub role: String,
    pub content: String,
    pub created_at: String,
    /// Structured content blocks (thinking / tool_use / text). Populated for
    /// assistant messages so the frontend can render tool calls and reasoning
    /// when loading history (finding #10). Empty for user messages, which
    /// only carry plain text in `content`.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub blocks: Vec<HistoryBlock>,
}

/// Serializable subset of a persisted content block for history replay.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HistoryBlock {
    Thinking {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
        /// Tool result text, attached when the corresponding `toolResult` message
        /// is present in the session history (joined by `tool_call_id`).
        #[serde(skip_serializing_if = "Option::is_none", default)]
        result: Option<String>,
        /// Whether the tool result reported an error.
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        is_error: bool,
    },
    Text {
        text: String,
    },
    Image {
        /// Raw base64 image data (no `data:image/…;base64,` prefix).
        data: String,
        /// Media type, e.g. `image/png`.
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
}

/// Read the message history of a session **directly from its JSONL file**,
/// without materializing an `AgentSessionRuntime` in the pool.
///
/// This is a pure read with no side effects: it does not touch the LRU
/// `SessionPool` (so loading history for an idle session cannot evict a
/// live one) and does not mutate the agent's in-memory state. It reuses
/// pi-rs's `SessionManager::open` + `get_entries` to parse the JSONL — the
/// same source of truth `AgentSession::load_messages_from_session` uses —
/// rather than re-implementing JSONL parsing.
///
/// `SessionManager::open` and file parsing are synchronous, so the work is
/// offloaded to `spawn_blocking` to avoid stalling the async executor.
pub(crate) fn read_messages_from_disk(
    session_id: &str,
) -> Vec<openatrading_core::llm::AgentMessage> {
    use openatrading_core::llm::AgentMessage;
    use pi_coding_agent::core::session_manager::{SessionEntry, SessionManager};

    let Some(path) = find_session_file(session_id) else {
        return Vec::new();
    };
    // `open` extracts cwd from the session header and loads + migrates entries.
    let mgr = SessionManager::open(&path.to_string_lossy(), None, None);
    mgr.get_entries()
        .iter()
        .filter_map(|entry| match entry {
            SessionEntry::Message { message, .. } => {
                serde_json::from_value::<AgentMessage>(message.clone()).ok()
            }
            _ => None,
        })
        .collect()
}

/// Turn a slice of `AgentMessage`s into the serializable history shape,
/// joining `ToolResult` messages onto the `ToolUse` block that issued the call
/// (keyed by `tool_call_id`) so the frontend can render results inline.
pub(crate) fn messages_to_session_messages(
    messages: &[openatrading_core::llm::AgentMessage],
) -> Vec<SessionMessage> {
    use openatrading_core::llm::pi_ai_types::ContentBlock;
    use openatrading_core::llm::AgentMessage;
    use std::collections::HashMap;

    let tool_results: HashMap<String, (String, bool)> = messages
        .iter()
        .filter_map(|msg| match msg {
            AgentMessage::ToolResult {
                tool_call_id,
                content,
                is_error,
                ..
            } => {
                let text = content
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text, .. } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                Some((tool_call_id.clone(), (text, *is_error)))
            }
            _ => None,
        })
        .collect();

    messages
        .iter()
        .filter_map(|msg| {
            let (role, content_blocks, timestamp) = match msg {
                AgentMessage::User { content, timestamp } => ("user", content, *timestamp),
                AgentMessage::Assistant {
                    content, timestamp, ..
                } => ("assistant", content, *timestamp),
                // ToolResult messages are joined onto ToolUse blocks above,
                // not emitted as standalone history entries.
                _ => return None,
            };

            let mut text_parts: Vec<String> = Vec::new();
            let mut blocks: Vec<HistoryBlock> = Vec::new();

            for block in content_blocks {
                match block {
                    ContentBlock::Text { text, .. } => {
                        text_parts.push(text.clone());
                        blocks.push(HistoryBlock::Text { text: text.clone() });
                    }
                    ContentBlock::Thinking { thinking, .. } => {
                        blocks.push(HistoryBlock::Thinking {
                            text: thinking.clone(),
                        });
                    }
                    ContentBlock::ToolCall {
                        id,
                        name,
                        arguments,
                        ..
                    } => {
                        let (result, is_error) = tool_results.get(id).cloned().unwrap_or_default();
                        blocks.push(HistoryBlock::ToolUse {
                            id: id.clone(),
                            name: name.clone(),
                            input: arguments.clone(),
                            result: (!result.is_empty()).then_some(result),
                            is_error,
                        });
                    }
                    ContentBlock::Image { data, mime_type } => {
                        blocks.push(HistoryBlock::Image {
                            data: data.clone(),
                            mime_type: mime_type.clone(),
                        });
                    }
                }
            }

            let content = text_parts.join("\n");
            if content.is_empty() && blocks.is_empty() {
                return None;
            }

            Some(SessionMessage {
                role: role.to_string(),
                content,
                created_at: timestamp.to_string(),
                blocks,
            })
        })
        .collect()
}

/// GET /api/ai/sessions/:id/messages — read messages from a session JSONL file
///
/// Reads history directly from disk (no pool/runtime side effects). Returns
/// an empty list if the session file does not exist.
pub async fn get_session_messages(
    axum::extract::Path(id): axum::extract::Path<String>,
    State(_state): State<Arc<AppState>>,
) -> Result<Json<Vec<SessionMessage>>, StatusCode> {
    // File parsing is synchronous; offload it so we don't stall the executor.
    // On panic (e.g. a corrupt session file that fails `SessionManager::open`),
    // surface it in the logs rather than silently swallowing it.
    match tokio::task::spawn_blocking(move || read_messages_from_disk(&id)).await {
        Ok(messages) => Ok(Json(messages_to_session_messages(&messages))),
        Err(join_err) => {
            openatrading_core::log::error!("get_session_messages: read task failed: {join_err}");
            Ok(Json(Vec::new()))
        }
    }
}
/// DELETE /api/ai/sessions/:id — delete a saved session
pub async fn delete_session(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> StatusCode {
    match crate::handlers::delete_session_data(&state, &id).await {
        Ok(_) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::NOT_FOUND,
    }
}

#[derive(Deserialize)]
pub struct RenameSessionRequest {
    pub name: String,
}

/// PATCH /api/ai/sessions/:id — rename a session
///
/// Updates the session name in the JSONL file header. Returns 204 on success,
/// 404 if the session file does not exist, 500 on I/O error.
pub async fn rename_session(
    axum::extract::Path(id): axum::extract::Path<String>,
    axum::Json(req): axum::Json<RenameSessionRequest>,
) -> StatusCode {
    let Some(path) = find_session_file(&id) else {
        return StatusCode::NOT_FOUND;
    };
    if !path.exists() {
        return StatusCode::NOT_FOUND;
    }

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            openatrading_core::log::error!("rename_session: read error: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    let mut lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return StatusCode::NOT_FOUND;
    }

    // Parse the header line (first line) and update the name field.
    let header: serde_json::Value = match serde_json::from_str(lines[0]) {
        Ok(v) => v,
        Err(e) => {
            openatrading_core::log::error!("rename_session: parse header error: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    let mut header = match header.as_object() {
        Some(obj) => obj.clone(),
        None => {
            openatrading_core::log::error!("rename_session: header is not an object");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    header.insert("name".to_string(), serde_json::Value::String(req.name));

    let new_header = match serde_json::to_string(&header) {
        Ok(s) => s,
        Err(e) => {
            openatrading_core::log::error!("rename_session: serialize error: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    lines[0] = &new_header;

    match std::fs::write(&path, lines.join("\n")) {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(e) => {
            openatrading_core::log::error!("rename_session: write error: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

/// GET /api/ai/sessions/:id/tool-results/:tool_call_id — fetch full tool result content (§15)
///
/// Returns the full tool result text for a given tool call. This is used by the
/// frontend to expand truncated tool results that were sent as previews in the
/// event stream.
///
/// # Errors
/// Returns 404 if the session file or tool result is not found.
pub async fn get_tool_result(
    axum::extract::Path((session_id, tool_call_id)): axum::extract::Path<(String, String)>,
) -> Result<axum::Json<serde_json::Value>, axum::http::StatusCode> {
    get_tool_result_data(&session_id, &tool_call_id)
        .map(axum::Json)
        .map_err(|_| axum::http::StatusCode::NOT_FOUND)
}

/// Data-layer variant shared by the HTTP handler and the Tauri IPC dispatch.
pub fn get_tool_result_data(
    session_id: &str,
    tool_call_id: &str,
) -> Result<serde_json::Value, String> {
    let Some(path) = find_session_file(session_id) else {
        return Err("session not found".to_string());
    };

    let Ok(content) = std::fs::read_to_string(&path) else {
        return Err("session not found".to_string());
    };

    // Scan the JSONL file for the matching tool result message
    for line in content.lines().skip(1) {
        if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
            if entry.get("type").and_then(|v| v.as_str()) != Some("message") {
                continue;
            }
            if let Some(msg) = entry.get("message") {
                if msg.get("role").and_then(|v| v.as_str()) != Some("toolResult") {
                    continue;
                }
                if msg.get("tool_call_id").and_then(|v| v.as_str()) != Some(tool_call_id) {
                    continue;
                }
                // Found the matching tool result — extract the text content
                let text: String = msg
                    .get("content")
                    .and_then(|v| v.as_array())
                    .map(|blocks| {
                        blocks
                            .iter()
                            .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .unwrap_or_default();
                return Ok(serde_json::json!({
                    "tool_call_id": tool_call_id,
                    "content": text,
                }));
            }
        }
    }

    Err("tool result not found".to_string())
}

// ── Inner functions for Tauri compatibility ─────────────────────────────

/// Inner version of `get_ai_commands` that returns `Vec<CommandSpec>`.
pub async fn get_ai_commands_inner() -> Vec<CommandSpec> {
    // Only expose the session-level slash commands this GUI actually handles
    // (model/compact/name/session/help). The rest of pi-rs's builtins
    // (quit/login/trust/export/import/share/...) are CLI/TUI-only and have
    // no meaning here. `/skill:` is not a builtin — it's expanded by pi-rs
    // `prompt()`, so we surface loaded skills below as invokable commands.
    let mut specs: Vec<CommandSpec> =
        pi_coding_agent::core::slash_commands::builtin_slash_commands()
            .iter()
            .filter(|cmd| SESSION_SLASH_COMMANDS.contains(&cmd.name.as_str()))
            .map(|cmd| CommandSpec {
                name: cmd.name.clone(),
                description: cmd.description.clone(),
                command_type: CommandType::Slash,
            })
            .collect();

    // Extension commands (e.g. pi-extensions `/goal`). Every session registers
    // the same set via `create_registry()`; trading/MCP extensions currently
    // contribute tools only, so `market_only()` reproduces the list exactly.
    // `resolve_extension_commands` applies the same `:N` conflict resolution
    // pi-rs uses when executing the command inside `prompt()`.
    let mut registry = openatrading_core::llm::create_registry(
        openatrading_core::llm::RegistryOptions::market_only(),
    );
    let resolved = pi_coding_agent::core::slash_commands::resolve_extension_commands(
        &registry.collect_commands(),
    );
    specs.extend(resolved.into_iter().map(|cmd| CommandSpec {
        name: cmd.invocation_name,
        description: cmd.description.unwrap_or_default(),
        command_type: CommandType::Extension,
    }));

    // Skills are invoked via `/skill:<name>`; pi-rs expands them in prompt().
    for skill in openatrading_core::skills::list_skills() {
        specs.push(CommandSpec {
            name: format!("skill:{}", skill.name),
            description: skill.description,
            command_type: CommandType::Skill,
        });
    }

    specs
}

/// Inner version of `list_sessions` that returns `Vec<serde_json::Value>`.
pub async fn list_sessions_inner(state: &Arc<AppState>) -> Vec<serde_json::Value> {
    use std::path::Path;

    let dir = Path::new(&state.ctx.session_dir);
    let infos =
        pi_coding_agent::core::session_manager::list_sessions_concurrent(dir, None, None).await;

    let mut sessions = Vec::with_capacity(infos.len());
    for info in infos {
        let status = if state.ctx.pool.is_stream(&info.id).await {
            "running"
        } else {
            "idle"
        };
        let mut val = serde_json::to_value(&info).unwrap_or(serde_json::Value::Null);
        if let Some(obj) = val.as_object_mut() {
            obj.insert("status".to_string(), serde_json::json!(status));
            obj.remove("all_messages_text");
        }
        sessions.push(val);
    }

    sessions
}

#[cfg(test)]
mod tests {
    use super::{get_ai_commands_inner, messages_to_session_messages, HistoryBlock};
    use openatrading_core::commands::CommandType;

    use openatrading_core::llm::pi_ai_types::ContentBlock;
    use openatrading_core::llm::{pi_ai_types::Usage, AgentMessage};

    fn user_text(text: &str) -> AgentMessage {
        AgentMessage::User {
            content: vec![ContentBlock::text(text)],
            timestamp: 1,
        }
    }

    fn assistant_with_tool_call(id: &str, name: &str) -> AgentMessage {
        AgentMessage::Assistant {
            content: vec![ContentBlock::ToolCall {
                id: id.to_string(),
                name: name.to_string(),
                arguments: serde_json::json!({}),
                thought_signature: None,
                namespace: None,
            }],
            api: "openai".into(),
            provider: "openrouter".into(),
            model: "m".into(),
            usage: Usage::default(),
            stop_reason: None,
            error_message: None,
            timestamp: 2,
        }
    }

    fn tool_result(id: &str, text: &str, is_error: bool) -> AgentMessage {
        AgentMessage::ToolResult {
            tool_call_id: id.to_string(),
            tool_name: "read".into(),
            content: vec![ContentBlock::text(text)],
            details: serde_json::Value::Null,
            is_error,
            usage: None,
            timestamp: 3,
        }
    }

    /// Happy path: a tool_result is joined onto the matching ToolUse block,
    /// and ToolResult is not emitted as a standalone history entry.
    #[test]
    fn tool_result_joins_onto_tool_use() {
        let messages = vec![
            user_text("run it"),
            assistant_with_tool_call("t1", "read"),
            tool_result("t1", "file contents", false),
        ];
        let out = messages_to_session_messages(&messages);
        assert_eq!(out.len(), 2, "user + assistant only; tool_result joined");
        let assistant = &out[1];
        assert_eq!(assistant.blocks.len(), 1);
        match &assistant.blocks[0] {
            HistoryBlock::ToolUse {
                result, is_error, ..
            } => {
                assert_eq!(result.as_deref(), Some("file contents"));
                assert!(!*is_error);
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
    }

    /// Failure path: an errored tool_result surfaces `is_error=true`.
    #[test]
    fn errored_tool_result_marks_error() {
        let messages = vec![
            assistant_with_tool_call("t2", "read"),
            tool_result("t2", "boom", true),
        ];
        let out = messages_to_session_messages(&messages);
        let block = &out[0].blocks[0];
        match block {
            HistoryBlock::ToolUse {
                is_error, result, ..
            } => {
                assert!(*is_error);
                assert_eq!(result.as_deref(), Some("boom"));
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
    }

    /// Edge case: a ToolUse whose result is missing (in-flight / not yet
    /// recorded) still renders, with no result and `is_error=false`.
    #[test]
    fn tool_use_without_result_renders_empty() {
        let messages = vec![assistant_with_tool_call("t3", "read")];
        let out = messages_to_session_messages(&messages);
        match &out[0].blocks[0] {
            HistoryBlock::ToolUse {
                result, is_error, ..
            } => {
                assert!(result.is_none());
                assert!(!*is_error);
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
    }

    /// Happy path: a user message carrying an image attachment surfaces an
    /// `Image` history block so the frontend can re-render it after reload.
    #[test]
    fn user_image_block_surfaces_in_history() {
        let messages = vec![AgentMessage::User {
            content: vec![
                ContentBlock::text("look at this chart"),
                ContentBlock::Image {
                    data: "aGVsbG8=".to_string(),
                    mime_type: "image/png".to_string(),
                },
            ],
            timestamp: 1,
        }];
        let out = messages_to_session_messages(&messages);
        assert_eq!(out.len(), 1);
        let user = &out[0];
        assert_eq!(user.content, "look at this chart");
        match &user.blocks[1] {
            HistoryBlock::Image { data, mime_type } => {
                assert_eq!(data, "aGVsbG8=");
                assert_eq!(mime_type, "image/png");
            }
            other => panic!("expected Image block, got {other:?}"),
        }
    }

    /// Happy path: the slash-command list exposes builtin session commands,
    /// extension commands (e.g. pi-extensions `/goal`), and skills.
    #[tokio::test]
    async fn ai_commands_include_extension_commands() {
        let specs = get_ai_commands_inner().await;

        // Builtin session-level commands keep `Slash` type. Note `help`/`?`
        // are handled locally by the session actor and are not part of
        // pi-rs's `builtin_slash_commands()` list, so they are not exposed.
        for name in ["model", "name", "compact", "session"] {
            assert!(
                specs
                    .iter()
                    .any(|c| { c.name == name && c.command_type == CommandType::Slash }),
                "missing builtin slash command /{name}: {specs:?}"
            );
        }

        // Extension commands (pi-extensions `/goal`) surface with `Extension`.
        let goal = specs
            .iter()
            .find(|c| c.name == "goal")
            .unwrap_or_else(|| panic!("missing extension command /goal: {specs:?}"));
        assert_eq!(goal.command_type, CommandType::Extension);
        assert!(!goal.description.is_empty());
    }
}
