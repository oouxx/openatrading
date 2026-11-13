// clippy fixes for tauri
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use openatrading_core::api::xuangubao::{KlinePeriod, XuanguBaoClient};
use openatrading_core::utils::attachments::{AttachmentService, FileAttachment, ImageAttachment};

use openatrading_web::gateway::Gateway;
use openatrading_web::{AppState, RoleEntry};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tokio::sync::RwLock;

/// 桌面端窗口行为开关（不参与 Web 宿主，单独管理）。
struct DesktopPrefs {
    /// 关闭窗口时隐藏到托盘而不是退出进程；自动化与 agent 继续在后台运行。
    close_to_tray: AtomicBool,
}

/// 桌面端可选服务（MCP server / Telegram bot）。
///
/// 每个服务持有停止信号：关闭开关时发 `true`，后台任务自行退出。
/// 默认全部关闭，由设置页按需开启。
#[derive(Default)]
struct DesktopServices {
    mcp: tokio::sync::Mutex<Option<tokio::sync::watch::Sender<bool>>>,
    telegram: tokio::sync::Mutex<Option<tokio::sync::watch::Sender<bool>>>,
}

fn load_app_config() -> Result<openatrading_cli::cli::config::AppConfig, String> {
    openatrading_cli::cli::config::AppConfig::load().map_err(|e| format!("加载配置失败: {e}"))
}

/// 启动桌面端 MCP server（复用 `[mcp_server].bind`）。
async fn start_desktop_mcp(
    services: &DesktopServices,
    cfg: &openatrading_cli::cli::config::AppConfig,
) -> Result<(), String> {
    let mut running = services.mcp.lock().await;
    if running.is_some() {
        return Ok(());
    }
    let bind: std::net::SocketAddr = cfg
        .mcp_server
        .bind
        .parse()
        .map_err(|e| format!("MCP 监听地址无效: {e}"))?;
    // 先探测端口：否则绑定失败只会在后台任务的日志里，用户看到的是「开关开了但连不上」。
    std::net::TcpListener::bind(bind).map_err(|e| format!("MCP 端口 {bind} 无法监听: {e}"))?;
    let (tx, rx) = tokio::sync::watch::channel(false);
    let registry = Arc::new(openatrading_core::llm::create_registry());
    tokio::spawn(async move {
        if let Err(e) = openatrading_core::llm::serve_http(registry, bind, rx).await {
            eprintln!("[desktop] MCP server error: {e}");
        }
    });
    *running = Some(tx);
    Ok(())
}

async fn stop_desktop_mcp(services: &DesktopServices) {
    if let Some(tx) = services.mcp.lock().await.take() {
        let _ = tx.send(true);
    }
}

/// 启动桌面端 Telegram 交互 bot（复用 `[social.telegram]` 凭据）。
async fn start_desktop_telegram(
    app_state: &Arc<AppState>,
    services: &DesktopServices,
    cfg: &openatrading_cli::cli::config::AppConfig,
) -> Result<(), String> {
    let mut running = services.telegram.lock().await;
    if running.is_some() {
        return Ok(());
    }
    let tg = cfg
        .social
        .as_ref()
        .and_then(|s| s.telegram.as_ref())
        .filter(|t| t.enabled)
        .ok_or_else(|| "请先在「通知渠道 → Telegram」中启用并填写配置".to_string())?;
    let bot_token = tg
        .bot_token
        .clone()
        .filter(|t| !t.is_empty())
        .ok_or_else(|| "请先填写 Telegram Bot Token".to_string())?;
    let chat_id = tg
        .chat_id
        .clone()
        .filter(|c| !c.is_empty())
        .ok_or_else(|| "请先填写 Telegram Chat ID".to_string())?;

    let roles = app_state.roles.read().await.clone();
    let bot = openatrading_core::social::TelegramBot::new(
        openatrading_core::social::TelegramBotConfig { bot_token, chat_id },
        roles,
        app_state.ctx.clone(),
    );
    let (tx, rx) = tokio::sync::watch::channel(false);
    tokio::spawn(async move {
        if let Err(e) = bot.start_polling_with_shutdown(rx).await {
            eprintln!("[desktop] Telegram bot error: {e}");
        }
    });
    *running = Some(tx);
    Ok(())
}

async fn stop_desktop_telegram(services: &DesktopServices) {
    if let Some(tx) = services.telegram.lock().await.take() {
        let _ = tx.send(true);
    }
}

/// 开启/关闭桌面端 MCP server，并持久化到 `[desktop] mcp_server`。
#[tauri::command]
async fn set_desktop_mcp(
    enabled: bool,
    services: tauri::State<'_, Arc<DesktopServices>>,
) -> Result<(), String> {
    let cfg = load_app_config()?;
    if enabled {
        start_desktop_mcp(&services, &cfg).await?;
        if let Err(e) =
            openatrading_cli::cli::config::AppConfig::set_bool("desktop", "mcp_server", true)
        {
            stop_desktop_mcp(&services).await;
            return Err(format!("保存设置失败: {e}"));
        }
    } else {
        openatrading_cli::cli::config::AppConfig::set_bool("desktop", "mcp_server", false)
            .map_err(|e| format!("保存设置失败: {e}"))?;
        stop_desktop_mcp(&services).await;
    }
    Ok(())
}

/// 开启/关闭桌面端 Telegram bot，并持久化到 `[desktop] telegram_bot`。
#[tauri::command]
async fn set_desktop_telegram(
    enabled: bool,
    state: tauri::State<'_, Arc<AppState>>,
    services: tauri::State<'_, Arc<DesktopServices>>,
) -> Result<(), String> {
    let cfg = load_app_config()?;
    if enabled {
        start_desktop_telegram(&state, &services, &cfg).await?;
        if let Err(e) =
            openatrading_cli::cli::config::AppConfig::set_bool("desktop", "telegram_bot", true)
        {
            stop_desktop_telegram(&services).await;
            return Err(format!("保存设置失败: {e}"));
        }
    } else {
        openatrading_cli::cli::config::AppConfig::set_bool("desktop", "telegram_bot", false)
            .map_err(|e| format!("保存设置失败: {e}"))?;
        stop_desktop_telegram(&services).await;
    }
    Ok(())
}

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

#[tauri::command]
async fn send_prompt(
    session_id: String,
    text: String,
    images: Option<Vec<ImageAttachment>>,
    // Any file (image / PDF / Office document / text), base64 encoded.
    files: Option<Vec<FileAttachment>>,
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
    // Attachments go through the same service as the WebSocket transport:
    // image type detection / resizing and text-file validation must not
    // differ between desktop and web. Invalid attachments reject the invoke
    // so the frontend restores the composer instead of losing the message.
    let images = images.unwrap_or_default();
    let files = files.unwrap_or_default();
    let prepared = tokio::task::spawn_blocking(move || {
        AttachmentService::prepare_prompt(&text, &images, &files)
    })
    .await
    .map_err(|e| {
        eprintln!("[tauri] attachment task failed: {e}");
        "附件处理失败，请重试".to_string()
    })?
    .map_err(|e| e.to_string())?;

    if let Err(e) = state
        .ctx
        .pool
        .prompt_with_opts(&session_id, &prepared.text, prepared.options)
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

    let desktop = cfg.desktop.clone().unwrap_or_default();
    Ok(serde_json::json!({
        "social": social,
        "desktop": {
            "close_to_tray": desktop.close_to_tray,
            "mcp_server": desktop.mcp_server,
            "telegram_bot": desktop.telegram_bot,
            "mcp_bind": cfg.mcp_server.bind,
        },
    }))
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

/// 把主窗口从托盘/最小化状态唤回前台。
fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// 托盘图标：左键唤回主窗口，菜单提供「显示 / 退出」。
///
/// 关闭窗口只隐藏、不退出进程时，没有托盘就再也没有入口把窗口找回来了。
fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let show_item = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    // 由 App 托管，避免句柄被回收后托盘图标消失。
    app.manage(builder.build(app)?);
    Ok(())
}

/// 保存「关闭窗口后继续在后台运行」开关，并立即更新运行中的行为。
#[tauri::command]
fn set_close_to_tray(enabled: bool, prefs: tauri::State<'_, DesktopPrefs>) -> Result<(), String> {
    openatrading_cli::cli::config::AppConfig::set_bool("desktop", "close_to_tray", enabled)
        .map_err(|e| format!("保存设置失败: {e}"))?;
    prefs.close_to_tray.store(enabled, Ordering::Relaxed);
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

    // 桌面端自己也是宿主：attach 后启动调度器（只跑用户自动化，不注册内置采集/新闻）。
    // 与 gateway/轻量进程共用 data.db 时，调度器租约保证同一时刻只有一个宿主执行。
    let app_config = match openatrading_cli::cli::config::AppConfig::load() {
        Ok(cfg) => Some(cfg),
        Err(e) => {
            eprintln!("[tauri] 加载配置失败，使用默认值: {e}");
            None
        }
    };
    let social: Arc<dyn openatrading_core::social::NotificationSender> = app_config
        .as_ref()
        .and_then(|cfg| cfg.social.as_ref())
        .map(openatrading_cli::cli::service_cmd::create_notification_sender)
        .unwrap_or_else(|| Arc::new(openatrading_core::social::NoopChannel));
    let close_to_tray = app_config
        .as_ref()
        .and_then(|cfg| cfg.desktop.as_ref())
        .map_or(true, |d| d.close_to_tray);

    let automation_hub = openatrading_core::automation::AutomationHub::with_owner(
        gateway.automation.clone(),
        "desktop",
    );

    // MCP 服务器配置读写入口：文件（$PI_RS_HOME/agent/mcp.json +
    // {cwd}/.pi-rs/mcp.json）是唯一事实源；会话侧由 pi-rs SDK 在装配时读取。
    let mcp_config = openatrading_core::llm::McpConfigStore::from_process()
        .unwrap_or_else(|e| panic!("failed to resolve MCP config paths: {e}"));

    let app_ctx = Arc::new(
        tokio::runtime::Runtime::new()
            .unwrap_or_else(|e| panic!("failed to create tokio runtime: {e}"))
            .block_on(openatrading_core::context::AppContext::build(
                openatrading_core::context::ContextParams {
                    memory: gateway.memory.clone(),
                    trade_store: gateway.trade.clone(),
                    social,
                    automations: Some(automation_hub.clone()),
                },
            )),
    );
    automation_hub.attach(app_ctx.clone());

    let app_state = Arc::new(AppState {
        ctx: (*app_ctx).clone(),
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

    let app = tauri::Builder::default()
        // 应用内更新：endpoint / 公钥见 tauri.conf.json 的 `plugins.updater`。
        // 更新包签名由 CI 用 `TAURI_SIGNING_PRIVATE_KEY` 生成，客户端只持有公钥。
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(app_state)
        .manage(api_router)
        .manage(DesktopPrefs {
            close_to_tray: AtomicBool::new(close_to_tray),
        })
        .manage(Arc::new(DesktopServices::default()))
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
            set_close_to_tray,
            set_desktop_mcp,
            set_desktop_telegram,
            restart_app
        ])
        .setup(|app| {
            // 桌面端调度器：与 gateway 共用租约，拿不到时自动待命。
            let state = app.state::<Arc<AppState>>();
            let hub = state.automation.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = hub.start(None).await {
                    eprintln!("[automation] scheduler error: {e}");
                }
            });

            // 按配置恢复桌面端可选服务（默认关闭）。
            let services = app.state::<Arc<DesktopServices>>().inner().clone();
            let service_state = app.state::<Arc<AppState>>().inner().clone();
            if let Ok(cfg) = openatrading_cli::cli::config::AppConfig::load() {
                let desktop = cfg.desktop.clone().unwrap_or_default();
                tauri::async_runtime::spawn(async move {
                    if desktop.mcp_server {
                        if let Err(e) = start_desktop_mcp(&services, &cfg).await {
                            eprintln!("[desktop] MCP server 启动失败: {e}");
                        }
                    }
                    if desktop.telegram_bot {
                        if let Err(e) =
                            start_desktop_telegram(&service_state, &services, &cfg).await
                        {
                            eprintln!("[desktop] Telegram bot 启动失败: {e}");
                        }
                    }
                });
            }

            // 托盘创建失败时退化为「关窗即退出」：否则窗口一隐藏就再也找不回来。
            if let Err(e) = build_tray(app) {
                eprintln!("[tauri] 托盘初始化失败，关闭窗口将退出应用: {e}");
                app.state::<DesktopPrefs>()
                    .close_to_tray
                    .store(false, Ordering::Relaxed);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window
                    .state::<DesktopPrefs>()
                    .close_to_tray
                    .load(Ordering::Relaxed)
                {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .build(tauri::generate_context!())
        .unwrap_or_else(|e| panic!("error while running tauri application: {e}"));

    app.run(|app_handle, event| {
        match event {
            // macOS 上窗口隐藏后点击 Dock 图标应重新显示主窗口。
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen { .. } => show_main_window(app_handle),
            tauri::RunEvent::Exit => {
                // 正常退出时立刻释放调度租约，下一个宿主无需等待过期。
                if let Some(state) = app_handle.try_state::<Arc<AppState>>() {
                    state.automation.release_scheduler_lease();
                }
            }
            _ => {}
        }
    });
}
