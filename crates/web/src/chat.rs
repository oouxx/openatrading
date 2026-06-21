use axum::{
    extract::State,
    http::StatusCode,
    response::{sse::Event, IntoResponse, Sse},
    Json,
};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use openatrading_core::api::Client as ApiClient;
use openatrading_core::commands::{CommandSpec, CommandType};

use crate::handlers::OverviewResponse;
use crate::AppState;

#[derive(Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Deserialize)]
pub struct ChatRequest {
    message: String,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    history: Vec<ChatMessage>,
    #[serde(default)]
    command_name: Option<String>,
    #[serde(default)]
    command_type: Option<openatrading_core::commands::CommandType>,
}

#[derive(Serialize)]
pub struct AiRoleItem {
    pub name: String,
    pub display_name: String,
}

pub async fn get_ai_commands(State(state): State<Arc<AppState>>) -> Json<Vec<CommandSpec>> {
    // Build full tool registry including MCP tools
    let tool_cfg = openatrading_core::llm::ToolConfig {
        mcp_servers: state.mcp_servers.clone(),
    };
    let registry = openatrading_core::llm::build_tools(&tool_cfg).await;

    let all_tools = registry.all_tools();
    let mcp_count = registry.mcp_tools().len();
    tracing::info!(
        "get_ai_commands: {} total tools ({} market, {} mcp, {} meta/skill/decision)",
        all_tools.len(),
        registry.market_tools().len(),
        mcp_count,
        all_tools.len() - registry.market_tools().len() - mcp_count,
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

    // Pipeline commands from PipelineRegistry
    let pipelines = openatrading_core::pipelines::PipelineRegistry::from_disk();
    commands.extend(pipelines.commands());

    Json(commands)
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
    use openatrading_core::llm::pi_ai_types::{ContentBlock, StopReason};
    use openatrading_core::llm::{AgentEvent, AgentMessage};

    let tool_cfg = openatrading_core::llm::ToolConfig {
        mcp_servers: state.mcp_servers.clone(),
    };
    let tools_registry = openatrading_core::llm::build_tools(&tool_cfg).await;

    // ── Command-aware configuration ──
    let role_name = req.role.as_deref().unwrap_or("");
    let roles_guard = state.roles.read().await;
    let default_strategy = openatrading_core::prompts::DEFAULT_CHAT_SYSTEM_PROMPT;

    let (system_prompt, tools) = match req.command_type {
        Some(CommandType::Tool) => {
            let strategy_text = roles_guard
                .iter()
                .find(|r| r.name == role_name)
                .map(|r| r.strategy.as_str())
                .unwrap_or(default_strategy);
            let sp = build_system_prompt(strategy_text);
            let tool_name = req.command_name.as_deref().unwrap_or("");
            if tool_name.is_empty() {
                tracing::warn!("ai_chat: Tool command with empty command_name");
            }
            let filtered: Vec<Arc<openatrading_core::llm::DynTool>> = tools_registry
                .all_tools()
                .iter()
                .filter(|t| t.name == tool_name)
                .cloned()
                .collect();
            (sp, filtered)
        },
        Some(CommandType::Skill) => {
            let skill_name = req.command_name.as_deref().unwrap_or("");
            let skills = openatrading_core::skills::SkillRegistry::from_disk();
            let prompt = skills
                .skills()
                .iter()
                .find(|s| s.name == skill_name)
                .map(|s| s.content.as_str())
                .unwrap_or(default_strategy);
            (
                build_system_prompt(prompt),
                tools_registry.all_tools().to_vec(),
            )
        },
        Some(CommandType::Pipeline) => {
            let pipeline_name = req.command_name.as_deref().unwrap_or("");
            let pipelines = openatrading_core::pipelines::PipelineRegistry::from_disk();
            let spec = pipelines.find(pipeline_name);
            let prompt = spec.map(|s| s.prompt.as_str()).unwrap_or(default_strategy);
            let allowed = spec.map(|s| s.tools.clone()).unwrap_or_default();
            let filtered: Vec<Arc<openatrading_core::llm::DynTool>> = tools_registry
                .all_tools()
                .iter()
                .filter(|t| allowed.contains(&t.name) || t.name == "submit_trading_decision")
                .cloned()
                .collect();
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
                tools_registry.all_tools().to_vec(),
            )
        },
    };

    let agent = Arc::new(state.ctx.llm.create_agent(tools, &system_prompt));

    let mut pi_messages: Vec<AgentMessage> = Vec::new();
    for h in req.history {
        match h.role.as_str() {
            "user" => {
                pi_messages.push(AgentMessage::User {
                    content: vec![ContentBlock::Text {
                        text: h.content,
                        text_signature: None,
                    }],
                    timestamp: 0,
                });
            },
            "assistant" => {
                pi_messages.push(AgentMessage::Assistant {
                    content: vec![ContentBlock::Text {
                        text: h.content,
                        text_signature: None,
                    }],
                    api: String::new(),
                    provider: String::new(),
                    model: String::new(),
                    usage: Default::default(),
                    stop_reason: Some(StopReason::Stop),
                    error_message: None,
                    timestamp: 0,
                });
            },
            _ => {},
        }
    }
    pi_messages.push(AgentMessage::User {
        content: vec![ContentBlock::Text {
            text: req.message,
            text_signature: None,
        }],
        timestamp: 0,
    });

    // ── 注册事件监听器，实现渐进式 SSE 推送 ──
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<AgentEvent>();

    let listener = Arc::new(
        move |event: AgentEvent, _signal: Option<tokio::sync::watch::Receiver<bool>>| {
            let tx = tx.clone();
            Box::pin(async move {
                let _ = tx.send(event);
            }) as std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
        },
    );

    let handle = agent.subscribe(listener).await;

    let agent_clone = Arc::clone(&agent);
    tokio::spawn(async move {
        let _ = agent_clone.process(pi_messages).await;
        // Keep handle alive until process completes (Drop replaces listener
        // with no-op, causing the SSE stream to miss events).
        drop(handle);
    });

    tracing::info!("ai/chat: SSE stream starting");
    let sse_stream = async_stream::stream! {
        loop {
            match rx.recv().await {
                Some(AgentEvent::MessageUpdate { assistant_message_event, .. }) => {
                    use openatrading_core::llm::pi_ai_types::AssistantMessageEvent as Ame;
                    match &assistant_message_event {
                        Ame::TextDelta { delta, .. } if !delta.is_empty() => {
                            tracing::trace!("ai/chat: TextDelta {} chars", delta.len());
                            yield Ok(Event::default().data(delta.replace('\n', "\\n")));
                        }
                        Ame::ThinkingDelta { delta, .. } if !delta.is_empty() => {
                            yield Ok(Event::default().event("thinking").data(delta.replace('\n', "\\n")));
                        }
                        Ame::ThinkingDelta { .. } => {}
                        Ame::ToolCallDelta { .. } => {
                            yield Ok(Event::default().event("tool-delta"));
                        }
                        Ame::ToolCallEnd { .. } => {
                            yield Ok(Event::default().event("tool-end"));
                        }
                        Ame::Error { reason, error } => {
                            // Provider stream ended abnormally — e.g. DeepSeek
                            // closing the connection without a `finish_reason`.
                            // Surface the reason + any partial content length so
                            // we can distinguish empty responses from truncated ones.
                            let partial_len = error
                                .content
                                .iter()
                                .map(|b| match b {
                                    openatrading_core::llm::pi_ai_types::ContentBlock::Text { text, .. } => text.len(),
                                    _ => 0,
                                })
                                .sum::<usize>();
                            tracing::warn!(
                                "ai/chat: stream error reason={:?} partial_content_chars={} error_msg={:?}",
                                reason,
                                partial_len,
                                error.error_message,
                            );
                        }
                        _ => {}
                    }
                }
                Some(AgentEvent::MessageEnd { message }) => {
                    tracing::info!("ai/chat: MessageEnd received");
                    if let AgentMessage::Assistant { error_message: Some(err), .. } = &message {
                        if !err.is_empty() {
                            tracing::warn!("ai/chat: MessageEnd carries error: {err}");
                            yield Ok(Event::default().data(format!("[ERROR] {err}")));
                        }
                    }
                }
                Some(AgentEvent::AgentEnd { .. }) => {
                    tracing::info!("ai/chat: AgentEnd, breaking SSE loop");
                    break;
                }
                Some(AgentEvent::ToolExecutionStart { .. }) => {
                    yield Ok(Event::default().event("tool-exec-start"));
                }
                Some(AgentEvent::ToolExecutionEnd { .. }) => {
                    yield Ok(Event::default().event("tool-exec-end"));
                }
                Some(_) => {}
                None => {
                    tracing::warn!("ai/chat: channel closed unexpectedly");
                    break;
                }
            }
        }
        yield Ok(Event::default().data("[DONE]"));
    };

    Ok(Sse::new(sse_stream))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── build_user_message ─────────────────────────────────

    fn make_overview() -> OverviewResponse {
        OverviewResponse {
            date: "2026-06-18 10:00".into(),
            rise_count: 2500,
            fall_count: 1500,
            limit_up_count: 45,
            limit_down_count: 5,
            bomb_rate: 0.25,
            risk_level: "中".into(),
            decision_summary: "风险等级：中；上涨2500家，下跌1500家，涨停45只，跌停5只".into(),
            market_temperature: 62.5,
            yesterday_limit_up_avg_pcp: 3.2,
        }
    }

    #[test]
    fn test_build_user_message_with_all_data() {
        let msg = build_user_message(&make_overview(), Some("板块A\n板块B"), Some("股票X\n股票Y"));
        assert!(msg.contains("韭研公社异动数据"));
        assert!(msg.contains("板块A"));
        assert!(msg.contains("强势股数据"));
        assert!(msg.contains("股票X"));
        assert!(msg.contains("2026-06-18"));
        assert!(msg.contains("2500"));
    }

    #[test]
    fn test_build_user_message_without_optional_sections() {
        let msg = build_user_message(&make_overview(), None, None);
        assert!(!msg.contains("韭研公社异动数据"));
        assert!(!msg.contains("强势股数据"));
        assert!(msg.contains("2026-06-18"));
    }

    #[test]
    fn test_build_user_message_empty_optional_sections() {
        let msg = build_user_message(&make_overview(), Some(""), Some(""));
        // empty strings still trigger section rendering because format! is called
        assert!(msg.contains("韭研公社异动数据"));
        assert!(msg.contains("强势股数据"));
    }

    #[test]
    fn test_build_user_message_zero_counts() {
        let ov = OverviewResponse {
            rise_count: 0,
            fall_count: 0,
            limit_up_count: 0,
            limit_down_count: 0,
            ..make_overview()
        };
        let msg = build_user_message(&ov, None, None);
        assert!(msg.contains("0"));
    }

    // ── build_system_prompt ────────────────────────────────

    #[test]
    fn test_build_system_prompt_returns_non_empty_string() {
        let prompt = build_system_prompt("你是一个股票交易助手。");
        assert!(!prompt.is_empty(), "system prompt should not be empty");
        assert!(
            prompt.contains("股票交易助手"),
            "should include strategy text"
        );
    }

    #[test]
    fn test_build_system_prompt_with_empty_strategy() {
        let prompt = build_system_prompt("");
        assert!(
            !prompt.is_empty(),
            "should produce a fallback prompt even when strategy is empty"
        );
    }

    // ── get_ai_commands (validates JSON structure only) ────

    #[test]
    fn test_command_spec_serde_roundtrip() {
        let cmd = CommandSpec {
            name: "test_cmd".into(),
            description: "A test command".into(),
            command_type: CommandType::Tool,
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let parsed: CommandSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "test_cmd");
        assert_eq!(parsed.command_type, CommandType::Tool);
    }

    #[test]
    fn test_command_spec_serialize_snake_case() {
        let cmd = CommandSpec {
            name: "test".into(),
            description: "desc".into(),
            command_type: CommandType::Skill,
        };
        let json = serde_json::to_value(&cmd).unwrap();
        assert_eq!(
            json["command_type"], "skill",
            "should serialize as snake_case"
        );
    }

    // ── ChatRequest / ChatMessage ──────────────────────────

    #[test]
    fn test_chat_request_deserialize_with_defaults() {
        let json = r#"{"message":"hello"}"#;
        let req: ChatRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.message, "hello");
        assert!(req.role.is_none(), "role should default to None");
        assert!(req.history.is_empty(), "history should default to empty");
        assert!(
            req.command_name.is_none(),
            "command_name should default to None"
        );
    }

    #[test]
    fn test_chat_request_deserialize_full() {
        let json = r#"{
            "message": "hello",
            "role": "test_role",
            "history": [
                {"role": "user", "content": "hi"},
                {"role": "assistant", "content": "hello"}
            ],
            "command_name": "get_market_overview",
            "command_type": "tool"
        }"#;
        let req: ChatRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.message, "hello");
        assert_eq!(req.role.unwrap(), "test_role");
        assert_eq!(req.history.len(), 2);
        assert_eq!(req.command_name.unwrap(), "get_market_overview");
        assert_eq!(req.command_type.unwrap(), CommandType::Tool);
    }

    #[test]
    fn test_chat_message_deserialize() {
        let json = r#"{"role":"user","content":"测试消息"}"#;
        let msg: ChatMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content, "测试消息");
    }
}

// ── Chat Thread CRUD ─────────────────────────────────

#[derive(Serialize)]
pub struct ThreadResponse {
    pub id: i64,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize)]
pub struct MessageResponse {
    pub id: i64,
    pub thread_id: i64,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Deserialize)]
pub struct CreateThreadPayload {
    pub title: Option<String>,
}

#[derive(Deserialize)]
pub struct RenameThreadPayload {
    pub title: String,
}

#[derive(Deserialize)]
pub struct SaveMessagePayload {
    pub role: String,
    pub content: String,
}

/// GET /api/ai/threads — list all threads
pub async fn list_threads(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ThreadResponse>>, StatusCode> {
    let threads = state.chat_threads.list_threads().map_err(|e| {
        tracing::error!("list_threads: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(
        threads
            .into_iter()
            .map(|t| ThreadResponse {
                id: t.id,
                title: t.title,
                created_at: t.created_at,
                updated_at: t.updated_at,
            })
            .collect(),
    ))
}

/// POST /api/ai/threads — create a new thread
pub async fn create_thread(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateThreadPayload>,
) -> Result<Json<ThreadResponse>, StatusCode> {
    let title = payload.title.unwrap_or_else(|| "新对话".to_string());
    let id = state.chat_threads.create_thread(&title).map_err(|e| {
        tracing::error!("create_thread: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let threads = state.chat_threads.list_threads().map_err(|e| {
        tracing::error!("create_thread: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let thread = threads.into_iter().find(|t| t.id == id).ok_or_else(|| {
        tracing::error!("create_thread: thread not found after insert");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(ThreadResponse {
        id: thread.id,
        title: thread.title,
        created_at: thread.created_at,
        updated_at: thread.updated_at,
    }))
}

/// PUT /api/ai/threads/:id — rename a thread
pub async fn rename_thread(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Json(payload): Json<RenameThreadPayload>,
) -> Result<Json<ThreadResponse>, StatusCode> {
    state
        .chat_threads
        .rename_thread(id, &payload.title)
        .map_err(|e| {
            tracing::error!("rename_thread: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let threads = state.chat_threads.list_threads().map_err(|e| {
        tracing::error!("rename_thread: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let thread = threads.into_iter().find(|t| t.id == id).ok_or_else(|| {
        tracing::error!("rename_thread: thread not found");
        StatusCode::NOT_FOUND
    })?;
    Ok(Json(ThreadResponse {
        id: thread.id,
        title: thread.title,
        created_at: thread.created_at,
        updated_at: thread.updated_at,
    }))
}

/// DELETE /api/ai/threads/:id — delete a thread
pub async fn delete_thread(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> StatusCode {
    match state.chat_threads.delete_thread(id) {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(e) => {
            tracing::error!("delete_thread: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        },
    }
}

/// GET /api/ai/threads/:id/messages — get messages for a thread
pub async fn get_thread_messages(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Result<Json<Vec<MessageResponse>>, StatusCode> {
    let messages = state.chat_threads.get_messages(id).map_err(|e| {
        tracing::error!("get_thread_messages: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(
        messages
            .into_iter()
            .map(|m| MessageResponse {
                id: m.id,
                thread_id: m.thread_id,
                role: m.role,
                content: m.content,
                created_at: m.created_at,
            })
            .collect(),
    ))
}

/// POST /api/ai/threads/:id/messages — save a message to a thread
pub async fn save_thread_message(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Json(payload): Json<SaveMessagePayload>,
) -> Result<Json<MessageResponse>, StatusCode> {
    let msg_id = state
        .chat_threads
        .save_message(id, &payload.role, &payload.content)
        .map_err(|e| {
            tracing::error!("save_thread_message: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let messages = state.chat_threads.get_messages(id).map_err(|e| {
        tracing::error!("save_thread_message: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let msg = messages
        .into_iter()
        .find(|m| m.id == msg_id)
        .ok_or_else(|| {
            tracing::error!("save_thread_message: message not found after insert");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(MessageResponse {
        id: msg.id,
        thread_id: msg.thread_id,
        role: msg.role,
        content: msg.content,
        created_at: msg.created_at,
    }))
}
