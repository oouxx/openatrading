// clippy fixes for tauri
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use openatrading_core::api::xuangubao::{KlinePeriod, XuanguBaoClient};

use openatrading_web::gateway::Gateway;
use openatrading_web::{AppState, RoleEntry};
use std::sync::Arc;
use tauri::Emitter;
use tokio::sync::RwLock;

fn load_env() {
    // Layer 2: 全局 .env
    let global_env = openatrading_core::atrading_home().join(".env");
    if global_env.exists() {
        dotenvy::from_path(&global_env).ok();
        eprintln!("[env] loaded {}", global_env.display());
    }

    // Layer 3: 项目 .env（覆盖全局）
    if let Ok(cwd) = std::env::current_dir() {
        let project_env = cwd.join(".atrading").join(".env");
        if project_env.exists() {
            dotenvy::from_path(&project_env).ok();
            eprintln!("[env] loaded {}", project_env.display());
        }
    }
}
/// serde 默认值：`true`（`[[roles]]` 的 `readonly`）。
const fn default_true() -> bool {
    true
}

fn build_roles() -> Vec<RoleEntry> {
    let mut roles: Vec<RoleEntry> = Vec::new();

    // Load readonly roles from {atrading_home}/config.toml
    let config_path = openatrading_core::atrading_home().join("config.toml");
    if config_path.exists() {
        if let Ok(raw) = std::fs::read_to_string(&config_path) {
            #[derive(serde::Deserialize)]
            struct RolesCfg {
                #[serde(default)]
                roles: Vec<RoleItem>,
            }
            #[derive(serde::Deserialize)]
            struct RoleItem {
                name: String,
                #[serde(default)]
                display_name: Option<String>,
                /// 与 `RoleConfig` 保持一致：config.toml 声明的角色默认只读。
                #[serde(default = "default_true")]
                readonly: bool,
                #[serde(default)]
                initial_capital: Option<f64>,
            }
            if let Ok(parsed) = toml::from_str::<RolesCfg>(&raw) {
                for rc in parsed.roles {
                    if !roles.iter().any(|r| r.name == rc.name) {
                        let display_name = rc.display_name.unwrap_or_else(|| rc.name.clone());
                        roles.push(RoleEntry {
                            name: rc.name,
                            display_name,
                            readonly: rc.readonly,
                            initial_capital: rc
                                .initial_capital
                                .unwrap_or(openatrading_core::roles::DEFAULT_INITIAL_CAPITAL),
                        });
                    }
                }
            }
        }
    }

    // Discover editable roles from disk
    roles.extend(openatrading_core::roles::list_roles_from_disk());
    if roles.is_empty() {
        eprintln!("[tauri] 警告: 未发现任何角色，可在角色管理或 config.toml [[roles]] 中添加");
    }
    roles
}

/// 桌面端的统一 HTTP 入口。
///
/// 直接把请求交给 `openatrading_web::build_router` 生成的同一个 Axum Router
/// 处理（进程内 `oneshot`，不经过网络），因此 Web 与 Tauri 共用同一份路由表、
/// 参数解析和错误响应，不会再出现「Web 加了接口、桌面端忘记加 dispatch」。
#[tauri::command]
async fn api_request(
    method: String,
    path: String,
    body: Option<serde_json::Value>,
    router: tauri::State<'_, axum::Router>,
) -> Result<serde_json::Value, String> {
    use tower::ServiceExt;

    let method = axum::http::Method::from_bytes(method.as_bytes())
        .map_err(|e| format!("invalid method {method}: {e}"))?;
    let mut builder = axum::http::Request::builder().method(method).uri(&path);
    let body = match body {
        Some(value) => {
            builder = builder.header(axum::http::header::CONTENT_TYPE, "application/json");
            axum::body::Body::from(
                serde_json::to_vec(&value).map_err(|e| format!("serialize body: {e}"))?,
            )
        }
        None => axum::body::Body::empty(),
    };
    let request = builder
        .body(body)
        .map_err(|e| format!("invalid request {path}: {e}"))?;

    let response = router
        .inner()
        .clone()
        .oneshot(request)
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .map_err(|e| format!("read response: {e}"))?;

    if !status.is_success() {
        let message = serde_json::from_slice::<serde_json::Value>(&bytes)
            .ok()
            .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
            .unwrap_or_else(|| status.to_string());
        return Err(message);
    }
    if bytes.is_empty() {
        return Ok(serde_json::Value::Null);
    }
    serde_json::from_slice(&bytes).map_err(|e| format!("invalid JSON response: {e}"))
}

/// An image attachment sent via Tauri IPC (mirrors the frontend `PromptImage`).
#[derive(Debug, serde::Deserialize)]
struct SendPromptImage {
    data: String,
    #[serde(rename = "mimeType")]
    mime_type: String,
}

#[tauri::command]
async fn send_prompt(
    session_id: String,
    text: String,
    #[allow(unused)] images: Option<Vec<SendPromptImage>>,
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), String> {
    // Ensure the session exists before handling any command.
    // This creates the session if needed and sets up the event forward task.
    ensure_session_tauri(&session_id, &state, &app).await?;

    // The event_state assigns seq + retains the event in the ring (kept
    // consistent with the WebSocket path). Desktop doesn't reconnect so dedup
    // isn't needed, but routing through emit keeps a single serialization path.
    let event_state = state.ctx.pool.event_state(&session_id).await;

    // Session-level slash commands (model/compact/name/session/help) are
    // executed by the actor; `/skill:` and unknown commands fall through to
    // pi-rs, which expands `/skill:name` itself.
    if let Some((cmd_name, cmd_args)) = openatrading_web::chat::parse_session_slash_command(&text) {
        let notice = openatrading_web::chat::run_session_slash_command(
            &state.ctx.pool,
            &session_id,
            &cmd_name,
            &cmd_args,
        )
        .await;
        let json = event_state.emit(openatrading_core::llm::TranscriptEvent::Session(notice));
        let event_value: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
        let payload = serde_json::json!({ "session_id": session_id, "event": event_value });
        let _ = app.emit("agent-event", payload);
        return Ok(());
    }

    // Normal prompt — forward to the agent. Events flow via the background
    // forward task set up by ensure_session_tauri.
    //
    // Default to steering: a prompt submitted while a turn is active is
    // injected into that run instead of waiting for a separate follow-up turn.
    // The actor checks its own active-turn state, avoiding a racy
    // `is_streaming()` TOCTOU check on our side.
    let image_blocks: Vec<pi_coding_agent::pi_ai_types::ContentBlock> = images
        .unwrap_or_default()
        .iter()
        .map(|img| pi_coding_agent::pi_ai_types::ContentBlock::Image {
            data: img.data.clone(),
            mime_type: img.mime_type.clone(),
        })
        .collect();
    let opts = pi_coding_agent::core::agent_session::PromptOptions {
        streaming_behavior: Some("steer".to_string()),
        images: (!image_blocks.is_empty()).then_some(image_blocks),
        ..Default::default()
    };
    if let Err(e) = state
        .ctx
        .pool
        .prompt_with_opts(&session_id, &text, opts)
        .await
    {
        // Surface the error to the frontend as a Notice event, mirroring the
        // WebSocket path (which emits via the session event channel). The
        // `send_prompt` invoke resolves Ok so the frontend renders the error
        // through the same event stream as everything else. Route through
        // event_state.emit for consistent seq/serialization.
        let json = event_state.emit(openatrading_core::llm::TranscriptEvent::Session(
            openatrading_core::llm::SessionEventPayload::Notice {
                text: e,
                is_error: true,
            },
        ));
        let event_value: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
        let payload = serde_json::json!({ "session_id": session_id, "event": event_value });
        let _ = app.emit("agent-event", payload);
    }
    Ok(())
}

#[tauri::command]
async fn interrupt_session(
    session_id: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), String> {
    // Signal the in-flight prompt via the per-session Notify rather than
    // locking the runtime mutex (held for the whole stream — would deadlock
    // the interrupt). See `SessionPool::prompt_with_opts`.
    state.ctx.pool.signal_abort(&session_id).await;
    Ok(())
}

#[tauri::command]
async fn respond_permission(session_id: String, id: String, approved: bool) -> Result<(), String> {
    eprintln!("permission response received but not yet wired: session={session_id}, id={id}, approved={approved}");
    Ok(())
}

/// Delay before restarting the app so the IPC response reaches the frontend.
const APP_RESTART_DELAY: std::time::Duration = std::time::Duration::from_millis(200);

/// Ensure a session runtime exists in the pool, setting up a background forward
/// task that pumps `TranscriptEvent`s to the frontend via `app.emit`.
///
/// Uses the shared `ensure_session_with_events` logic (same as the WebSocket
/// path) so that session creation, resumption, and event channel setup are
/// consistent between Tauri and Web.
async fn ensure_session_tauri(
    session_id: &str,
    state: &Arc<AppState>,
    app: &tauri::AppHandle,
) -> Result<(), String> {
    // Only set up the event-forward task once per session. A new subscription
    // on every send_prompt would accumulate listeners that never unsubscribe,
    // delivering each event N times to the frontend.
    {
        let mut fwd = state
            .forward_tasks
            .lock()
            .map_err(|_| "internal lock poisoned".to_string())?;
        if fwd.contains_key(session_id) {
            return Ok(());
        }
        fwd.insert(session_id.to_string(), ());
    }

    let session_with_events =
        openatrading_web::chat_session::ensure_session_with_events(&state.ctx.pool, session_id)
            .await
            .map_err(|e| {
                // Roll back the forward-task marker so a later call can retry.
                state
                    .forward_tasks
                    .lock()
                    .ok()
                    .map(|mut m| m.remove(session_id));
                e
            })?;

    let mut rx = session_with_events.rx;
    let app_handle = app.clone();
    let sid = session_id.to_string();
    let cleanup_state = state.clone();

    // rx yields already-serialized EventEnvelope JSON strings (with seq). The
    // actor's fan-out prunes this connection's channel when `rx` is dropped.
    let join_handle = tokio::spawn(async move {
        loop {
            let Some(json) = rx.recv().await else { break };
            let event_value: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
            let payload = serde_json::json!({ "session_id": sid, "event": event_value });
            let _ = app_handle.emit("agent-event", payload);
        }
        // The stream ended (session actor evicted / closed / session switched):
        // clear the forward-task marker so a later send_prompt rebuilds the
        // forward task instead of short-circuiting on the stale marker.
        if let Ok(mut fwd) = cleanup_state.forward_tasks.lock() {
            fwd.remove(&sid);
        }
    });
    tokio::spawn(async move {
        if let Err(e) = join_handle.await {
            eprintln!("[agent-forward] forward task panicked: {e}");
        }
    });
    Ok(())
}

#[tauri::command]
async fn api_realtime_stream(
    symbol: String,
    channel: tauri::ipc::Channel<String>,
    _state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let symbols: Vec<String> = symbol
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let client = openatrading_core::api::xuangubao::XuanguBaoClient::new();

    // One-shot: the frontend re-invokes on its own 3s polling interval. An
    // infinite loop here would outlive the subscription (the JS Channel is
    // held by the pending invoke), leaking a permanent upstream poller per
    // subscribe call.
    match client.fetch_realtime_batch(&symbols).await {
        Ok(data) => {
            let json = serde_json::to_string(&data).map_err(|e| format!("serialize error: {e}"))?;
            channel
                .send(json)
                .map_err(|e| format!("channel error: {e}"))
        }
        Err(e) => {
            let msg = e.to_string();
            eprintln!("[realtime-stream] {:?}: {msg}", symbols);
            channel
                .send(format!(r#"{{"error":"{msg}"}}"#))
                .map_err(|f| format!("channel error: {f}"))
        }
    }
}

/// 盘口五档一次性拉取。前端每 3 秒重新 invoke 一次（与 `api_kline_stream` 同一
/// 约定：不在这里常驻轮询，否则每次订阅都会漏一个永久上游轮询任务）。
#[tauri::command]
async fn api_order_book_stream(
    symbol: String,
    channel: tauri::ipc::Channel<String>,
) -> Result<(), String> {
    let client = openatrading_core::api::Client::new();
    match client.fetch_order_book(&symbol).await {
        Ok(book) => {
            // 与 Web 端 SSE 的批量形状保持一致：{ "000001": OrderBook }
            let value = serde_json::to_value(&book).map_err(|e| format!("serialize error: {e}"))?;
            let json = serde_json::to_string(&serde_json::json!({ symbol.as_str(): value }))
                .map_err(|e| format!("serialize error: {e}"))?;
            channel
                .send(json)
                .map_err(|e| format!("channel error: {e}"))
        }
        Err(e) => {
            eprintln!("[order-book-stream] {symbol}: {e}");
            channel
                .send(format!(r#"{{"error":"{e}"}}"#))
                .map_err(|f| format!("channel error: {f}"))
        }
    }
}

#[tauri::command]
async fn api_kline_stream(
    symbol: String,
    period: String,
    count: u32,
    channel: tauri::ipc::Channel<String>,
) -> Result<(), String> {
    let kline_period = match period.as_str() {
        "week" => KlinePeriod::Week,
        "month" => KlinePeriod::Month,
        "minute" => KlinePeriod::Minute,
        _ => KlinePeriod::Day,
    };
    let client = XuanguBaoClient::new();
    // One-shot — see api_realtime_stream.
    match client.fetch_kline(&symbol, kline_period, count).await {
        Ok(data) => {
            let json = serde_json::to_string(&data).map_err(|e| format!("serialize error: {e}"))?;
            channel
                .send(json)
                .map_err(|e| format!("channel error: {e}"))
        }
        Err(e) => {
            eprintln!("[kline-stream] {symbol}: {e}");
            channel
                .send(format!(r#"{{"error":"{e}"}}"#))
                .map_err(|f| format!("channel error: {f}"))
        }
    }
}

#[tauri::command]
async fn get_config() -> Result<serde_json::Value, String> {
    let cfg = openatrading_cli::cli::config::AppConfig::load()
        .map_err(|e| format!("加载配置失败: {e}"))?;

    let social = cfg
        .social
        .as_ref()
        .map(|s| {
            let mut m = serde_json::Map::new();
            if let Some(ch) = &s.notify_channel {
                m.insert(
                    "notify_channel".into(),
                    serde_json::Value::String(ch.clone()),
                );
            }
            m.insert(
                "notify_on_collect".into(),
                serde_json::Value::Bool(s.notify_on_collect),
            );
            m.insert(
                "notify_on_news".into(),
                serde_json::Value::Bool(s.notify_on_news),
            );
            if let Some(tg) = &s.telegram {
                m.insert(
                    "telegram_bot_token".into(),
                    tg.bot_token
                        .as_ref()
                        .map(|v| serde_json::Value::String(v.clone()))
                        .unwrap_or(serde_json::Value::Null),
                );
                m.insert(
                    "telegram_chat_id".into(),
                    tg.chat_id
                        .as_ref()
                        .map(|v| serde_json::Value::String(v.clone()))
                        .unwrap_or(serde_json::Value::Null),
                );
            }
            if let Some(dc) = &s.discord {
                m.insert(
                    "discord_bot_token".into(),
                    dc.bot_token
                        .as_ref()
                        .map(|v| serde_json::Value::String(v.clone()))
                        .unwrap_or(serde_json::Value::Null),
                );
                m.insert(
                    "discord_channel_id".into(),
                    dc.channel_id
                        .as_ref()
                        .map(|v| serde_json::Value::String(v.clone()))
                        .unwrap_or(serde_json::Value::Null),
                );
            }
            serde_json::Value::Object(m)
        })
        .unwrap_or(serde_json::Value::Null);

    Ok(serde_json::json!({ "social": social }))
}

#[tauri::command]
async fn update_config(
    app_handle: tauri::AppHandle,
    section: String,
    key: String,
    value: String,
) -> Result<(), String> {
    openatrading_cli::cli::config::AppConfig::set_value(&section, &key, &value)
        .map_err(|e| format!("写入配置失败: {e}"))?;
    let _ = app_handle.emit("config-updated", ());
    Ok(())
}

#[tauri::command]
async fn restart_app(app_handle: tauri::AppHandle) -> Result<(), String> {
    // Spawn a brief delay so the IPC response can reach the frontend
    let h = app_handle.clone();
    tokio::spawn(async move {
        tokio::time::sleep(APP_RESTART_DELAY).await;
        h.restart();
    });
    Ok(())
}

#[allow(clippy::disallowed_methods)]
fn main() {
    load_env();

    // 品牌根即 pi-rs 的家（`{品牌根}/agent`）。必须在任何 pi-rs 调用与多线程 runtime
    // 起来之前绑定：pi-rs 内部有大量路径是直接读全局解析的（bash 工具的 bin、skills、
    // prompt 模板、扩展 runtime handle），绑一次 env 才能与我们的调用点同源。
    openatrading_core::init::bind_pi_rs_home();

    // 首次启动幂等初始化：创建 ~/.atrading + config.toml 模板 + roles/（已存在则跳过）。
    // 失败不阻塞启动——Gateway::init 会以更明确的错误暴露目录不可写等问题。
    if let Err(e) = openatrading_core::init::ensure_initialized() {
        eprintln!("[tauri] 初始化 .atrading 失败: {e}");
    }

    let db_path = openatrading_core::atrading_home().join("data.db");

    let gateway = Gateway::init(&db_path).unwrap_or_else(|e| panic!("failed to init gateway: {e}"));

    let roles = build_roles();

    // 自动化中枢:桌面端提供 store 级 CRUD/收件箱 + agent 工具;调度器在
    // gateway 进程启动,避免 gateway 与桌面端同时运行时重复执行同一任务。
    // 工具创建的任务先落库,由 gateway 的 reconcile(每分钟)感知并加载。
    let automation_hub =
        openatrading_core::automation::AutomationHub::new(gateway.automation.clone());

    // MCP 服务器配置读写入口：文件（$PI_RS_HOME/agent/mcp.json +
    // {cwd}/.pi-rs/mcp.json）是唯一事实源；会话侧由 pi-rs SDK 在装配时读取。
    let mcp_config = openatrading_core::llm::McpConfigStore::from_process()
        .unwrap_or_else(|e| panic!("failed to resolve MCP config paths: {e}"));

    let app_ctx = tokio::runtime::Runtime::new()
        .unwrap_or_else(|e| panic!("failed to create tokio runtime: {e}"))
        .block_on(openatrading_core::context::AppContext::build(
            openatrading_core::context::ContextParams {
                memory: gateway.memory.clone(),
                trade_store: gateway.trade.clone(),
                social: Arc::new(openatrading_core::social::NoopChannel),
                automations: Some(automation_hub.clone()),
            },
        ));

    let app_state = Arc::new(AppState {
        ctx: app_ctx,
        roles: Arc::new(RwLock::new(roles)),
        mcp: mcp_config,
        cache: openatrading_web::cache::new_cache(),
        in_flight: openatrading_web::cache::InFlightRequests::new(),
        market_hub: openatrading_web::market_stream::MarketStreamHub::new(),
        forward_tasks: std::sync::Mutex::new(std::collections::HashMap::new()),
        automation: automation_hub,
    });

    // Web 与桌面端共用同一份 Axum Router：桌面端 `api_request` 进程内 oneshot
    // 到这个 Router，新增/修改接口只需改 `crates/web/src/lib.rs` 一处。
    let api_router = openatrading_web::build_router(app_state.clone());

    tauri::Builder::default()
        .manage(app_state)
        .manage(api_router)
        .invoke_handler(tauri::generate_handler![
            api_request,
            send_prompt,
            interrupt_session,
            respond_permission,
            api_realtime_stream,
            api_kline_stream,
            api_order_book_stream,
            get_config,
            update_config,
            restart_app
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| panic!("error while running tauri application: {e}"));
}
