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
use openatrading_core::skills::SkillRegistry;
use openatrading_core::skills::SkillRequest;

use crate::handlers::OverviewResponse;
use crate::AppState;

/// Extract the human-readable text from a serialized `AgentToolResult`.
///
/// NOTE: moved to `chat_session::extract_tool_result_text` as the single
/// source of truth shared with the Tauri backend. This re-export keeps any
/// external callers working.
pub use crate::chat_session::extract_tool_result_text;

#[derive(Serialize)]
pub struct AiRoleItem {
    pub name: String,
    pub display_name: String,
}

pub async fn get_ai_commands() -> Json<Vec<CommandSpec>> {
    // Commands are provided by pi-coding-agent's built-in slash commands.
    // Tools and skills are managed internally by pi-coding-agent at session
    // creation time and are not listed here.
    let commands: Vec<CommandSpec> =
        pi_coding_agent::core::slash_commands::builtin_slash_commands()
            .iter()
            .map(|cmd| CommandSpec {
                name: cmd.name.clone(),
                description: cmd.description.clone(),
                command_type: CommandType::Slash,
            })
            .collect();

    Json(commands)
}

#[derive(Serialize)]
pub struct ToolResponse {
    pub name: String,
    pub description: String,
}

pub async fn get_skills(State(_state): State<Arc<AppState>>) -> Json<Vec<SkillEntry>> {
    let skills = openatrading_core::skills::SkillRegistry::from_disk();
    Json(skills.skills().to_vec())
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
    let entry = req.into_skill_entry();
    if let Err(e) = SkillRegistry::install_skill(&entry) {
        openatrading_core::log::error!("failed to save skill: {e}");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "failed to save skill"})),
        )
            .into_response();
    }
    let skills = SkillRegistry::from_disk();
    let saved = skills
        .skills()
        .iter()
        .find(|s| s.name == entry.name)
        .cloned();
    (StatusCode::OK, Json(saved)).into_response()
}

/// DELETE /api/skills/:name — delete a skill
pub async fn delete_skill(
    State(_state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match SkillRegistry::uninstall_skill(&name) {
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
    let roles: Vec<AiRoleItem> = {
        let roles_guard = state.roles.read().await;
        roles_guard
            .iter()
            .map(|r| AiRoleItem {
                name: r.name.clone(),
                display_name: r.display_name.clone(),
            })
            .collect()
    };
    Ok(Json(roles))
}

#[must_use]
pub fn build_system_prompt(strategy_text: &str) -> String {
    openatrading_core::skills::SkillRegistry::build_chat_prompt(strategy_text)
}

#[must_use]
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
pub async fn handle_slash_command(
    cmd_name: &str,
    cmd_args: &str,
    agent_facade: &mut openatrading_core::llm::runtime::AgentFacade,
) -> Result<String, String> {
    match cmd_name {
        "model" => {
            // Resolve model from "provider/model" format
            let model_registry = agent_facade.session.get_model_registry();
            let result = pi_coding_agent::core::model_resolver::resolve_cli_model(
                None,
                Some(cmd_args),
                model_registry,
            );
            match result.model {
                Some(model) => {
                    let display = format!("{}/{}", model.provider, model.id);
                    agent_facade
                        .session
                        .set_model(model)
                        .await
                        .map(|_| format!("Model set to {display}"))
                        .map_err(|e| format!("Failed to set model: {e}"))
                }
                None => {
                    let err = result.error.unwrap_or_else(|| "Unknown model".to_string());
                    Err(format!("Failed to resolve model: {err}"))
                }
            }
        }
        "name" => {
            if cmd_args.is_empty() {
                return Err("Usage: /name <session name>".to_string());
            }
            agent_facade.session.set_session_name(cmd_args);
            Ok(format!("Session name set to \"{cmd_args}\""))
        }
        "compact" => match agent_facade.session.compact(None).await {
            Ok(result) => Ok(format!("Session compacted: {result}")),
            Err(e) => Err(format!("Compaction failed: {e}")),
        },
        "session" => {
            let sid = agent_facade.get_session_id();
            let name = agent_facade.get_session_name();
            let mgr = agent_facade.session.get_session_manager();
            let msg_count = mgr.get_entries().len();
            let file = mgr
                .get_session_file()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            drop(mgr);
            Ok(format!(
                "Session: {name} ({sid})\nMessages: {msg_count}\nFile: {file}"
            ))
        }
        "help" | "?" => {
            let commands = pi_coding_agent::core::slash_commands::builtin_slash_commands();
            let mut help = "Available slash commands:\n".to_string();
            for cmd in &commands {
                let hint = cmd.argument_hint.as_deref().unwrap_or("");
                help.push_str(&format!(
                    "  /{} {} — {}\n",
                    cmd.name,
                    if hint.is_empty() { "" } else { hint },
                    cmd.description
                ));
            }
            Ok(help)
        }
        _ => Err(format!("Unknown slash command: /{cmd_name}")),
    }
}
// ── Session listing (pi-coding-agent JSONL sessions) ────────────────────

/// Request body for `POST /api/ai/sessions`.
///
/// `name` is the first user message — the session is created lazily after the
/// user sends their first prompt, so the session name reflects that message.
#[derive(Deserialize)]
pub struct CreateSessionRequest {
    #[serde(default)]
    name: Option<String>,
}

/// POST /api/ai/sessions — create a new session via pi-coding-agent.
///
/// Accepts an optional `{ "name": "<first message>" }` body. The name is
/// persisted as the session title so the sidebar can display it without
/// waiting for the first assistant turn.
pub async fn create_session(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateSessionRequest>,
) -> Json<serde_json::Value> {
    let name = req.name.as_deref().filter(|s| !s.is_empty());
    let agent_facade = match state
        .ctx
        .runtime
        .create_session(None, None, Some(true), None, name)
        .await
    {
        Ok(session) => session,
        Err(e) => {
            return Json(serde_json::json!({
                "success": false,
                "error": e.to_string(),
            }));
        }
    };

    // Use session manager to get session metadata
    let mgr = agent_facade.session.get_session_manager();
    let session_file = mgr.get_session_file().map(std::path::Path::to_path_buf);
    let session_name = mgr.get_session_name();
    drop(mgr);

    Json(serde_json::json!({
        "success": true,
        "id": agent_facade.get_session_id(),
        "name": session_name,
        "path": session_file.map(|p| p.to_string_lossy().to_string()),
        "message_count": 0,
    }))
}

/// GET /api/ai/sessions — list all saved sessions
pub async fn list_sessions(State(state): State<Arc<AppState>>) -> Json<Vec<serde_json::Value>> {
    let infos = pi_coding_agent::core::session_manager::SessionManager::list_all(Some(
        &state.ctx.session_dir,
    ))
    .await;

    let running = state
        .running_sessions
        .lock()
        .unwrap()
        .iter()
        .cloned()
        .collect::<std::collections::HashSet<_>>();

    let sessions: Vec<serde_json::Value> = infos
        .into_iter()
        .map(|info| {
            let status = if running.contains(&info.id) {
                "running"
            } else {
                "idle"
            };
            serde_json::json!({
                "id": info.id,
                "name": info.name,
                "created": info.created.to_rfc3339(),
                "modified": info.modified.to_rfc3339(),
                "message_count": info.message_count,
                "status": status,
                "path": info.path.to_string_lossy(),
                "cwd": info.cwd,
                "first_message": info.first_message,
                "parent_session_path": info.parent_session_path,
            })
        })
        .collect();

    Json(sessions)
}

#[derive(Serialize)]
pub struct SessionMessage {
    pub role: String,
    pub content: String,
    pub created_at: String,
}

/// GET /api/ai/sessions/:id/messages — read messages from a session JSONL file
///
/// # Errors
/// Returns an empty list if the session file does not exist or cannot be read.
pub async fn get_session_messages(
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<Vec<SessionMessage>>, StatusCode> {
    let path = pi_coding_agent::config::get_sessions_dir().join(format!("{id}.jsonl"));

    let Ok(content) = std::fs::read_to_string(&path) else {
        return Ok(Json(Vec::new()));
    };

    let mut messages: Vec<SessionMessage> = Vec::new();
    for line in content.lines().skip(1) {
        // skip header line
        if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
            if entry.get("type").and_then(|v| v.as_str()) != Some("message") {
                continue;
            }
            let ts = entry
                .get("timestamp")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if let Some(msg) = entry.get("message") {
                let role = msg
                    .get("role")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let content = msg
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
                // Only return user and assistant messages with non-empty content;
                // filter out toolResult and assistant tool-call-only turns
                if (role == "user" || role == "assistant") && !content.is_empty() {
                    messages.push(SessionMessage {
                        role,
                        content,
                        created_at: ts,
                    });
                }
            }
        }
    }

    Ok(Json(messages))
}

/// DELETE /api/ai/sessions/:id — delete a saved session
pub async fn delete_session(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> StatusCode {
    let path = std::path::Path::new(&state.ctx.session_dir).join(format!("{id}.jsonl"));
    if path.exists() {
        match std::fs::remove_file(&path) {
            Ok(()) => StatusCode::NO_CONTENT,
            Err(e) => {
                openatrading_core::log::error!("delete_session: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    } else {
        StatusCode::NOT_FOUND
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
    let path = pi_coding_agent::config::get_sessions_dir().join(format!("{id}.jsonl"));
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
    let path = pi_coding_agent::config::get_sessions_dir().join(format!("{session_id}.jsonl"));

    let Ok(content) = std::fs::read_to_string(&path) else {
        return Err(axum::http::StatusCode::NOT_FOUND);
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
                if msg.get("tool_call_id").and_then(|v| v.as_str()) != Some(&tool_call_id) {
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
                return Ok(axum::Json(serde_json::json!({
                    "tool_call_id": tool_call_id,
                    "content": text,
                })));
            }
        }
    }

    Err(axum::http::StatusCode::NOT_FOUND)
}
