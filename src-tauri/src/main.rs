#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use openatrading_core::api::xuangubao::{KlinePeriod, XuanguBaoClient};
use openatrading_web::gateway::Gateway;
use openatrading_web::{AppState, RoleEntry};
use std::sync::Arc;
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
        "/api/sentiment" => {
            if std::env::var("JIUYAN_PHONE").is_err() {
                return Err("JIUYAN_PHONE not set".into());
            }
            let data = openatrading_core::agents::sentiment::analyze_sentiment(&state.ctx.api)
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
        "/api/roles" => {
            let roles_guard = state.roles.read().await;
            Ok(serde_json::to_value(roles_guard.clone()).unwrap_or_default())
        },
        "/api/replay/events" => {
            let client = XuanguBaoClient::new();
            let has_cursor = extract_query(path, "cursor").is_some();
            let timestamp = extract_query(path, "cursor")
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(XuanguBaoClient::latest_trading_day_close_ts);
            let raw = client
                .fetch_events_page(timestamp, 1000, "11000,11001")
                .await
                .map_err(|e| format!("{e}"))?;
            let events: Vec<openatrading_core::api::xuangubao::ReplayEvent> = raw
                .iter()
                .map(openatrading_core::api::xuangubao::ReplayEvent::from)
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
                openatrading_web::cache::current_market_phase()
                    .as_str()
                    .to_string()
            };
            let page = openatrading_core::api::xuangubao::ReplayPage {
                date,
                market_phase,
                events,
                next_cursor,
            };
            Ok(serde_json::to_value(page).unwrap_or_default())
        },
        "/api/replay/snapshots" => {
            let client = XuanguBaoClient::new();
            let events = client
                .fetch_all_day_events()
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
            if path.strip_prefix("/api/dragon-tiger/detail").is_some() {
                let dates = openatrading_core::api::utils::last_n_trading_days(3);
                let raw = state
                    .ctx
                    .api
                    .eastmoney
                    .lhb_detail(
                        dates.first().map(|s| s.as_str()).unwrap_or(""),
                        dates.last().map(|s| s.as_str()).unwrap_or(""),
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
            } else {
                Err(format!("unknown path: {path}"))
            }
        },
    }
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

    let llm = state.ctx.llm.clone();
    let api = state.ctx.api.clone();
    let roles = state.roles.clone();

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
    let strategy_text = roles_guard
        .iter()
        .find(|r| r.name == role_name)
        .map(|r| r.strategy.as_str())
        .unwrap_or("你是一个A股短线交易AI助手。");

    let system_prompt = openatrading_web::chat::build_system_prompt(strategy_text);
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
            },
            Ok(_) => {
                empty_count += 1;
            },
            Err(e) => {
                eprintln!("[chat_stream] stream error: {e}");
                return Err(format!("stream error: {e}"));
            },
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
    let llm = state.ctx.llm.clone();

    let client = XuanguBaoClient::new();
    let events = client
        .fetch_all_day_events()
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

fn main() {
    load_env();

    let db_path = openatrading_core::atrading_home().join("data.db");

    let gateway = Gateway::init(&db_path).expect("failed to init gateway");

    let roles = build_roles();

    let app_ctx = tokio::runtime::Runtime::new().unwrap().block_on(
        openatrading_core::context::AppContext::build(
            Some("deepseek"),
            Some("deepseek-v4-flash"),
            &[],
            gateway.memory.clone(),
            gateway.trade.clone(),
            Arc::new(openatrading_core::social::NoopChannel),
        ),
    );

    let app_state = Arc::new(AppState {
        ctx: app_ctx,
        roles: Arc::new(RwLock::new(roles)),
        mcp_servers: Vec::new(),
        cache: openatrading_web::cache::new_cache(),
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
