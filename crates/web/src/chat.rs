use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{sse::Event, IntoResponse, Sse},
    Json,
};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use openatrading_core::api::Client as ApiClient;
use openatrading_core::commands::{CommandSpec, CommandType};
use openatrading_core::llm::{assistant_event_to_stream_events, StreamEvent};
use openatrading_core::skills::SkillEntry;
use openatrading_core::skills::SkillRegistry;
use openatrading_core::skills::SkillRequest;

use crate::handlers::OverviewResponse;
use crate::AppState;

/// Extract the human-readable text from a serialized `AgentToolResult`.
///
/// The `ToolExecutionEnd` event carries `result` as the full
/// `AgentToolResult` JSON (`{ content: [...], details, terminate }`).
/// The frontend tool-call block expects a plain string, so we concatenate
/// the `text` of every `content` block. Falls back to the raw JSON string
/// when the shape is unexpected.
fn extract_tool_result_text(result: &serde_json::Value) -> String {
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

#[derive(Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Deserialize)]
pub struct ChatRequest {
    message: String,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    history: Vec<ChatMessage>,
    #[serde(default)]
    command_name: Option<String>,
    #[serde(default)]
    command_type: Option<openatrading_core::commands::CommandType>,
    #[serde(default)]
    skill_name: Option<String>,
}

#[derive(Serialize)]
pub struct AiRoleItem {
    pub name: String,
    pub display_name: String,
}

pub async fn get_ai_commands(State(state): State<Arc<AppState>>) -> Json<Vec<CommandSpec>> {
    // Use the pre-built tool registry from AppContext (startup lifetime)
    let registry = &state.ctx.tool_registry;

    let all_tools = registry.load().all_tools();
    let mcp_count = registry.load().mcp_tools().len();
    openatrading_core::log::info!(
        "get_ai_commands: {} total tools ({} market, {} mcp, {} meta/skill/decision)",
        all_tools.len(),
        registry.load().market_tools().len(),
        mcp_count,
        all_tools.len() - registry.load().market_tools().len() - mcp_count,
    );

    // All tools (market + meta skill + MCP + decision)
    let mut commands: Vec<CommandSpec> = all_tools
        .iter()
        .map(|t| CommandSpec {
            name: t.name.clone(),
            description: t.description.clone(),
            command_type: CommandType::Tool,
        })
        .collect();

    // Skill commands from SkillRegistry
    let skills = openatrading_core::skills::SkillRegistry::from_disk();
    for s in skills.skills() {
        commands.push(CommandSpec {
            name: s.name.clone(),
            description: s.description.clone(),
            command_type: CommandType::Skill,
        });
    }

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
        },
    }
}

pub async fn get_ai_roles(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    let roles_guard = state.roles.read().await;
    let roles: Vec<AiRoleItem> = roles_guard
        .iter()
        .map(|r| AiRoleItem {
            name: r.name.clone(),
            display_name: r.display_name.clone(),
        })
        .collect();
    Ok(Json(roles))
}

pub fn build_system_prompt(strategy_text: &str) -> String {
    openatrading_core::skills::SkillRegistry::build_chat_prompt(strategy_text)
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
        &overview.date,
        overview.rise_count,
        overview.fall_count,
        overview.limit_up_count,
        overview.limit_down_count,
        overview.bomb_rate,
        &overview.risk_level,
        &overview.decision_summary,
        &field_section,
        &super_section,
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

pub async fn fetch_overview_data(api: &ApiClient) -> anyhow::Result<OverviewResponse> {
    let overview = api.xuangubao.fetch_market_overview().await?;

    let ratio = overview.rise_count as f64 / overview.fall_count.max(1) as f64;
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

pub async fn ai_chat(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ChatRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>>, StatusCode> {
    use openatrading_core::llm::pi_ai_types::ContentBlock;
    use openatrading_core::llm::{AgentEvent, AgentMessage};

    // Use the pre-built tool registry from AppContext (startup lifetime)
    let tools_registry = &state.ctx.tool_registry;

    // ── Command-aware configuration ──
    let role_name = req.role.as_deref().unwrap_or("");
    let roles_guard = state.roles.read().await;
    let default_strategy = openatrading_core::prompts::DEFAULT_CHAT_SYSTEM_PROMPT;

    // skill_name support: route to Skill command if skill_name present
    let effective_command_type = req.command_type.clone().or_else(|| {
        req.skill_name
            .as_ref()
            .map(|_| openatrading_core::commands::CommandType::Skill)
    });
    let effective_command_name = req.command_name.clone().or_else(|| req.skill_name.clone());

    let (system_prompt, tools) = match effective_command_type {
        Some(CommandType::Tool) => {
            let strategy_text: &str = roles_guard
                .iter()
                .find(|r| r.name == role_name)
                .map(|r| r.strategy.as_str())
                .unwrap_or(default_strategy);
            let sp = build_system_prompt(strategy_text);
            let tool_name = req.command_name.as_deref().unwrap_or("");
            if tool_name.is_empty() {
                openatrading_core::log::warn!("ai_chat: Tool command with empty command_name");
            }
            let filtered: Vec<Arc<openatrading_core::llm::DynTool>> = tools_registry
                .load()
                .all_tools()
                .iter()
                .filter(|t| t.name == tool_name)
                .cloned()
                .collect();
            (sp, filtered)
        },
        Some(CommandType::Skill) => {
            let skill_name = effective_command_name.as_deref().unwrap_or("");
            let skills = openatrading_core::skills::SkillRegistry::from_disk();
            let skill = skills.skills().iter().find(|s| s.name == skill_name);
            let prompt = skill
                .map(|s| s.content.as_str())
                .unwrap_or(default_strategy);
            let allowed = skill
                .and_then(|s| s.allowed_tools.as_ref())
                .map(|s| s.split_whitespace().map(String::from).collect::<Vec<_>>())
                .filter(|v| !v.is_empty());
            let filtered: Vec<Arc<openatrading_core::llm::DynTool>> = match allowed {
                Some(names) => tools_registry
                    .load()
                    .all_tools()
                    .iter()
                    .filter(|t| names.contains(&t.name))
                    .cloned()
                    .collect(),
                None => tools_registry.load().all_tools().to_vec(),
            };
            (build_system_prompt(prompt), filtered)
        },
        None => {
            let strategy_text = roles_guard
                .iter()
                .find(|r| r.name == role_name)
                .map(|r| r.strategy.as_str())
                .unwrap_or(default_strategy);
            (
                build_system_prompt(strategy_text),
                tools_registry.load().all_tools().to_vec(),
            )
        },
    };

    let mut pi_messages: Vec<AgentMessage> = Vec::new();
    pi_messages.push(AgentMessage::User {
        content: vec![ContentBlock::Text {
            text: req.message,
            text_signature: None,
        }],
        timestamp: 0,
    });

    // ── 通过 pi-coding-agent 执行 agent loop ──
    let (tx, mut rx) = tokio::sync::mpsc::channel::<AgentEvent>(256);

    let sid = req.session_id.clone();
    let (sid_tx, sid_rx) = tokio::sync::oneshot::channel::<anyhow::Result<String>>();
    tokio::spawn(async move {
        let result =
            openatrading_core::llm::process_messages(&system_prompt, tools, pi_messages, tx, sid)
                .await;
        let _ = sid_tx.send(result.map(|(_, id)| id));
    });
    openatrading_core::log::info!("ai/chat: SSE stream starting");
    let sse_stream = async_stream::stream! {
        // Emit session_id first so the frontend can store it
        match sid_rx.await {
            Ok(Ok(id)) => {
                let data = serde_json::to_string(&serde_json::json!({"session_id": id})).unwrap_or_default();
                yield Ok(Event::default().event("session").data(data));
            },
            Ok(Err(e)) => {
                let err = format!("agent error: {e}");
                openatrading_core::log::error!("ai/chat: {err}");
                yield Ok(Event::default().event("error").data(err.replace('\n', "\\n")));
                return;
            },
            Err(_) => return,
        }
        loop {
            match rx.recv().await {
                Some(AgentEvent::MessageUpdate { assistant_message_event, .. }) => {
                    for stream_event in assistant_event_to_stream_events(&assistant_message_event) {
                        match stream_event {
                            StreamEvent::Text(t) => {
                                openatrading_core::log::trace!("ai/chat: Text {} chars", t.len());
                                yield Ok(Event::default().data(t.replace('\n', "\\n")));
                            }
                            StreamEvent::ReasoningText(t) => {
                                yield Ok(Event::default().event("thinking").data(t.replace('\n', "\\n")));
                            }
                            StreamEvent::ToolCallStart(tc) => {
                                let data = serde_json::to_string(&serde_json::json!({
                                    "id": tc.id,
                                    "name": tc.function_name,
                                    "args": tc.arguments,
                                })).unwrap_or_default();
                                yield Ok(Event::default().event("tool-call").data(data));
                            }
                            StreamEvent::ToolCallDelta(tc) => {
                                let data = serde_json::to_string(&serde_json::json!({
                                    "args": tc.arguments,
                                })).unwrap_or_default();
                                yield Ok(Event::default().event("tool-delta").data(data));
                            }
                            StreamEvent::Error(e) => {
                                openatrading_core::log::warn!("ai/chat: assistant error: {e}");
                                yield Ok(Event::default().data(format!("[ERROR] {e}")));
                            }
                        }
                    }
                }
                Some(AgentEvent::MessageEnd { message }) => {
                    openatrading_core::log::info!("ai/chat: MessageEnd received");
                    if let AgentMessage::Assistant { error_message: Some(err), .. } = &message {
                        if !err.is_empty() {
                            openatrading_core::log::warn!("ai/chat: MessageEnd carries error: {err}");
                            yield Ok(Event::default().data(format!("[ERROR] {err}")));
                        }
                    }
                }
                Some(AgentEvent::AgentEnd { .. }) => {
                    openatrading_core::log::info!("ai/chat: AgentEnd, breaking SSE loop");
                    break;
                }
                Some(AgentEvent::ToolExecutionStart { .. }) => {
                    yield Ok(Event::default().event("tool-exec-start"));
                }
                Some(AgentEvent::ToolExecutionEnd { tool_call_id, tool_name, result, is_error }) => {
                    let status = if is_error { "error" } else { "success" };
                    // `result` is a serialized `AgentToolResult`:
                    // { "content": [ { "type": "text", "text": "..." }, ... ], "details": ..., "terminate": ... }
                    // Extract the concatenated text so the frontend receives a plain string
                    // (Claude/ChatGPT-style tool call block), not a nested JSON object.
                    let result_text = extract_tool_result_text(&result);
                    let data = serde_json::to_string(&serde_json::json!({
                        "id": tool_call_id,
                        "name": tool_name,
                        "result": result_text,
                        "status": status,
                    })).unwrap_or_default();
                    yield Ok(Event::default().event("tool-exec-end").data(data));
                }
                Some(_) => {}
                None => {
                    openatrading_core::log::warn!("ai/chat: channel closed unexpectedly");
                    break;
                }
            }
        }
        yield Ok(Event::default().data("[DONE]"));
    };

    Ok(Sse::new(sse_stream))
}

// ── Session listing (pi-coding-agent JSONL sessions) ────────────────────

/// A saved chat session backed by a pi-coding-agent JSONL file.
#[derive(Serialize)]
pub struct SessionItem {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: usize,
}

/// POST /api/ai/sessions — create a new empty session via pi-coding-agent
pub async fn create_session() -> Json<serde_json::Value> {
    let (id, title) =
        openatrading_core::llm::create_session().unwrap_or_else(|_| (String::new(), String::new()));
    Json(serde_json::json!({
        "id": id,
        "title": title,
        "created_at": "",
        "updated_at": "",
        "message_count": 0,
    }))
}

/// GET /api/ai/sessions — list all saved sessions
pub async fn list_sessions(State(_state): State<Arc<AppState>>) -> Json<Vec<SessionItem>> {
    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let session_dir = std::path::Path::new(&cwd).join(".pi-rs/agent/sessions");

    let mut sessions: Vec<SessionItem> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&session_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            // Read first line (session header) for metadata
            let (title, created_at) = if let Ok(content) = std::fs::read_to_string(&path) {
                let first_line = content.lines().next().unwrap_or("");
                if let Ok(header) = serde_json::from_str::<serde_json::Value>(first_line) {
                    let name = header
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&stem)
                        .to_string();
                    let ts = header
                        .get("timestamp")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    (name, ts)
                } else {
                    (stem.clone(), String::new())
                }
            } else {
                (stem.clone(), String::new())
            };
            // Count message entries
            let msg_count = if let Ok(content) = std::fs::read_to_string(&path) {
                content
                    .lines()
                    .filter(|l| l.contains(r#""type":"message""#))
                    .count()
            } else {
                0
            };
            // Use file modified time as updated_at
            let updated_at = std::fs::metadata(&path)
                .and_then(|m| {
                    m.modified().map(|t| {
                        use std::time::SystemTime;
                        let dur = t.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
                        format!("{}", dur.as_secs())
                    })
                })
                .unwrap_or_default();
            sessions.push(SessionItem {
                id: stem,
                title,
                created_at,
                updated_at,
                message_count: msg_count,
            });
        }
    }
    // Most recent first
    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Json(sessions)
}

#[derive(Serialize)]
pub struct SessionMessage {
    pub role: String,
    pub content: String,
    pub created_at: String,
}

/// GET /api/ai/sessions/:id/messages — read messages from a session JSONL file
pub async fn get_session_messages(
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<Vec<SessionMessage>>, StatusCode> {
    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let path = std::path::Path::new(&cwd)
        .join(".pi-rs/agent/sessions")
        .join(format!("{id}.jsonl"));

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Ok(Json(Vec::new())),
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
                if !role.is_empty() {
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
pub async fn delete_session(axum::extract::Path(id): axum::extract::Path<String>) -> StatusCode {
    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let path = std::path::Path::new(&cwd)
        .join(".pi-rs/agent/sessions")
        .join(format!("{id}.jsonl"));
    if path.exists() {
        match std::fs::remove_file(&path) {
            Ok(()) => StatusCode::NO_CONTENT,
            Err(e) => {
                openatrading_core::log::error!("delete_session: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            },
        }
    } else {
        StatusCode::NOT_FOUND
    }
}
