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
use openatrading_core::skills::SkillEntry;
use openatrading_core::skills::SkillRegistry;
use openatrading_core::skills::SkillRequest;

use crate::chat_session::ChatFrame;
use crate::handlers::OverviewResponse;
use crate::AppState;

/// Extract the human-readable text from a serialized `AgentToolResult`.
///
/// NOTE: moved to `chat_session::extract_tool_result_text` as the single
/// source of truth shared with the Tauri backend. This re-export keeps any
/// external callers working.
pub use crate::chat_session::extract_tool_result_text;

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
        "get_ai_commands: {} total tools ({} mcp, {} extension-provided)",
        all_tools.len(),
        mcp_count,
        all_tools.len() - mcp_count,
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
    use openatrading_core::llm::AgentEvent;

    // Use the pre-built tool registry from AppContext (startup lifetime)
    let tools_registry = &state.ctx.tool_registry;
    let all_tools = tools_registry.load().all_tools();

    // ── Command-aware configuration (shared with the Tauri backend) ──
    let role_name = req.role.as_deref().unwrap_or("");
    let roles_guard = state.roles.read().await;
    let config = crate::chat_session::resolve_chat_config(crate::chat_session::ChatConfigInput {
        role_name,
        command_type: req.command_type.clone(),
        command_name: req.command_name.as_deref(),
        skill_name: req.skill_name.as_deref(),
        roles: &roles_guard,
        all_tools,
    });
    let system_prompt = config.system_prompt;
    drop(roles_guard);

    // ── 通过 pi-coding-agent 执行 agent loop ──
    // 注意：SessionHandle 必须在整个 SSE 流生命周期内保持存活，
    // 因为 listener 注册在 AgentSession 上，handle 一旦 drop，
    // AgentSession 被销毁，listener 中的 tx clone 也会被 drop，
    // 导致 rx.recv() 返回 None，流提前结束。
    let runtime = state.ctx.runtime.clone();
    let system_prompt = system_prompt.clone();
    let user_message = req.message.clone();
    let sid = req.session_id.clone();
    openatrading_core::log::info!("ai/chat: SSE stream starting");
    let sse_stream = async_stream::stream! {
        // 在流内部创建 session，handle 随流生命周期存活
        let (tx, mut rx) = tokio::sync::mpsc::channel::<AgentEvent>(256);
        let mut handle = match runtime
            .create_session(&system_prompt, Vec::new(), true, sid.as_deref())
            .await
        {
            Ok(h) => h,
            Err(e) => {
                let err = format!("agent error: {e}");
                openatrading_core::log::error!("ai/chat: {err}");
                yield Ok(Event::default().event("error").data(err.replace('\n', "\\n")));
                return;
            },
        };
        let id = handle.session_id().to_string();
        handle.subscribe(tx).await;
        handle.send(&user_message).await;

        // Emit session_id first so the frontend can store it
        let data = serde_json::to_string(&serde_json::json!({"session_id": id})).unwrap_or_default();
        yield Ok(Event::default().event("session").data(data));

        loop {
            match rx.recv().await {
                Some(event) => {
                    for frame in crate::chat_session::serialize_chat_event(event) {
                        match frame {
                            ChatFrame::Text(t) => {
                                openatrading_core::log::trace!("ai/chat: Text {} chars", t.len());
                                yield Ok(Event::default().data(t.replace('\n', "\\n")));
                            },
                            ChatFrame::Thinking(t) => {
                                yield Ok(Event::default().event("thinking").data(t.replace('\n', "\\n")));
                            },
                            ChatFrame::ToolCall { id, name, args } => {
                                let data = serde_json::to_string(&serde_json::json!({
                                    "id": id,
                                    "name": name,
                                    "args": args,
                                })).unwrap_or_default();
                                yield Ok(Event::default().event("tool-call").data(data));
                            },
                            ChatFrame::ToolDelta { args } => {
                                let data = serde_json::to_string(&serde_json::json!({
                                    "args": args,
                                })).unwrap_or_default();
                                yield Ok(Event::default().event("tool-delta").data(data));
                            },
                            ChatFrame::Error(e) => {
                                openatrading_core::log::warn!("ai/chat: assistant error: {e}");
                                yield Ok(Event::default().data(format!("[ERROR] {e}")));
                            },
                            ChatFrame::ToolExecStart => {
                                yield Ok(Event::default().event("tool-exec-start"));
                            },
                            ChatFrame::ToolExecutionUpdate { id, name, args, partial_result } => {
                                let data = serde_json::to_string(&serde_json::json!({
                                    "id": id,
                                    "name": name,
                                    "args": args,
                                    "partial_result": partial_result,
                                })).unwrap_or_default();
                                yield Ok(Event::default().event("tool-exec-update").data(data));
                            },
                            ChatFrame::ToolExecEnd { id, name, result, is_error } => {
                                let status = if is_error { "error" } else { "success" };
                                let data = serde_json::to_string(&serde_json::json!({
                                    "id": id,
                                    "name": name,
                                    "result": result,
                                    "status": status,
                                })).unwrap_or_default();
                                yield Ok(Event::default().event("tool-exec-end").data(data));
                            },
                            ChatFrame::TurnStart => {
                                yield Ok(Event::default().event("turn-start"));
                            },
                            ChatFrame::TurnEnd { tool_results } => {
                                let data = serde_json::to_string(&serde_json::json!({
                                    "tool_results": tool_results,
                                })).unwrap_or_default();
                                yield Ok(Event::default().event("turn-end").data(data));
                            },
                            ChatFrame::Done => break,
                        }
                    }
                },
                None => {
                    // handle 存活期内 channel 不应关闭；若关闭说明 agent 内部异常
                    openatrading_core::log::warn!("ai/chat: channel closed unexpectedly (handle still alive)");
                    break;
                },
            }
        }
        // handle 在此处 drop，AgentSession 随之销毁
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
