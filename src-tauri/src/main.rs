#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use openatrading_core::api::xuangubao::{KlinePeriod, XuanguBaoClient};
use openatrading_core::commands::CommandType;
use openatrading_core::llm::{
    assistant_event_to_stream_events, AgentEvent, AgentMessage, StreamEvent,
};
use openatrading_web::gateway::Gateway;
use openatrading_web::{AppState, RoleEntry};
use std::sync::Arc;
use tauri::Emitter;
use tokio::sync::RwLock;

fn load_env() {
    let env_path = openatrading_core::atrading_home().join(".env");
    if env_path.exists() {
        dotenvy::from_path(&env_path).ok();
        eprintln!("[env] loaded {}", env_path.display());
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
                display_name: String,
            }
            if let Ok(parsed) = toml::from_str::<RolesCfg>(&raw) {
                let skills_dir = openatrading_core::roles::default_skills_dir();
                for rc in parsed.roles {
                    if !roles.iter().any(|r| r.name == rc.name) {
                        let role_path = skills_dir.join(&rc.name).join("SKILL.md");
                        if let Ok(raw) = std::fs::read_to_string(&role_path) {
                            let (fm, body) = openatrading_core::roles::parse_frontmatter(&raw);
                            if !body.is_empty() {
                                roles.push(RoleEntry {
                                    name: rc.name,
                                    display_name: fm
                                        .get("description")
                                        .cloned()
                                        .unwrap_or(rc.display_name),
                                    strategy: body.to_string(),
                                    readonly: true,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // Discover roles from disk (editable)
    roles.extend(openatrading_core::roles::list_roles_from_disk());
    if roles.is_empty() {
        eprintln!("[tauri] 警告: skills/ 目录中未发现任何角色");
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
                "date": openatrading_core::utils::time::beijing_now().format("%Y-%m-%d %H:%M").to_string(),
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
        },
        "/api/limit-up" => {
            let client = XuanguBaoClient::new();
            let data = client.fetch_limit_up().await.map_err(|e| format!("{e}"))?;
            Ok(serde_json::to_value(data).unwrap_or_default())
        },
        "/api/limit-down" => {
            let client = XuanguBaoClient::new();
            let data = client
                .fetch_limit_down()
                .await
                .map_err(|e| format!("{e}"))?;
            Ok(serde_json::to_value(data).unwrap_or_default())
        },
        "/api/super-stock" => {
            let client = XuanguBaoClient::new();
            let data = client
                .fetch_super_stock()
                .await
                .map_err(|e| format!("{e}"))?;
            Ok(serde_json::to_value(data).unwrap_or_default())
        },
        "/api/yesterday-limit-up" => {
            let client = XuanguBaoClient::new();
            let data = client
                .fetch_yesterday_limit_up()
                .await
                .map_err(|e| format!("{e}"))?;
            Ok(serde_json::to_value(data).unwrap_or_default())
        },
        "/api/index-quotes" => {
            let client = XuanguBaoClient::new();
            let data = client
                .fetch_market_real(&["000001.SS", "399001.SZ", "399006.SZ"])
                .await
                .map_err(|e| format!("{e}"))?;
            Ok(serde_json::to_value(data).unwrap_or_default())
        },
        "/api/dapan-abnormal" => {
            let data = state
                .ctx
                .api
                .xuangubao
                .fetch_dapan_abnormal()
                .await
                .map_err(|e| format!("{e}"))?;
            Ok(serde_json::to_value(data).unwrap_or_default())
        },
        "/api/timeline" => {
            if std::env::var("JIUYAN_PHONE").is_err() {
                return Ok(serde_json::json!([]));
            }
            let month = openatrading_core::utils::time::beijing_now()
                .format("%Y-%m")
                .to_string();
            let data = state
                .ctx
                .api
                .jiuyan
                .fetch_timeline(&month)
                .await
                .map_err(|e| format!("{e}"))?;
            Ok(serde_json::to_value(data).unwrap_or_default())
        },

        "/api/telegraph" => {
            let data = state
                .ctx
                .api
                .cls
                .fetch_telegraph("", None, 20)
                .await
                .map_err(|e| format!("{e}"))?;
            Ok(serde_json::to_value(data).unwrap_or_default())
        },
        "/api/dragon-tiger/combined" => {
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
            Ok(serde_json::to_value(data).unwrap_or_default())
        },
        "/api/ai/roles" => {
            let roles_guard = state.roles.read().await;
            let list: Vec<serde_json::Value> = roles_guard
                .iter()
                .map(|r| serde_json::json!({"name": r.name, "display_name": r.display_name}))
                .collect();
            Ok(serde_json::json!(list))
        },
        "/api/ai/commands" => {
            let registry = state.ctx.tool_registry.load();
            let skills = openatrading_core::skills::SkillRegistry::from_disk();
            let mut commands: Vec<serde_json::Value> = Vec::new();
            for t in registry.all_tools() {
                commands.push(serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "command_type": "tool",
                }));
            }
            for s in skills.skills() {
                commands.push(serde_json::json!({
                    "name": s.name,
                    "description": s.description,
                    "command_type": "skill",
                }));
            }
            Ok(serde_json::json!(commands))
        },
        "/api/ai/sessions" => {
            let cwd = std::env::current_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let dir = std::path::Path::new(&cwd).join(".pi-rs/sessions");
            let mut sessions: Vec<serde_json::Value> = Vec::new();
            if let Ok(entries) = std::fs::read_dir(&dir) {
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
                    sessions.push(serde_json::json!({
                        "id": stem,
                        "title": stem,
                        "created_at": "",
                        "updated_at": "",
                        "message_count": 0,
                    }));
                }
            }
            sessions.sort_by(|a, b| b["id"].as_str().cmp(&a["id"].as_str()));
            Ok(serde_json::to_value(sessions).unwrap_or_default())
        },
        "/api/roles" => {
            let roles_guard = state.roles.read().await;
            Ok(serde_json::to_value(roles_guard.clone()).unwrap_or_default())
        },
        "/api/watchlist" => {
            #[derive(serde::Serialize, serde::Deserialize)]
            struct WsItem {
                code: String,
                name: String,
                added_at: String,
            }
            fn path_buf() -> std::path::PathBuf {
                openatrading_core::atrading_home().join("watchlist.json")
            }
            fn load() -> Vec<WsItem> {
                let p = path_buf();
                if !p.exists() {
                    return vec![];
                }
                std::fs::read_to_string(&p)
                    .ok()
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default()
            }
            Ok(serde_json::to_value(load()).unwrap_or_default())
        },
        "/api/replay/snapshots" => {
            let client = XuanguBaoClient::new();
            let events = client
                .fetch_all_day_events(0.03)
                .await
                .map_err(|e| format!("{e}"))?;
            let snapshots = openatrading_core::api::xuangubao::build_replay_snapshots(&events);
            Ok(serde_json::to_value(snapshots).unwrap_or_default())
        },
        "/api/replay/index" => {
            use openatrading_core::api::xuangubao::fetch_index_trends;
            let client = XuanguBaoClient::new();
            let data = fetch_index_trends(&client)
                .await
                .map_err(|e| format!("{e}"))?;
            Ok(serde_json::to_value(data).unwrap_or_default())
        },
        _ => {
            if path.strip_prefix("/api/replay/events").is_some() {
                let min_pcp: f64 = extract_query(path, "min_pcp")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0.03);
                let client = XuanguBaoClient::new();
                let all_events = client
                    .fetch_all_day_events(min_pcp)
                    .await
                    .map_err(|e| format!("{e}"))?;
                let events: Vec<openatrading_core::api::xuangubao::ReplayEvent> = all_events
                    .iter()
                    .map(openatrading_core::api::xuangubao::ReplayEvent::from)
                    .collect();
                let date = XuanguBaoClient::last_n_trading_days(1)
                    .first()
                    .cloned()
                    .unwrap_or_default();
                let page = openatrading_core::api::xuangubao::ReplayPage {
                    date,
                    market_phase: String::new(),
                    events,
                    next_cursor: None,
                };
                return Ok(serde_json::to_value(page).unwrap_or_default());
            }
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
            if path.strip_prefix("/api/search-stocks").is_some() {
                let q = extract_query(path, "q").unwrap_or_default();
                if q.is_empty() {
                    return Err("missing q".to_string());
                }
                use openatrading_core::api::eastmoney::LhbClient;
                let client = LhbClient::new();
                let items = client.search_stocks(&q).await.map_err(|e| format!("{e}"))?;
                return Ok(serde_json::to_value(items).unwrap_or_default());
            }
            if path.strip_prefix("/api/field-items").is_some() {
                let date = extract_query(path, "date").unwrap_or_else(|| {
                    openatrading_core::utils::time::beijing_now()
                        .format("%Y-%m-%d")
                        .to_string()
                });
                if std::env::var("JIUYAN_PHONE").is_err() {
                    return Ok(serde_json::json!([]));
                }
                let data = state
                    .ctx
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
                        days.push(serde_json::json!({
                            "date": d.date,
                            "sentiment": (d.market_temperature * 10.0).round() / 10.0,
                            "limit_up_count": d.limit_up_count,
                            "bomb_rate": (d.bomb_rate * 100.0).round() / 100.0,
                            "rise_ratio": (rise_ratio * 100.0).round() / 100.0,
                        }));
                    }
                }
                days.reverse();
                return Ok(serde_json::json!(days));
            }
            if path.strip_prefix("/api/dragon-tiger/detail").is_some() {
                let dates = openatrading_core::api::utils::last_n_trading_days(3);
                let raw = state
                    .ctx
                    .api
                    .eastmoney
                    .lhb_detail(
                        dates.last().map(|s| s.as_str()).unwrap_or(""),
                        dates.first().map(|s| s.as_str()).unwrap_or(""),
                    )
                    .await
                    .map_err(|e| format!("{e}"))?;
                // Merge same-stock entries
                use std::collections::HashMap;
                let mut merged: HashMap<
                    (String, String),
                    openatrading_core::api::eastmoney::LhbDetail,
                > = HashMap::new();
                for item in raw {
                    let key = (item.code.clone(), item.trade_date.clone());
                    match merged.get_mut(&key) {
                        Some(existing) => {
                            existing.billboard_net_amt = existing
                                .billboard_net_amt
                                .zip(item.billboard_net_amt)
                                .map(|(a, b)| a + b);
                            existing.billboard_buy_amt = existing
                                .billboard_buy_amt
                                .zip(item.billboard_buy_amt)
                                .map(|(a, b)| a + b);
                            existing.billboard_sell_amt = existing
                                .billboard_sell_amt
                                .zip(item.billboard_sell_amt)
                                .map(|(a, b)| a + b);
                            existing.billboard_deal_amt = existing
                                .billboard_deal_amt
                                .zip(item.billboard_deal_amt)
                                .map(|(a, b)| a + b);
                            if let Some(ref exp) = item.explanation {
                                if !exp.is_empty() {
                                    let prefix = existing.explanation.as_deref().unwrap_or("");
                                    let sep = if prefix.is_empty() { "" } else { "; " };
                                    existing.explanation = Some(format!("{prefix}{sep}{exp}"));
                                }
                            }
                        },
                        None => {
                            merged.insert(key, item);
                        },
                    }
                }
                let merged_all: Vec<_> = merged.into_values().collect();
                let total = merged_all.len();
                let total_net_amt: f64 =
                    merged_all.iter().filter_map(|d| d.billboard_net_amt).sum();
                let net_buy_count = merged_all
                    .iter()
                    .filter(|d| d.billboard_net_amt.is_some_and(|v| v > 0.0))
                    .count();
                let net_sell_count = merged_all
                    .iter()
                    .filter(|d| d.billboard_net_amt.is_some_and(|v| v < 0.0))
                    .count();
                let page: usize = extract_query(path, "page")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1);
                let page_size: usize = extract_query(path, "page_size")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(20);
                let start = page.saturating_sub(1) * page_size;
                let data: Vec<_> = merged_all.into_iter().skip(start).take(page_size).collect();
                return Ok(serde_json::json!({
                    "data": data, "total": total, "page": page, "page_size": page_size,
                    "total_net_amt": total_net_amt, "net_buy_count": net_buy_count, "net_sell_count": net_sell_count,
                }));
            }
            if path.strip_prefix("/api/dragon-tiger/stock-seat").is_some() {
                let code = extract_query(path, "code").unwrap_or_default();
                let date = extract_query(path, "date").unwrap_or_default();
                let flag = extract_query(path, "flag").unwrap_or_default();
                let seat_flag = match flag.as_str() {
                    "buy" => openatrading_core::api::eastmoney::SeatFlag::Buy,
                    "sell" => openatrading_core::api::eastmoney::SeatFlag::Sell,
                    _ => return Err("flag must be 'buy' or 'sell'".to_string()),
                };
                let seats = state
                    .ctx
                    .api
                    .eastmoney
                    .lhb_stock_seat(&code, &date, seat_flag)
                    .await
                    .map_err(|e| format!("{e}"))?;
                return Ok(serde_json::to_value(seats).unwrap_or_default());
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
                    .ctx
                    .api
                    .cls
                    .fetch_telegraph(category, None, 20)
                    .await
                    .map_err(|e| format!("{e}"))?;
                return Ok(serde_json::to_value(data).unwrap_or_default());
            }
            if let Some(rest) = path.strip_prefix("/api/market-trend/") {
                let symbol = rest.split('?').next().unwrap_or("");
                let data = state
                    .ctx
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
                    .ctx
                    .api
                    .xuangubao
                    .plates_by_any_stock(symbol)
                    .await
                    .map_err(|e| format!("{e}"))?;
                return Ok(serde_json::to_value(data).unwrap_or_default());
            }
            if let Some(rest) = path.strip_prefix("/api/ai/sessions/") {
                if let Some(msg_id) = rest.strip_suffix("/messages") {
                    let cwd = std::env::current_dir()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    let fpath = std::path::Path::new(&cwd)
                        .join(".pi-rs/sessions")
                        .join(format!("{msg_id}.jsonl"));
                    if !fpath.exists() {
                        return Err("session not found".to_string());
                    }
                    let content = std::fs::read_to_string(&fpath).map_err(|e| format!("{e}"))?;
                    let mut messages: Vec<serde_json::Value> = Vec::new();
                    for line in content.lines().skip(1) {
                        if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
                            if entry.get("type").and_then(|v| v.as_str()) != Some("message") {
                                continue;
                            }
                            if let Some(msg) = entry.get("message") {
                                let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");
                                let text = msg
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
                                messages.push(serde_json::json!({"role": role, "content": text}));
                            }
                        }
                    }
                    return Ok(serde_json::to_value(messages).unwrap_or_default());
                }
            }
            if let Some(rest) = path.strip_prefix("/api/roles/") {
                return dispatch_roles_get(rest, state).await;
            }
            Err(format!("unknown path: {path}"))
        },
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
                .ctx
                .trade
                .list_positions(name)
                .map_err(|e| format!("{e}"))?;
            let result: Vec<serde_json::Value> = positions
                .iter()
                .map(|p| {
                    serde_json::json!({
                        "id": p.id, "symbol": p.symbol, "stock_name": p.stock_name,
                        "cost_price": p.cost_price, "current_price": p.current_price.unwrap_or(0.0),
                        "shares": p.shares, "entry_reason": p.entry_reason, "entry_date": p.entry_date,
                    })
                })
                .collect();
            Ok(serde_json::json!(result))
        },
        "logs" => {
            let logs = state
                .ctx
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
        },
        "pnl-history" => {
            let days: usize = extract_query(rest, "days")
                .and_then(|s| s.parse().ok())
                .unwrap_or(30);
            let all_logs = state
                .ctx
                .trade
                .get_all_trade_logs_by_role(name)
                .map_err(|e| format!("{e}"))?;
            let positions = state.ctx.trade.list_positions(name).unwrap_or_default();
            let current_value: f64 = positions
                .iter()
                .map(|p| p.current_price.unwrap_or(p.cost_price) * p.shares as f64)
                .sum();
            let mut points =
                openatrading_core::roles::pnl::compute_pnl_history(&all_logs, current_value);
            if days > 0 {
                let cutoff = points.len().saturating_sub(days);
                points = points.into_iter().skip(cutoff).collect();
            }
            Ok(serde_json::json!({"points": points, "initial_capital": 1_000_000.0}))
        },
        "history" => {
            let limit: usize = extract_query(rest, "limit")
                .and_then(|s| s.parse().ok())
                .unwrap_or(100);
            let positions = state
                .ctx
                .trade
                .get_closed_positions(name, limit)
                .map_err(|e| format!("{e}"))?;
            Ok(serde_json::to_value(positions).unwrap_or_default())
        },
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
            let (id, title) = openatrading_core::llm::create_session()
                .unwrap_or_else(|_| (String::new(), String::new()));
            Ok(
                serde_json::json!({"id": id, "title": title, "created_at": "", "updated_at": "", "message_count": 0}),
            )
        },
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
            let strategy = body
                .get("strategy")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            openatrading_core::roles::save_role_strategy(&name, &strategy)
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
                    readonly: false,
                });
            }
            Ok(serde_json::json!({"ok": true}))
        },
        _ => {
            if let Some(rest) = path.strip_prefix("/api/roles/") {
                let parts: Vec<&str> = rest.split('/').collect();
                let name = parts[0];
                if parts.len() < 2 {
                    // DELETE /api/roles/{name} — no action suffix → delete
                    state
                        .ctx
                        .trade
                        .delete_positions_by_role(name)
                        .map_err(|e| format!("{e}"))?;
                    state
                        .ctx
                        .trade
                        .delete_trade_logs_by_role(name)
                        .map_err(|e| format!("{e}"))?;
                    openatrading_core::roles::delete_role_dir(name).map_err(|e| format!("{e}"))?;
                    let mut roles = state.roles.write().await;
                    roles.retain(|r| r.name != name);
                    return Ok(serde_json::json!({"ok": true}));
                }
                let action = parts[1];
                match action {
                    "strategy" => {
                        let strategy = body
                            .get("strategy")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        openatrading_core::roles::save_role_strategy(name, &strategy)
                            .map_err(|e| format!("{e}"))?;
                        let mut roles = state.roles.write().await;
                        if let Some(entry) = roles.iter_mut().find(|r| r.name == name) {
                            entry.strategy = strategy;
                        }
                        Ok(serde_json::json!({"ok": true}))
                    },
                    "reset" => {
                        state
                            .ctx
                            .trade
                            .delete_trade_logs_by_role(name)
                            .map_err(|e| format!("{e}"))?;
                        state
                            .ctx
                            .trade
                            .delete_positions_by_role(name)
                            .map_err(|e| format!("{e}"))?;
                        Ok(serde_json::json!({"ok": true}))
                    },
                    "delete" => {
                        state
                            .ctx
                            .trade
                            .delete_positions_by_role(name)
                            .map_err(|e| format!("{e}"))?;
                        state
                            .ctx
                            .trade
                            .delete_trade_logs_by_role(name)
                            .map_err(|e| format!("{e}"))?;
                        openatrading_core::roles::delete_role_dir(name)
                            .map_err(|e| format!("{e}"))?;
                        let mut roles = state.roles.write().await;
                        roles.retain(|r| r.name != name);
                        Ok(serde_json::json!({"ok": true}))
                    },
                    _ => Err(format!("unknown roles action: {action}")),
                }
            } else if let Some(sid) = path.strip_prefix("/api/ai/sessions/") {
                // DELETE /api/ai/sessions/:id
                let cwd = std::env::current_dir()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let fpath = std::path::Path::new(&cwd)
                    .join(".pi-rs/sessions")
                    .join(format!("{sid}.jsonl"));
                if fpath.exists() {
                    std::fs::remove_file(&fpath).map_err(|e| format!("{e}"))?;
                    Ok(serde_json::json!({"ok": true}))
                } else {
                    Err(format!("session not found: {sid}"))
                }
            } else if path.starts_with("/api/watchlist") {
                #[derive(serde::Serialize, serde::Deserialize)]
                struct WsItem {
                    code: String,
                    name: String,
                    added_at: String,
                }
                fn wl_path() -> std::path::PathBuf {
                    openatrading_core::atrading_home().join("watchlist.json")
                }
                fn wl_load() -> Vec<WsItem> {
                    let p = wl_path();
                    if !p.exists() {
                        return vec![];
                    }
                    std::fs::read_to_string(&p)
                        .ok()
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or_default()
                }
                fn wl_save(items: &[WsItem]) -> Result<(), String> {
                    let p = wl_path();
                    if let Some(parent) = p.parent() {
                        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                    }
                    std::fs::write(
                        &p,
                        serde_json::to_string_pretty(items).map_err(|e| e.to_string())?,
                    )
                    .map_err(|e| e.to_string())
                }

                if let Some(code) = path.strip_prefix("/api/watchlist/") {
                    // DELETE /api/watchlist/CODE
                    let code = url_decode(code);
                    let mut items = wl_load();
                    let before = items.len();
                    items.retain(|i| i.code != code);
                    if items.len() == before {
                        return Err(format!("not found: {code}"));
                    }
                    wl_save(&items).map_err(|e| e.to_string())?;
                    Ok(serde_json::to_value(items).unwrap_or_default())
                } else {
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
                    let mut items = wl_load();
                    if items.iter().any(|i| i.code == code) {
                        return Err("already exists".to_string());
                    }
                    items.push(WsItem {
                        code,
                        name,
                        added_at: chrono::Utc::now().to_rfc3339(),
                    });
                    wl_save(&items).map_err(|e| e.to_string())?;
                    Ok(serde_json::to_value(items).unwrap_or_default())
                }
            } else {
                Err(format!("unknown path: {path}"))
            }
        },
    }
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
async fn api_chat_stream(
    body: serde_json::Value,
    channel: tauri::ipc::Channel<String>,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), String> {
    eprintln!("[chat_stream] start, body={}", body);

    let api = state.ctx.api.clone();
    let roles = state.roles.clone();
    let tools_registry = &state.ctx.tool_registry;

    let role_name = body.get("role").and_then(|v| v.as_str()).unwrap_or("");
    eprintln!("[chat_stream] role={role_name}");

    eprintln!("[chat_stream] fetching overview...");
    let overview = openatrading_web::chat::fetch_overview_data(&api)
        .await
        .map_err(|e| {
            eprintln!("[chat_stream] overview error: {e}");
            format!("overview error: {e}")
        })?;
    eprintln!("[chat_stream] overview ok");

    let field_items = openatrading_web::chat::fetch_field_json(&api).await;
    let super_stocks = openatrading_web::chat::fetch_super_stock_json(&api).await;
    eprintln!(
        "[chat_stream] field_items={}, super_stocks={}",
        field_items.is_some(),
        super_stocks.is_some()
    );

    let roles_guard = roles.read().await;
    let default_strategy = openatrading_core::prompts::DEFAULT_CHAT_SYSTEM_PROMPT;

    let command_type_str = body
        .get("command_type")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let command_name = body
        .get("command_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let effective_command_type = match command_type_str {
        "tool" => Some(CommandType::Tool),
        "skill" => Some(CommandType::Skill),
        _ => None,
    };

    let (system_prompt, tools) = match &effective_command_type {
        Some(CommandType::Tool) => {
            let strategy_text = roles_guard
                .iter()
                .find(|r| r.name == role_name)
                .map(|r| r.strategy.as_str())
                .unwrap_or(default_strategy);
            let sp = openatrading_web::chat::build_system_prompt(strategy_text);
            let filtered: Vec<std::sync::Arc<openatrading_core::llm::DynTool>> = tools_registry
                .load()
                .all_tools()
                .iter()
                .filter(|t| t.name == command_name)
                .cloned()
                .collect();
            (sp, filtered)
        },
        Some(CommandType::Skill) => {
            let skills = openatrading_core::skills::SkillRegistry::from_disk();
            let prompt = skills
                .skills()
                .iter()
                .find(|s| s.name == command_name)
                .map(|s| s.content.as_str())
                .unwrap_or(default_strategy);
            (
                openatrading_web::chat::build_system_prompt(prompt),
                tools_registry.load().all_tools().to_vec(),
            )
        },
        None => {
            let strategy_text = roles_guard
                .iter()
                .find(|r| r.name == role_name)
                .map(|r| r.strategy.as_str())
                .unwrap_or(default_strategy);
            (
                openatrading_web::chat::build_system_prompt(strategy_text),
                tools_registry.load().all_tools().to_vec(),
            )
        },
    };

    use openatrading_core::llm::pi_ai_types::{ContentBlock, StopReason};

    // ── 构建市场数据上下文消息 ──

    let user_message = openatrading_web::chat::build_user_message(
        &overview,
        field_items.as_deref(),
        super_stocks.as_deref(),
    );
    eprintln!(
        "[chat_stream] system_prompt len={}, user_message len={}",
        system_prompt.len(),
        user_message.len()
    );

    let mut pi_messages: Vec<AgentMessage> = Vec::new();
    // Data context as first user message
    pi_messages.push(AgentMessage::User {
        content: vec![ContentBlock::Text {
            text: user_message,
            text_signature: None,
        }],
        timestamp: 0,
    });
    // Assistant ack
    pi_messages.push(AgentMessage::Assistant {
        content: vec![ContentBlock::Text {
            text: "已收到市场数据，请提问。".into(),
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

    // History
    if let Some(history) = body.get("history").and_then(|h| h.as_array()) {
        eprintln!("[chat_stream] history len={}", history.len());
        for msg in history {
            let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");
            let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");
            match role {
                "user" => {
                    pi_messages.push(AgentMessage::User {
                        content: vec![ContentBlock::Text {
                            text: content.to_string(),
                            text_signature: None,
                        }],
                        timestamp: 0,
                    });
                },
                "assistant" => {
                    pi_messages.push(AgentMessage::Assistant {
                        content: vec![ContentBlock::Text {
                            text: content.to_string(),
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
    }

    // Current user message
    if let Some(message) = body.get("message").and_then(|m| m.as_str()) {
        eprintln!("[chat_stream] user message: {message}");
        pi_messages.push(AgentMessage::User {
            content: vec![ContentBlock::Text {
                text: message.to_string(),
                text_signature: None,
            }],
            timestamp: 0,
        });
    }

    eprintln!("[chat_stream] creating agent and subscribing to events");

    let (tx, mut rx) = tokio::sync::mpsc::channel::<AgentEvent>(256);

    let session_id = body
        .get("session_id")
        .and_then(|v| v.as_str())
        .map(String::from);
    let (sid_tx, sid_rx) = tokio::sync::oneshot::channel::<Result<String, String>>();
    tokio::spawn(async move {
        let result = openatrading_core::llm::process_messages(
            &system_prompt,
            tools,
            pi_messages,
            tx,
            session_id,
        )
        .await;
        let _ = sid_tx.send(result.map(|(_, id)| id).map_err(|e| e.to_string()));
    });

    eprintln!("[chat_stream] reading events from channel");

    // Send session_id first so the frontend can store it
    match sid_rx.await {
        Ok(Ok(id)) => {
            channel
                .send(format!("\x00SESSION\x00{}", id))
                .map_err(|e| format!("channel error: {e}"))?;
        },
        Ok(Err(e)) => {
            let err = format!("agent error: {e}");
            eprintln!("[chat_stream] {err}");
            channel
                .send(format!("\x00ERROR\x00{}", err))
                .map_err(|e| format!("channel error: {e}"))?;
            return Ok(());
        },
        Err(_) => return Ok(()),
    }

    let mut text_count = 0u32;
    loop {
        match rx.recv().await {
            Some(AgentEvent::MessageUpdate {
                assistant_message_event,
                ..
            }) => {
                let stream_events = assistant_event_to_stream_events(&assistant_message_event);
                for se in stream_events {
                    match se {
                        StreamEvent::Text(t) => {
                            text_count += 1;
                            if text_count <= 3 || text_count.is_multiple_of(20) {
                                eprintln!(
                                    "[chat_stream] text #{text_count}: {:?}",
                                    &t[..t.len().min(80)]
                                );
                            }
                            channel
                                .send(format!("\x00TEXT\x00{}", t.replace('\n', "\\n")))
                                .map_err(|e| format!("channel error: {e}"))?;
                        },
                        StreamEvent::ReasoningText(t) => {
                            channel
                                .send(format!("\x00THINKING\x00{}", t.replace('\n', "\\n")))
                                .map_err(|e| format!("channel error: {e}"))?;
                        },
                        StreamEvent::ToolCallStart(tc) => {
                            let data = serde_json::to_string(&serde_json::json!({
                                "id": tc.id,
                                "name": tc.function_name,
                                "args": tc.arguments,
                            }))
                            .unwrap_or_default();
                            channel
                                .send(format!("\x00TOOL_CALL\x00{data}"))
                                .map_err(|e| format!("channel error: {e}"))?;
                        },
                        StreamEvent::Error(e) => {
                            channel
                                .send(format!("[ERROR] {e}"))
                                .map_err(|f| format!("channel error: {f}"))?;
                        },
                        _ => {},
                    }
                }
            },
            Some(AgentEvent::ToolExecutionEnd {
                tool_call_id,
                tool_name,
                result,
                is_error,
            }) => {
                let status = if is_error { "error" } else { "success" };
                let data = serde_json::to_string(&serde_json::json!({
                    "id": tool_call_id,
                    "name": tool_name,
                    "result": result,
                    "status": status,
                }))
                .unwrap_or_default();
                channel
                    .send(format!("\x00TOOL_RESULT\x00{data}"))
                    .map_err(|e| format!("channel error: {e}"))?;
            },
            Some(AgentEvent::AgentEnd { .. }) => {
                eprintln!("[chat_stream] AgentEnd, done");
                break;
            },
            Some(_) => {},
            None => {
                eprintln!("[chat_stream] channel closed unexpectedly");
                break;
            },
        }
    }

    eprintln!("[chat_stream] done, total text chunks={text_count}");
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

    loop {
        match client.fetch_realtime_batch(&symbols).await {
            Ok(data) => {
                let json = serde_json::to_string(&data).unwrap_or_default();
                channel
                    .send(json)
                    .map_err(|e| format!("channel error: {e}"))?;
            },
            Err(e) => {
                let msg = e.to_string();
                eprintln!("[realtime-stream] {:?}: {msg}", symbols);
                channel
                    .send(format!(r#"{{"error":"{msg}"}}"#))
                    .map_err(|f| format!("channel error: {f}"))?;
            },
        }
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
}

#[tauri::command]
async fn api_kline_stream(
    symbol: String,
    period: String,
    count: u32,
    channel: tauri::ipc::Channel<String>,
) -> Result<(), String> {
    use openatrading_core::api::xuangubao::{KlinePeriod, XuanguBaoClient};
    let kline_period = match period.as_str() {
        "week" => KlinePeriod::Week,
        "month" => KlinePeriod::Month,
        "minute" => KlinePeriod::Minute,
        _ => KlinePeriod::Day,
    };
    let client = XuanguBaoClient::new();
    loop {
        match client.fetch_kline(&symbol, kline_period, count).await {
            Ok(data) => {
                let json = serde_json::to_string(&data).unwrap_or_default();
                channel
                    .send(json)
                    .map_err(|e| format!("channel error: {e}"))?;
            },
            Err(e) => {
                eprintln!("[kline-stream] {symbol}: {e}");
                channel
                    .send(format!(r#"{{"error":"{e}"}}"#))
                    .map_err(|f| format!("channel error: {f}"))?;
            },
        }
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
}

#[tauri::command]
async fn api_replay_analyze(
    channel: tauri::ipc::Channel<String>,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let llm = state.ctx.llm.clone();

    let client = XuanguBaoClient::new();
    let events = client
        .fetch_all_day_events(0.03)
        .await
        .map_err(|e| format!("fetch error: {e}"))?;
    let date = XuanguBaoClient::last_n_trading_days(1)
        .first()
        .cloned()
        .unwrap_or_default();
    let snapshots = openatrading_core::api::xuangubao::build_replay_snapshots(&events);
    let prompt = openatrading_core::api::xuangubao::build_analysis_prompt(&date, &snapshots);

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
            },
            Err(e) => {
                channel
                    .send(format!("[ERROR] {e}"))
                    .map_err(|e| format!("channel error: {e}"))?;
                break;
            },
            _ => {},
        }
    }
    Ok(())
}

fn load_llm_config_from_toml() -> (Option<String>, Option<String>) {
    match openatrading_cli::cli::config::AppConfig::load() {
        Ok(cfg) => (
            cfg.llm_provider().map(String::from),
            cfg.llm_model().map(String::from),
        ),
        Err(_) => (None, None),
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

    let llm = serde_json::json!({
        "provider": cfg.llm.provider,
        "model": cfg.llm.model,
    });

    Ok(serde_json::json!({ "llm": llm, "social": social }))
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
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        h.restart();
    });
    Ok(())
}

fn main() {
    load_env();

    let db_path = openatrading_core::atrading_home().join("data.db");

    let gateway = Gateway::init(&db_path).expect("failed to init gateway");

    let roles = build_roles();

    let (provider, model) = load_llm_config_from_toml();

    let app_ctx = tokio::runtime::Runtime::new().unwrap().block_on(
        openatrading_core::context::AppContext::build(
            provider.as_deref(),
            model.as_deref(),
            &[],
            gateway.memory.clone(),
            gateway.trade.clone(),
            Arc::new(openatrading_core::social::NoopChannel),
        ),
    );

    let app_state = Arc::new(AppState {
        ctx: app_ctx,
        roles: Arc::new(RwLock::new(roles)),
        mcp_servers: std::sync::Mutex::new(Vec::new()),
        cache: openatrading_web::cache::new_cache(),
    });

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            api_fetch,
            api_post,
            api_chat_stream,
            api_realtime_stream,
            api_kline_stream,
            api_replay_analyze,
            api_search_stocks,
            get_config,
            update_config,
            restart_app
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
