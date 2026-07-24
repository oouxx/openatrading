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

use crate::handlers::OverviewResponse;
use crate::AppState;

/// Extract the human-readable text from a serialized `AgentToolResult`.
///
/// NOTE: moved to `chat_session::extract_tool_result_text` as the single
/// source of truth shared with the Tauri backend. This re-export keeps any
/// external callers working.
pub use crate::chat_session::extract_tool_result_text;

#[derive(Deserialize)]
pub struct ChatRequest {
    message: String,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    role: Option<String>,
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
    let system_prompt = system_prompt;
    let user_message = req.message.clone();
    let sid = req.session_id.clone();
    openatrading_core::log::info!("ai/chat: SSE stream starting");
    let sse_stream = async_stream::stream! {
        // 在流内部创建 session，handle 随流生命周期存活
        let (tx, mut rx) = tokio::sync::mpsc::channel::<AgentEvent>(256);
        let agent_facade = match runtime
            .create_session(Some(&system_prompt), None, Some(true), sid.as_deref(), None)
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
        let id = agent_facade.get_session_id().clone();
        agent_facade.subscribe(tx).await;
        agent_facade.send(&user_message).await;

        // Emit session_id first so the frontend can store it
        let data = serde_json::to_string(&serde_json::json!({"session_id": id})).unwrap_or_default();
        yield Ok(Event::default().event("session").data(data));

        'outer: loop {
            if let Some(event) = rx.recv().await {
                match event {
                    AgentEvent::MessageUpdate {
                        assistant_message_event,
                        ..
                    } => {
                        use openatrading_core::llm::pi_ai_types::AssistantMessageEvent;
                        match assistant_message_event {
                            AssistantMessageEvent::TextDelta { delta, .. } if !delta.is_empty() => {
                                openatrading_core::log::trace!(
                                    "ai/chat: Text {} chars",
                                    delta.len()
                                );
                                yield Ok(Event::default()
                                    .data(delta.replace('\n', "\\n")));
                            },
                            AssistantMessageEvent::TextEnd { content, .. } if !content.is_empty() => {
                                openatrading_core::log::trace!(
                                    "ai/chat: TextEnd {} chars",
                                    content.len()
                                );
                                yield Ok(Event::default()
                                    .data(content.replace('\n', "\\n")));
                            },
                            AssistantMessageEvent::ThinkingDelta { delta, .. } if !delta.is_empty() => {
                                yield Ok(Event::default()
                                    .event("thinking")
                                    .data(delta.replace('\n', "\\n")));
                            },
                            AssistantMessageEvent::ThinkingEnd { content, .. } if !content.is_empty() => {
                                yield Ok(Event::default()
                                    .event("thinking")
                                    .data(content.replace('\n', "\\n")));
                            },
                            AssistantMessageEvent::ToolCallDelta { delta, .. } if !delta.is_empty() => {
                                let data = serde_json::to_string(
                                    &serde_json::json!({ "args": delta }),
                                )
                                .unwrap_or_default();
                                yield Ok(Event::default()
                                    .event("tool-delta")
                                    .data(data));
                            },
                            AssistantMessageEvent::ToolCallEnd { tool_call, .. } if !tool_call.name.is_empty() => {
                                let data = serde_json::to_string(
                                    &serde_json::json!({
                                        "id": tool_call.id,
                                        "name": tool_call.name,
                                        "args": tool_call.arguments.to_string(),
                                    }),
                                )
                                .unwrap_or_default();
                                yield Ok(Event::default()
                                    .event("tool-call")
                                    .data(data));
                            },
                            AssistantMessageEvent::Done { message, .. } => {
                                for block in &message.content {
                                    if let openatrading_core::llm::pi_ai_types::ContentBlock::Text { text, .. } = block {
                                        if !text.is_empty() {
                                            yield Ok(Event::default()
                                                .data(text.replace('\n', "\\n")));
                                        }
                                    }
                                }
                            },
                            AssistantMessageEvent::Error { error, .. } => {
                                let msg = error.error_message.as_deref().unwrap_or("unknown error");
                                openatrading_core::log::warn!(
                                    "ai/chat: assistant error: {msg}"
                                );
                                yield Ok(Event::default()
                                    .data(format!("[ERROR] {msg}")));
                            },
                            _ => {},
                        }
                    },
                    AgentEvent::MessageEnd { message } => {
                        if let openatrading_core::llm::AgentMessage::Assistant {
                            error_message: Some(err),
                            ..
                        } = &message
                        {
                            if !err.is_empty() {
                                yield Ok(Event::default()
                                    .data(format!("[ERROR] {err}")));
                            }
                        }
                    },
                    AgentEvent::AgentEnd { .. } => break 'outer,
                    AgentEvent::TurnStart => {
                        yield Ok(Event::default().event("turn-start"));
                    },
                    AgentEvent::TurnEnd { tool_results, .. } => {
                        let results: Vec<String> = tool_results
                            .iter()
                            .map(|m| {
                                if let openatrading_core::llm::AgentMessage::ToolResult {
                                    content,
                                    ..
                                } = m
                                {
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
                        let data = serde_json::to_string(&serde_json::json!({
                            "tool_results": results,
                        }))
                        .unwrap_or_default();
                        yield Ok(Event::default().event("turn-end").data(data));
                    },
                    AgentEvent::ToolExecutionStart { .. } => {
                        yield Ok(Event::default().event("tool-exec-start"));
                    },
                    AgentEvent::ToolExecutionUpdate {
                        tool_call_id,
                        tool_name,
                        args,
                        partial_result,
                    } => {
                        let data = serde_json::to_string(&serde_json::json!({
                            "id": tool_call_id,
                            "name": tool_name,
                            "args": args.to_string(),
                            "partial_result": partial_result.to_string(),
                        }))
                        .unwrap_or_default();
                        yield Ok(Event::default()
                            .event("tool-exec-update")
                            .data(data));
                    },
                    AgentEvent::ToolExecutionEnd {
                        tool_call_id,
                        tool_name,
                        result,
                        is_error,
                    } => {
                        let status = if is_error { "error" } else { "success" };
                        let data = serde_json::to_string(&serde_json::json!({
                            "id": tool_call_id,
                            "name": tool_name,
                            "result": crate::chat_session::extract_tool_result_text(
                                &result,
                            ),
                            "status": status,
                        }))
                        .unwrap_or_default();
                        yield Ok(Event::default()
                            .event("tool-exec-end")
                            .data(data));
                    },
                    _ => {},
                }
            } else {
                // handle 存活期内 channel 不应关闭；若关闭说明 agent 内部异常
                openatrading_core::log::warn!("ai/chat: channel closed unexpectedly (handle still alive)");
                break;
            }
        }
        // handle 在此处 drop，AgentSession 随之销毁
        yield Ok(Event::default().data("[DONE]"));
    };

    Ok(Sse::new(sse_stream))
}

// ── Session listing (pi-coding-agent JSONL sessions) ────────────────────

/// POST /api/ai/sessions — create a new empty session via pi-coding-agent
pub async fn create_session(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let agent_facade = match state
        .ctx
        .runtime
        .create_session(None, None, Some(true), None, None)
        .await
    {
        Ok(session) => session,
        Err(e) => {
            return Json(serde_json::json!({
                "success": false,
                "error": e.to_string(),
            }));
        },
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

    let sessions: Vec<serde_json::Value> = infos
        .into_iter()
        .map(|info| {
            serde_json::json!({
                "id": info.id,
                "name": info.name,
                "created": info.created.to_rfc3339(),
                "modified": info.modified.to_rfc3339(),
                "message_count": info.message_count,
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
pub async fn get_session_messages(
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<Vec<SessionMessage>>, StatusCode> {
    let path = pi_coding_agent::config::get_sessions_dir().join(format!("{id}.jsonl"));

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
            },
        }
    } else {
        StatusCode::NOT_FOUND
    }
}
