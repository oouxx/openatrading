#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use trending_agent_core::api::xuangubao::{KlinePeriod, XuanguBaoClient};
use trending_agent_core::api::Client as ApiClient;
use trending_agent_core::gateway::Gateway;
use trending_agent_core::llm::openrouter::LlmClient;
use trending_agent_core::roles::BUILTIN_STRATEGIES;
use trending_agent_core::trade::TradeManager;
use trending_agent_core::web::{AppState, RoleEntry};

fn find_project_root() -> PathBuf {
    // Walk up from current_dir looking for .env first (workspace root),
    // then Cargo.toml as fallback (crate root).
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    // First pass: look for .env (workspace root marker)
    let mut current = cwd.as_path();
    loop {
        if current.join(".env").exists() {
            return current.to_path_buf();
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => break,
        }
    }
    // Second pass: fall back to any Cargo.toml directory
    let mut current = cwd.as_path();
    loop {
        if current.join("Cargo.toml").exists() {
            return current.to_path_buf();
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => break,
        }
    }
    cwd
}

fn load_env(project_root: &PathBuf) {
    // Load from ~/.atrading/.env (global config for secrets, highest priority)
    if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        let home_env = PathBuf::from(home).join(".atrading").join(".env");
        if home_env.exists() {
            dotenvy::from_path(&home_env).ok();
            eprintln!("[env] loaded {}", home_env.display());
        }
    }
    // Load from project root
    let project_env = project_root.join(".env");
    if project_env.exists() {
        dotenvy::from_path(&project_env).ok();
        eprintln!("[env] loaded {}", project_env.display());
    }
    let atrading_env = project_root.join(".atrading").join(".env");
    if atrading_env.exists() && atrading_env != project_env {
        dotenvy::from_path(&atrading_env).ok();
        eprintln!("[env] loaded {}", atrading_env.display());
    }
}

fn build_roles() -> Vec<RoleEntry> {
    BUILTIN_STRATEGIES
        .iter()
        .map(|(name, display_name, strategy)| RoleEntry {
            name: name.to_string(),
            display_name: display_name.to_string(),
            strategy: strategy.to_string(),
        })
        .collect()
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

async fn dispatch_get(path: &str, state: &Arc<AppState>) -> Result<serde_json::Value, String> {
    match path {
        "/api/overview" => {
            let client = XuanguBaoClient::new();
            let overview = client
                .fetch_market_overview()
                .await
                .map_err(|e| format!("{e}"))?;
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
            Ok(serde_json::json!({
                "date": chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
                "rise_count": overview.rise_count,
                "fall_count": overview.fall_count,
                "limit_up_count": overview.limit_up_count,
                "limit_down_count": overview.limit_down_count,
                "bomb_rate": overview.bomb_rate,
                "risk_level": risk_level,
                "decision_summary": decision_summary,
                "market_temperature": overview.market_temperature,
                "yesterday_limit_up_avg_pcp": overview.yesterday_limit_up_avg_pcp,
            }))
        }
        "/api/limit-up" => {
            let client = XuanguBaoClient::new();
            let data = client.fetch_limit_up().await.map_err(|e| format!("{e}"))?;
            Ok(serde_json::to_value(data).unwrap_or_default())
        }
        "/api/limit-down" => {
            let client = XuanguBaoClient::new();
            let data = client
                .fetch_limit_down()
                .await
                .map_err(|e| format!("{e}"))?;
            Ok(serde_json::to_value(data).unwrap_or_default())
        }
        "/api/super-stock" => {
            let client = XuanguBaoClient::new();
            let data = client
                .fetch_super_stock()
                .await
                .map_err(|e| format!("{e}"))?;
            Ok(serde_json::to_value(data).unwrap_or_default())
        }
        "/api/yesterday-limit-up" => {
            let client = XuanguBaoClient::new();
            let data = client
                .fetch_yesterday_limit_up()
                .await
                .map_err(|e| format!("{e}"))?;
            Ok(serde_json::to_value(data).unwrap_or_default())
        }
        "/api/index-quotes" => {
            let client = XuanguBaoClient::new();
            let data = client
                .fetch_market_real(&["000001.SS", "399001.SZ", "399006.SZ"])
                .await
                .map_err(|e| format!("{e}"))?;
            Ok(serde_json::to_value(data).unwrap_or_default())
        }
        "/api/dapan-abnormal" => {
            let data = state
                .api
                .xuangubao
                .fetch_dapan_abnormal()
                .await
                .map_err(|e| format!("{e}"))?;
            Ok(serde_json::to_value(data).unwrap_or_default())
        }
        "/api/timeline" => {
            if std::env::var("JIUYAN_PHONE").is_err() {
                return Ok(serde_json::json!([]));
            }
            let month = chrono::Local::now().format("%Y-%m").to_string();
            let data = state
                .api
                .jiuyan
                .fetch_timeline(&month)
                .await
                .map_err(|e| format!("{e}"))?;
            Ok(serde_json::to_value(data).unwrap_or_default())
        }
        "/api/sentiment" => {
            if std::env::var("JIUYAN_PHONE").is_err() {
                return Err("JIUYAN_PHONE not set".into());
            }
            let data = trending_agent_core::agents::sentiment::analyze_sentiment(&state.api)
                .await
                .map_err(|e| format!("{e}"))?;
            Ok(serde_json::to_value(data).unwrap_or_default())
        }
        "/api/telegraph" => {
            let data = state
                .api
                .cls
                .fetch_telegraph("")
                .await
                .map_err(|e| format!("{e}"))?;
            Ok(serde_json::to_value(data).unwrap_or_default())
        }
        "/api/ai/roles" => {
            let roles_guard = state.roles.read().await;
            let list: Vec<serde_json::Value> = roles_guard
                .iter()
                .map(|r| serde_json::json!({"name": r.name, "display_name": r.display_name}))
                .collect();
            Ok(serde_json::json!(list))
        }
        "/api/roles" => {
            let roles_guard = state.roles.read().await;
            Ok(serde_json::to_value(roles_guard.clone()).unwrap_or_default())
        }
        "/api/replay/events" => {
            let client = XuanguBaoClient::new();
            let has_cursor = extract_query(path, "cursor").is_some();
            let timestamp = extract_query(path, "cursor")
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(XuanguBaoClient::latest_trading_day_close_ts);
            let raw = client
                .fetch_events_page(timestamp, 50)
                .await
                .map_err(|e| format!("{e}"))?;
            let events: Vec<trending_agent_core::api::xuangubao::ReplayEvent> = raw
                .iter()
                .map(trending_agent_core::api::xuangubao::ReplayEvent::from)
                .collect();
            let next_cursor = events.last().map(|e| e.event_timestamp);
            let date = if has_cursor {
                String::new()
            } else {
                XuanguBaoClient::last_n_trading_days(1)
                    .first()
                    .cloned()
                    .unwrap_or_default()
            };
            let market_phase = if has_cursor {
                String::new()
            } else {
                trending_agent_core::web::cache::current_market_phase()
                    .as_str()
                    .to_string()
            };
            let page = trending_agent_core::api::xuangubao::ReplayPage {
                date,
                market_phase,
                events,
                next_cursor,
            };
            Ok(serde_json::to_value(page).unwrap_or_default())
        }
        "/api/replay/index" => {
            use trending_agent_core::api::xuangubao::fetch_index_trends;
            let client = XuanguBaoClient::new();
            let data = fetch_index_trends(&client)
                .await
                .map_err(|e| format!("{e}"))?;
            Ok(serde_json::to_value(data).unwrap_or_default())
        }
        _ => {
            if path.strip_prefix("/api/replay/news").is_some() {
                let client = XuanguBaoClient::new();
                let tailmark = extract_query(path, "tailmark").and_then(|s| s.parse().ok());
                let msg_id_mark = extract_query(path, "msgIdMark");
                let (items, next_tail, next_msg) = client
                    .fetch_news_stream(tailmark, msg_id_mark.as_deref())
                    .await
                    .map_err(|e| format!("{e}"))?;
                return Ok(serde_json::json!({
                    "items": items,
                    "tailmark": next_tail,
                    "msgIdMark": next_msg,
                }));
            }
            if path.strip_prefix("/api/field-items").is_some() {
                let date = extract_query(path, "date")
                    .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());
                if std::env::var("JIUYAN_PHONE").is_err() {
                    return Ok(serde_json::json!([]));
                }
                let data = state
                    .api
                    .jiuyan
                    .fetch_field(&date, 1)
                    .await
                    .map_err(|e| format!("{e}"))?;
                return Ok(serde_json::to_value(data).unwrap_or_default());
            }
            if path.strip_prefix("/api/plate-abnormal").is_some() {
                let count: i32 = extract_query(path, "count")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(30);
                let client = XuanguBaoClient::new();
                let data = client
                    .fetch_plate_abnormal(count)
                    .await
                    .map_err(|e| format!("{e}"))?;
                return Ok(serde_json::to_value(data).unwrap_or_default());
            }
            if path.strip_prefix("/api/sentiment-history").is_some() {
                let client = XuanguBaoClient::new();
                let trading_days = XuanguBaoClient::last_n_trading_days(10);
                let mut days = Vec::new();
                for date in &trading_days {
                    if let Ok(Some(d)) = client.fetch_market_overview_for_date(date).await {
                        let rise_ratio = d.rise_count as f64 / d.fall_count.max(1) as f64;
                        let sentiment = d.market_temperature * 0.5
                            + (1.0 - d.bomb_rate) * 25.0
                            + (rise_ratio / 3.0).min(1.0) * 25.0;
                        days.push(serde_json::json!({
                            "date": d.date,
                            "sentiment": (sentiment * 10.0).round() / 10.0,
                            "limit_up_count": d.limit_up_count,
                            "bomb_rate": (d.bomb_rate * 100.0).round() / 100.0,
                            "rise_ratio": (rise_ratio * 100.0).round() / 100.0,
                        }));
                    }
                }
                days.reverse();
                return Ok(serde_json::json!(days));
            }
            if let Some(rest) = path.strip_prefix("/api/kline/") {
                let parts: Vec<&str> = rest.split('?').collect();
                let symbol = parts[0];
                let period_str = extract_query(path, "period").unwrap_or_else(|| "day".into());
                let count: u32 = extract_query(path, "count")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(256);
                let period = match period_str.as_str() {
                    "week" => KlinePeriod::Week,
                    "month" => KlinePeriod::Month,
                    "minute" => KlinePeriod::Minute,
                    _ => KlinePeriod::Day,
                };
                let client = XuanguBaoClient::new();
                let data = client
                    .fetch_kline(symbol, period, count)
                    .await
                    .map_err(|e| format!("{e}"))?;
                return Ok(serde_json::to_value(data).unwrap_or_default());
            }
            if let Some(rest) = path.strip_prefix("/api/realtime/") {
                let symbol = rest.split('?').next().unwrap_or("");
                let client = XuanguBaoClient::new();
                let data = client
                    .fetch_realtime(symbol)
                    .await
                    .map_err(|e| format!("{e}"))?;
                return Ok(serde_json::to_value(data).unwrap_or_default());
            }
            if let Some(rest) = path.strip_prefix("/api/telegraph/") {
                let category = rest.split('?').next().unwrap_or("");
                let data = state
                    .api
                    .cls
                    .fetch_telegraph(category)
                    .await
                    .map_err(|e| format!("{e}"))?;
                return Ok(serde_json::to_value(data).unwrap_or_default());
            }
            if let Some(rest) = path.strip_prefix("/api/market-trend/") {
                let symbol = rest.split('?').next().unwrap_or("");
                let data = state
                    .api
                    .xuangubao
                    .fetch_market_trend(symbol)
                    .await
                    .map_err(|e| format!("{e}"))?;
                return Ok(serde_json::to_value(data).unwrap_or_default());
            }
            if let Some(rest) = path.strip_prefix("/api/plates-by-any-stock/") {
                let symbol = rest.split('?').next().unwrap_or("");
                let data = state
                    .api
                    .xuangubao
                    .plates_by_any_stock(symbol)
                    .await
                    .map_err(|e| format!("{e}"))?;
                return Ok(serde_json::to_value(data).unwrap_or_default());
            }
            if let Some(rest) = path.strip_prefix("/api/roles/") {
                return dispatch_roles_get(rest, state).await;
            }
            Err(format!("unknown path: {path}"))
        }
    }
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
        "positions" => {
            let positions = state
                .trade
                .list_positions(name)
                .map_err(|e| format!("{e}"))?;
            let result: Vec<serde_json::Value> = positions.iter().map(|p| {
                serde_json::json!({
                    "id": p.id, "symbol": p.symbol, "stock_name": p.stock_name,
                    "cost_price": p.cost_price, "current_price": p.current_price.unwrap_or(0.0),
                    "shares": p.shares, "entry_reason": p.entry_reason, "entry_date": p.entry_date,
                })
            }).collect();
            Ok(serde_json::json!(result))
        }
        "logs" => {
            let logs = state
                .trade
                .get_trade_logs_by_role(name, 20)
                .map_err(|e| format!("{e}"))?;
            let result: Vec<serde_json::Value> = logs
                .iter()
                .map(|l| {
                    let date: String = l.created_at.chars().take(10).collect();
                    serde_json::json!({
                        "date": date, "action": l.action, "symbol": l.symbol,
                        "price": l.price, "shares": l.shares, "reason": l.reason,
                    })
                })
                .collect();
            Ok(serde_json::json!(result))
        }
        "pnl-history" => {
            let days: usize = extract_query(rest, "days")
                .and_then(|s| s.parse().ok())
                .unwrap_or(30);
            let all_logs = state
                .trade
                .get_all_trade_logs_by_role(name)
                .map_err(|e| format!("{e}"))?;
            let positions = state.trade.list_positions(name).unwrap_or_default();
            let current_value: f64 = positions
                .iter()
                .map(|p| p.current_price.unwrap_or(p.cost_price) * p.shares as f64)
                .sum();
            let mut points =
                trending_agent_core::roles::pnl::compute_pnl_history(&all_logs, current_value);
            let cutoff = points.len().saturating_sub(days);
            points = points.into_iter().skip(cutoff).collect();
            Ok(serde_json::json!({"points": points, "initial_capital": 1_000_000.0}))
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
        "/api/roles" => {
            let name = body
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let display_name = body
                .get("display_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let strategy = body
                .get("strategy")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            trending_agent_core::roles::save_role_strategy(&state.strategies_dir, &name, &strategy)
                .map_err(|e| format!("{e}"))?;
            let mut roles = state.roles.write().await;
            if let Some(existing) = roles.iter_mut().find(|r| r.name == name) {
                existing.display_name = display_name;
                existing.strategy = strategy;
            } else {
                roles.push(RoleEntry {
                    name,
                    display_name,
                    strategy,
                });
            }
            Ok(serde_json::json!({"ok": true}))
        }
        _ => {
            if let Some(rest) = path.strip_prefix("/api/roles/") {
                let parts: Vec<&str> = rest.split('/').collect();
                if parts.len() < 2 {
                    return Err("invalid roles path".to_string());
                }
                let name = parts[0];
                let action = parts[1];
                match action {
                    "strategy" => {
                        let strategy = body
                            .get("strategy")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        trending_agent_core::roles::save_role_strategy(
                            &state.strategies_dir,
                            name,
                            &strategy,
                        )
                        .map_err(|e| format!("{e}"))?;
                        let mut roles = state.roles.write().await;
                        if let Some(entry) = roles.iter_mut().find(|r| r.name == name) {
                            entry.strategy = strategy;
                        }
                        Ok(serde_json::json!({"ok": true}))
                    }
                    "reset" => {
                        state
                            .trade
                            .delete_trade_logs_by_role(name)
                            .map_err(|e| format!("{e}"))?;
                        state
                            .trade
                            .delete_positions_by_role(name)
                            .map_err(|e| format!("{e}"))?;
                        Ok(serde_json::json!({"ok": true}))
                    }
                    _ => Err(format!("unknown roles action: {action}")),
                }
            } else {
                Err(format!("unknown path: {path}"))
            }
        }
    }
}

fn extract_query(path: &str, key: &str) -> Option<String> {
    let query_part = path.split('?').nth(1)?;
    for pair in query_part.split('&') {
        let mut kv = pair.splitn(2, '=');
        if kv.next() == Some(key) {
            return kv.next().map(|s| s.to_string());
        }
    }
    None
}

#[tauri::command]
async fn api_chat_stream(
    body: serde_json::Value,
    channel: tauri::ipc::Channel<String>,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), String> {
    eprintln!("[chat_stream] start, body={}", body);

    let llm = state.llm.clone();
    let api = state.api.clone();
    let roles = state.roles.clone();

    let role_name = body
        .get("role")
        .and_then(|v| v.as_str())
        .unwrap_or("zarili");
    eprintln!("[chat_stream] role={role_name}");

    eprintln!("[chat_stream] fetching overview...");
    let overview = trending_agent_core::web::chat::fetch_overview_data(&api)
        .await
        .map_err(|e| {
            eprintln!("[chat_stream] overview error: {e}");
            format!("overview error: {e}")
        })?;
    eprintln!("[chat_stream] overview ok");

    let field_items = trending_agent_core::web::chat::fetch_field_json(&api).await;
    let super_stocks = trending_agent_core::web::chat::fetch_super_stock_json(&api).await;
    eprintln!(
        "[chat_stream] field_items={}, super_stocks={}",
        field_items.is_some(),
        super_stocks.is_some()
    );

    let roles_guard = roles.read().await;
    let strategy_text = roles_guard
        .iter()
        .find(|r| r.name == role_name)
        .map(|r| r.strategy.as_str())
        .unwrap_or("你是一个A股短线交易AI助手。");

    let system_prompt = trending_agent_core::web::chat::build_system_prompt(strategy_text);
    let user_message = trending_agent_core::web::chat::build_user_message(
        &overview,
        field_items.as_deref(),
        super_stocks.as_deref(),
    );
    eprintln!(
        "[chat_stream] system_prompt len={}, user_message len={}",
        system_prompt.len(),
        user_message.len()
    );

    let mut messages_vec: Vec<serde_json::Value> = vec![
        serde_json::json!({"role": "system", "content": system_prompt}),
        serde_json::json!({"role": "user", "content": user_message}),
        serde_json::json!({"role": "assistant", "content": "已收到市场数据，请提问。"}),
    ];

    if let Some(history) = body.get("history").and_then(|h| h.as_array()) {
        eprintln!("[chat_stream] history len={}", history.len());
        for msg in history {
            messages_vec.push(msg.clone());
        }
    }

    if let Some(message) = body.get("message").and_then(|m| m.as_str()) {
        eprintln!("[chat_stream] user message: {message}");
        messages_vec.push(serde_json::json!({
            "role": "user",
            "content": message,
        }));
    }

    let prompt = serde_json::json!(messages_vec).to_string();
    eprintln!(
        "[chat_stream] calling llm chat_stream_with_messages, prompt len={}",
        prompt.len()
    );

    let mut llm_stream = llm.chat_stream_with_messages(&prompt).await.map_err(|e| {
        eprintln!("[chat_stream] llm error: {e}");
        format!("llm error: {e}")
    })?;
    eprintln!("[chat_stream] llm stream started");

    use futures_util::StreamExt;

    let mut chunk_count = 0u32;
    let mut empty_count = 0u32;
    while let Some(chunk) = llm_stream.next().await {
        let chunk: Result<String, _> = chunk;
        match chunk {
            Ok(text) if !text.is_empty() => {
                chunk_count += 1;
                if chunk_count <= 3 || chunk_count.is_multiple_of(20) {
                    eprintln!(
                        "[chat_stream] chunk #{chunk_count}: {:?}",
                        &text[..text.len().min(80)]
                    );
                }
                channel.send(text).map_err(|e| {
                    eprintln!("[chat_stream] channel send error: {e}");
                    format!("channel error: {e}")
                })?;
            }
            Ok(_) => {
                empty_count += 1;
            }
            Err(e) => {
                eprintln!("[chat_stream] stream error: {e}");
                return Err(format!("stream error: {e}"));
            }
        }
    }

    eprintln!("[chat_stream] done, total chunks={chunk_count}, empty chunks={empty_count}");
    Ok(())
}

#[tauri::command]
async fn api_replay_analyze(
    channel: tauri::ipc::Channel<String>,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let llm = state.llm.clone();

    let client = XuanguBaoClient::new();
    let events = client
        .fetch_all_day_events()
        .await
        .map_err(|e| format!("fetch error: {e}"))?;
    let date = XuanguBaoClient::last_n_trading_days(1)
        .first()
        .cloned()
        .unwrap_or_default();
    let snapshots = trending_agent_core::api::xuangubao::build_replay_snapshots(&events);
    let prompt = trending_agent_core::api::xuangubao::build_analysis_prompt(&date, &snapshots);

    let mut llm_stream = llm
        .chat_stream(&prompt)
        .await
        .map_err(|e| format!("llm error: {e}"))?;

    use futures_util::StreamExt;
    while let Some(chunk) = llm_stream.next().await {
        match chunk {
            Ok(text) if !text.is_empty() => {
                channel
                    .send(text)
                    .map_err(|e| format!("channel error: {e}"))?;
            }
            Err(e) => {
                channel
                    .send(format!("[ERROR] {e}"))
                    .map_err(|e| format!("channel error: {e}"))?;
                break;
            }
            _ => {}
        }
    }
    Ok(())
}

fn main() {
    let project_root = find_project_root();
    load_env(&project_root);

    let db_path = project_root.join(".atrading").join("data.db");

    let gateway = Gateway::init(&db_path).expect("failed to init gateway");
    let memory = gateway.memory.clone();
    let trade = gateway.trade.clone();

    let roles = build_roles();
    let strategies_dir = project_root.join(".atrading").join("strategies");

    let app_state = Arc::new(AppState {
        llm: LlmClient::from_env(),
        api: ApiClient::new(),
        memory,
        trade,
        roles: Arc::new(RwLock::new(roles)),
        strategies_dir,
        cache: trending_agent_core::web::cache::new_cache(),
    });

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            api_fetch,
            api_post,
            api_chat_stream,
            api_replay_analyze
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
