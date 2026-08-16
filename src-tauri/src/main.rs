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
            }
            if let Ok(parsed) = toml::from_str::<RolesCfg>(&raw) {
                for rc in parsed.roles {
                    if !roles.iter().any(|r| r.name == rc.name) {
                        let display_name = rc.display_name.unwrap_or_else(|| rc.name.clone());
                        roles.push(RoleEntry {
                            name: rc.name,
                            display_name,
                            readonly: true,
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

#[tauri::command]
async fn api_fetch(
    path: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    dispatch_get(&path, &state).await
}

#[tauri::command]
async fn api_post(
    path: String,
    body: serde_json::Value,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    dispatch_post(&path, &body, &state).await
}

#[tauri::command]
async fn api_patch(
    path: String,
    body: serde_json::Value,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    dispatch_patch(&path, &body, &state).await
}

async fn dispatch_get(path: &str, state: &Arc<AppState>) -> Result<serde_json::Value, String> {
    // Delegate to tauri_handlers for all known paths
    match path {
        "/api/overview" => openatrading_web::handlers::get_overview_data(state).await,
        "/api/limit-up" => openatrading_web::handlers::get_limit_up_data(state).await,
        "/api/limit-down" => openatrading_web::handlers::get_limit_down_data(state).await,
        "/api/super-stock" => openatrading_web::handlers::get_super_stock_data(state).await,
        "/api/yesterday-limit-up" => {
            openatrading_web::handlers::get_yesterday_limit_up_data(state).await
        }
        "/api/index-quotes" => openatrading_web::handlers::get_index_quotes_data(state).await,
        "/api/dapan-abnormal" => openatrading_web::handlers::get_dapan_abnormal_data(state).await,
        "/api/timeline" => openatrading_web::handlers::get_timeline_data(state).await,
        "/api/telegraph" => {
            openatrading_web::handlers::get_telegraph_all_data(state, None, None).await
        }
        "/api/ai/roles" => openatrading_web::handlers::get_ai_roles_data(state).await,
        "/api/ai/commands" => openatrading_web::handlers::get_ai_commands_data().await,
        "/api/agent/settings" => openatrading_web::handlers::get_pi_agent_settings_data(),
        "/api/agent/models" => openatrading_web::handlers::get_pi_agent_models_data(),
        "/api/ai/sessions" => openatrading_web::handlers::list_sessions_data(state).await,
        "/api/roles" => openatrading_web::handlers::get_roles_data(state).await,
        "/api/watchlist" => openatrading_web::handlers::get_watchlist_data().await,
        "/api/about" => Ok(openatrading_web::web::get_about().await.0),
        "/api/replay/snapshots" => openatrading_web::handlers::get_replay_snapshots_data().await,
        "/api/replay/index" => openatrading_web::handlers::get_replay_index_data().await,
        "/api/sentiment-history" => {
            openatrading_web::handlers::get_sentiment_history_data(state).await
        }
        "/api/skills" => Ok(
            serde_json::to_value(openatrading_web::handlers::get_skills_data()).unwrap_or_default(),
        ),
        "/api/mcp/servers" => Ok(serde_json::to_value(
            openatrading_web::mcp::list_mcp_servers_data(state).await,
        )
        .unwrap_or_default()),
        _ => dispatch_get_fallback(path, state).await,
    }
}

async fn dispatch_get_fallback(
    path: &str,
    state: &Arc<AppState>,
) -> Result<serde_json::Value, String> {
    // Paths with query parameters or path parameters
    if let Some(rest) = path.strip_prefix("/api/kline/") {
        let parts: Vec<&str> = rest.split('?').collect();
        let symbol = parts[0];
        let period = extract_query(path, "period").unwrap_or_else(|| "day".into());
        let count: u32 = extract_query(path, "count")
            .and_then(|s| s.parse().ok())
            .unwrap_or(256);
        return openatrading_web::handlers::get_kline_data(state, symbol, &period, count).await;
    }
    if let Some(rest) = path.strip_prefix("/api/realtime/") {
        let symbol = rest.split('?').next().unwrap_or("");
        return openatrading_web::handlers::get_realtime_data(state, symbol).await;
    }
    if let Some(rest) = path.strip_prefix("/api/telegraph/") {
        let category = rest.split('?').next().unwrap_or("");
        return openatrading_web::handlers::get_telegraph_data(state, category).await;
    }
    // /api/telegraph?last_time=...&count=20 — infinite-scroll pagination.
    // The exact-match arm in dispatch_get only covers the bare path; with a
    // query string the path no longer equals "/api/telegraph", so it lands
    // here. Without this, the Tauri app could load page 1 but never page 2+.
    if path == "/api/telegraph" || path.starts_with("/api/telegraph?") {
        let last_time = extract_query(path, "last_time").and_then(|s| s.parse::<i64>().ok());
        let count = extract_query(path, "count").and_then(|s| s.parse::<u32>().ok());
        return openatrading_web::handlers::get_telegraph_all_data(state, last_time, count).await;
    }
    if let Some(rest) = path.strip_prefix("/api/market-trend/") {
        let symbol = rest.split('?').next().unwrap_or("");
        return openatrading_web::handlers::get_market_trend_data(state, symbol).await;
    }
    if let Some(rest) = path.strip_prefix("/api/plates-by-any-stock/") {
        let symbol = rest.split('?').next().unwrap_or("");
        return openatrading_web::handlers::get_plates_by_any_stock_data(state, symbol).await;
    }
    if path.strip_prefix("/api/replay/events").is_some() {
        let min_pcp: f64 = extract_query(path, "min_pcp")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.03);
        return openatrading_web::handlers::get_replay_events_data(min_pcp).await;
    }
    if path.strip_prefix("/api/replay/news").is_some() {
        let tailmark = extract_query(path, "tailmark").and_then(|s| s.parse().ok());
        let msg_id_mark = extract_query(path, "msgIdMark");
        return openatrading_web::handlers::get_replay_news_data(tailmark, msg_id_mark).await;
    }
    if path.strip_prefix("/api/search-stocks").is_some() {
        let q = extract_query(path, "q").unwrap_or_default();
        return openatrading_web::handlers::search_stocks_data(&q).await;
    }
    if path.strip_prefix("/api/field-items").is_some() {
        let date = extract_query(path, "date");
        return openatrading_web::handlers::get_field_items_data(state, date, None).await;
    }
    if path.strip_prefix("/api/plate-abnormal").is_some() {
        let date = extract_query(path, "count").unwrap_or_else(|| {
            openatrading_core::api::utils::last_n_trading_days(1)
                .first()
                .cloned()
                .unwrap_or_default()
        });
        return openatrading_web::handlers::get_plate_abnormal_data(state, Some(date)).await;
    }
    if path.strip_prefix("/api/sentiment-history").is_some() {
        return openatrading_web::handlers::get_sentiment_history_data(state).await;
    }
    if path.strip_prefix("/api/dragon-tiger/detail").is_some() {
        let page: usize = extract_query(path, "page")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);
        let page_size: usize = extract_query(path, "page_size")
            .and_then(|s| s.parse().ok())
            .unwrap_or(20);
        return openatrading_web::handlers::get_dragon_tiger_detail_data(state, page, page_size)
            .await;
    }
    if path.strip_prefix("/api/dragon-tiger/stock-seat").is_some() {
        let code = extract_query(path, "code").unwrap_or_default();
        let date = extract_query(path, "date").unwrap_or_default();
        let flag = extract_query(path, "flag").unwrap_or_default();
        return openatrading_web::handlers::get_dragon_tiger_stock_seat_data(
            state, &code, &date, &flag,
        )
        .await;
    }
    if let Some(rest) = path.strip_prefix("/api/ai/sessions/") {
        if let Some(msg_id) = rest.strip_suffix("/messages") {
            return openatrading_web::handlers::get_session_messages_data(state, msg_id).await;
        }
        if let Some((sid, tail)) = rest.split_once("/tool-results/") {
            return openatrading_web::chat::get_tool_result_data(sid, tail);
        }
    }
    if let Some(rest) = path.strip_prefix("/api/mcp/servers/") {
        if let Some(name) = rest.strip_suffix("/toggle") {
            openatrading_web::mcp::toggle_mcp_server_data(state, &url_decode(name)).await?;
            return Ok(serde_json::Value::Null);
        }
    }
    if let Some(rest) = path.strip_prefix("/api/roles/") {
        return dispatch_roles_get(rest, state).await;
    }
    if path.strip_prefix("/api/dragon-tiger/combined").is_some() {
        let date = XuanguBaoClient::last_n_trading_days(1)
            .first()
            .cloned()
            .unwrap_or_default();
        let data = state
            .ctx
            .api
            .eastmoney
            .lhb_combined(&date, &date)
            .await
            .map_err(|e| format!("{e}"))?;
        return Ok(serde_json::to_value(data).unwrap_or_default());
    }
    Err(format!("unknown path: {path}"))
}

async fn dispatch_roles_get(
    rest: &str,
    state: &Arc<AppState>,
) -> Result<serde_json::Value, String> {
    let parts: Vec<&str> = rest.split('/').collect();
    if parts.len() < 2 {
        return Err("invalid roles path".to_string());
    }
    let name = parts[0];
    let action = parts[1].split('?').next().unwrap_or("");
    match action {
        "positions" => openatrading_web::handlers::get_role_positions_data(state, name).await,
        "logs" => openatrading_web::handlers::get_role_logs_data(state, name).await,
        "pnl-history" => {
            let days: usize = extract_query(rest, "days")
                .and_then(|s| s.parse().ok())
                .unwrap_or(30);
            openatrading_web::handlers::get_role_pnl_history_data(state, name, days).await
        }
        "history" => {
            let limit: usize = extract_query(rest, "limit")
                .and_then(|s| s.parse().ok())
                .unwrap_or(100);
            openatrading_web::handlers::get_role_history_data(state, name, limit).await
        }
        _ => Err(format!("unknown roles action: {action}")),
    }
}

async fn dispatch_post(
    path: &str,
    body: &serde_json::Value,
    state: &Arc<AppState>,
) -> Result<serde_json::Value, String> {
    match path {
        "/api/ai/sessions" => {
            let name = body.get("name").and_then(|v| v.as_str());
            let session_id = body.get("session_id").and_then(|v| v.as_str());
            let message = body.get("message").and_then(|v| v.as_str()).unwrap_or("");
            openatrading_web::handlers::create_session_data(state, name, session_id, message).await
        }
        "/api/roles" => {
            let name = body
                .get("name")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| "角色名不能为空".to_string())?
                .to_string();
            let display_name = body
                .get("display_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            openatrading_web::handlers::create_role_data(state, &name, &display_name).await
        }
        "/api/skills" => {
            let req: openatrading_core::skills::SkillRequest = serde_json::from_value(body.clone())
                .map_err(|e| format!("invalid skill payload: {e}"))?;
            serde_json::to_value(openatrading_web::handlers::save_skill_data(req)?)
                .map_err(|e| format!("serialize error: {e}"))
        }
        "/api/mcp/servers" => {
            let config: openatrading_core::llm::McpServerConfig =
                serde_json::from_value(body.clone())
                    .map_err(|e| format!("invalid MCP server payload: {e}"))?;
            openatrading_web::mcp::add_mcp_server_data(state, config).await?;
            Ok(serde_json::Value::Null)
        }
        "/api/agent/settings" => {
            let provider = body.get("defaultProvider").and_then(|v| v.as_str());
            let model = body.get("defaultModel").and_then(|v| v.as_str());
            let thinking_level = body.get("thinkingLevel").and_then(|v| v.as_str());
            openatrading_web::handlers::update_pi_agent_settings_data(
                provider,
                model,
                thinking_level,
            )?;
            Ok(serde_json::Value::Null)
        }
        _ => {
            if let Some(rest) = path.strip_prefix("/api/roles/") {
                let parts: Vec<&str> = rest.split('/').collect();
                let name = parts[0];
                if parts.len() < 2 {
                    // DELETE /api/roles/{name} — no action suffix → delete
                    return openatrading_web::handlers::delete_role_data(state, name).await;
                }
                let action = parts[1];
                match action {
                    "reset" => openatrading_web::handlers::reset_role_data(state, name).await,
                    "delete" => openatrading_web::handlers::delete_role_data(state, name).await,
                    _ => Err(format!("unknown roles action: {action}")),
                }
            } else if let Some(sid) = path.strip_prefix("/api/ai/sessions/") {
                openatrading_web::handlers::delete_session_data(state, sid).await
            } else if let Some(code) = path.strip_prefix("/api/watchlist/") {
                // DELETE /api/watchlist/CODE
                let code = url_decode(code);
                openatrading_web::handlers::delete_watchlist_data(&code).await
            } else if path == "/api/watchlist" {
                // POST /api/watchlist
                let code = body
                    .get("code")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing code".to_string())?
                    .to_string();
                let name = body
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing name".to_string())?
                    .to_string();
                openatrading_web::handlers::add_watchlist_data(&code, &name).await
            } else if let Some(name) = path.strip_prefix("/api/skills/") {
                // DELETE /api/skills/{name}
                openatrading_web::handlers::delete_skill_data(name)?;
                Ok(serde_json::Value::Null)
            } else if let Some(name) = path.strip_prefix("/api/mcp/servers/") {
                // PUT /api/mcp/servers/{name} (edit, has body) or
                // DELETE /api/mcp/servers/{name} (no body) — the frontend
                // routes both through `api_post`.
                let name = url_decode(name);
                if body.as_object().map(|o| !o.is_empty()).unwrap_or(false) {
                    let config: openatrading_core::llm::McpServerConfig =
                        serde_json::from_value(body.clone())
                            .map_err(|e| format!("invalid MCP server payload: {e}"))?;
                    openatrading_web::mcp::update_mcp_server_data(state, &name, config).await?;
                } else {
                    openatrading_web::mcp::delete_mcp_server_data(state, &name).await?;
                }
                Ok(serde_json::Value::Null)
            } else {
                Err(format!("unknown path: {path}"))
            }
        }
    }
}

async fn dispatch_patch(
    path: &str,
    body: &serde_json::Value,
    _state: &Arc<AppState>,
) -> Result<serde_json::Value, String> {
    // PATCH /api/ai/sessions/:id — rename session
    if let Some(sid) = path.strip_prefix("/api/ai/sessions/") {
        let name = body
            .get("name")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "missing name".to_string())?;
        return openatrading_web::handlers::rename_session_data(sid, name).await;
    }
    Err(format!("unknown patch path: {path}"))
}

fn url_decode(s: &str) -> String {
    let s = s.replace('+', " ");
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            } else {
                result.push('%');
                result.push_str(&hex);
            }
        } else {
            result.push(c);
        }
    }
    result
}

fn extract_query(path: &str, key: &str) -> Option<String> {
    let query_part = path.split('?').nth(1)?;
    for pair in query_part.split('&') {
        let mut kv = pair.splitn(2, '=');
        if kv.next() == Some(key) {
            let raw = kv.next()?;
            // Simple percent-decode: %XX -> char
            let decoded = raw.replace('+', " ");
            let mut result = String::with_capacity(decoded.len());
            let mut chars = decoded.chars();
            while let Some(c) = chars.next() {
                if c == '%' {
                    let hex: String = chars.by_ref().take(2).collect();
                    if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                        result.push(byte as char);
                    } else {
                        result.push('%');
                        result.push_str(&hex);
                    }
                } else {
                    result.push(c);
                }
            }
            return Some(result);
        }
    }
    None
}

#[tauri::command]
async fn send_prompt(
    session_id: String,
    text: String,
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
    // pi-rs `prompt()` checks `is_streaming()` internally and uses
    // `streaming_behavior` only to choose steer vs followUp. We pass a fixed
    // policy (followUp = queue as a new turn when streaming) and let pi-rs
    // decide, avoiding a racy `is_stream()` TOCTOU check on our side.
    let opts = pi_coding_agent::core::agent_session::PromptOptions {
        streaming_behavior: Some("followUp".to_string()),
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

    // rx yields already-serialized EventEnvelope JSON strings (with seq). The
    // actor's fan-out prunes this connection's channel when `rx` is dropped.
    let join_handle = tokio::spawn(async move {
        loop {
            let Some(json) = rx.recv().await else { break };
            let event_value: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
            let payload = serde_json::json!({ "session_id": sid, "event": event_value });
            let _ = app_handle.emit("agent-event", payload);
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
                "notify_on_decision".into(),
                serde_json::Value::Bool(s.notify_on_decision),
            );
            m.insert(
                "notify_on_trade".into(),
                serde_json::Value::Bool(s.notify_on_trade),
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
async fn api_search_stocks(
    q: String,
) -> Result<Vec<openatrading_core::api::eastmoney::StockSearchItem>, String> {
    use openatrading_core::api::eastmoney::LhbClient;
    let client = LhbClient::new();
    client
        .search_stocks(&q)
        .await
        .map_err(|e| format!("search failed: {e}"))
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

    // 首次启动幂等初始化：创建 ~/.atrading + config.toml 模板 + roles/（已存在则跳过）。
    // 失败不阻塞启动——Gateway::init 会以更明确的错误暴露目录不可写等问题。
    if let Err(e) = openatrading_core::init::ensure_initialized() {
        eprintln!("[tauri] 初始化 .atrading 失败: {e}");
    }

    let db_path = openatrading_core::atrading_home().join("data.db");

    let gateway = Gateway::init(&db_path).unwrap_or_else(|e| panic!("failed to init gateway: {e}"));

    let roles = build_roles();

    let app_ctx = tokio::runtime::Runtime::new()
        .unwrap_or_else(|e| panic!("failed to create tokio runtime: {e}"))
        .block_on(openatrading_core::context::AppContext::build(
            &[],
            gateway.memory.clone(),
            gateway.trade.clone(),
            Arc::new(openatrading_core::social::NoopChannel),
        ));

    let app_state = Arc::new(AppState {
        ctx: app_ctx,
        roles: Arc::new(RwLock::new(roles)),
        mcp_servers: std::sync::Mutex::new(Vec::new()),
        cache: openatrading_web::cache::new_cache(),
        market_hub: openatrading_web::market_stream::MarketStreamHub::new(),
        forward_tasks: std::sync::Mutex::new(std::collections::HashMap::new()),
    });

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            api_fetch,
            api_post,
            api_patch,
            send_prompt,
            interrupt_session,
            respond_permission,
            api_realtime_stream,
            api_kline_stream,
            api_search_stocks,
            get_config,
            update_config,
            restart_app
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| panic!("error while running tauri application: {e}"));
}
