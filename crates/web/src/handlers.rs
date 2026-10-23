//! Tauri-compatible handler wrappers.
//!
//! Each function takes `&Arc<AppState>` and returns `Result<serde_json::Value, String>`,
//! making them callable from both axum route handlers and Tauri IPC commands.
//! The axum handlers in `handlers.rs` and `chat.rs` are thin wrappers around these.

use std::sync::Arc;

use axum::http::StatusCode;

use crate::cache::{self, CachePolicy};
use crate::AppState;

// ── Cache helper ──────────────────────────────────────────────────────────

/// Tauri-compatible version of `cache::get_or_fetch_json` that returns
/// `Result<serde_json::Value, String>` instead of `Result<Json<...>, StatusCode>`.
async fn get_or_fetch_json_tauri<F, Fut, T>(
    cache: &crate::cache::AppCache,
    key: &str,
    policy: CachePolicy,
    fetch: F,
) -> Result<serde_json::Value, String>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, String>>,
    T: serde::Serialize,
{
    if let Some(cached) = cache::get(cache, key) {
        return Ok(serde_json::from_str(&cached).unwrap_or_default());
    }

    let value = fetch().await?;
    let json_string = serde_json::to_string(&value).unwrap_or_default();
    let json_value: serde_json::Value = serde_json::from_str(&json_string).unwrap_or_default();
    cache::set(cache, key, &json_string, policy);
    Ok(json_value)
}

// ── Market data handlers ─────────────────────────────────────────────────

/// GET /api/overview — market overview + risk level + decision summary
pub async fn get_overview_data(state: &Arc<AppState>) -> Result<serde_json::Value, String> {
    use openatrading_core::api::xuangubao::XuanguBaoClient;

    get_or_fetch_json_tauri(
        &state.cache,
        "overview",
        CachePolicy::overview(),
        || async {
            let client = XuanguBaoClient::new();
            let overview = client
                .fetch_market_overview()
                .await
                .map_err(|e| format!("overview error: {e}"))?;

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

            let resp = crate::web::OverviewResponse {
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
            };

            Ok(resp)
        },
    )
    .await
}

/// GET /api/limit-up
pub async fn get_limit_up_data(state: &Arc<AppState>) -> Result<serde_json::Value, String> {
    use openatrading_core::api::xuangubao::XuanguBaoClient;

    get_or_fetch_json_tauri(
        &state.cache,
        "limit_up",
        CachePolicy::realtime(),
        || async {
            let client = XuanguBaoClient::new();
            let data = client.fetch_limit_up().await.map_err(|e| e.to_string())?;
            Ok(data)
        },
    )
    .await
}

/// GET /api/limit-down
pub async fn get_limit_down_data(state: &Arc<AppState>) -> Result<serde_json::Value, String> {
    use openatrading_core::api::xuangubao::XuanguBaoClient;

    get_or_fetch_json_tauri(
        &state.cache,
        "limit_down",
        CachePolicy::realtime(),
        || async {
            let client = XuanguBaoClient::new();
            let data = client.fetch_limit_down().await.map_err(|e| e.to_string())?;
            Ok(data)
        },
    )
    .await
}

/// GET /api/super-stock
pub async fn get_super_stock_data(state: &Arc<AppState>) -> Result<serde_json::Value, String> {
    use openatrading_core::api::xuangubao::XuanguBaoClient;

    get_or_fetch_json_tauri(
        &state.cache,
        "super_stock",
        CachePolicy::realtime(),
        || async {
            let client = XuanguBaoClient::new();
            let data = client
                .fetch_super_stock()
                .await
                .map_err(|e| e.to_string())?;
            Ok(data)
        },
    )
    .await
}

/// GET /api/yesterday-limit-up
pub async fn get_yesterday_limit_up_data(
    state: &Arc<AppState>,
) -> Result<serde_json::Value, String> {
    use openatrading_core::api::xuangubao::XuanguBaoClient;

    get_or_fetch_json_tauri(
        &state.cache,
        "yesterday_limit_up",
        CachePolicy::realtime(),
        || async {
            let client = XuanguBaoClient::new();
            let data = client
                .fetch_yesterday_limit_up()
                .await
                .map_err(|e| e.to_string())?;
            Ok(data)
        },
    )
    .await
}

/// GET /api/index-quotes
pub async fn get_index_quotes_data(state: &Arc<AppState>) -> Result<serde_json::Value, String> {
    use openatrading_core::api::xuangubao::XuanguBaoClient;

    get_or_fetch_json_tauri(
        &state.cache,
        "index_quotes",
        CachePolicy::realtime(),
        || async {
            let client = XuanguBaoClient::new();
            let data = client
                .fetch_market_real(&["000001.SS", "399001.SZ", "399006.SZ"])
                .await
                .map_err(|e| e.to_string())?;
            Ok(data)
        },
    )
    .await
}

/// GET /api/dapan-abnormal
pub async fn get_dapan_abnormal_data(state: &Arc<AppState>) -> Result<serde_json::Value, String> {
    get_or_fetch_json_tauri(
        &state.cache,
        "dapan_abnormal",
        CachePolicy::realtime(),
        || async {
            let data = state
                .ctx
                .api
                .xuangubao
                .fetch_dapan_abnormal()
                .await
                .map_err(|e| e.to_string())?;
            Ok(data)
        },
    )
    .await
}

/// GET /api/timeline
pub async fn get_timeline_data(state: &Arc<AppState>) -> Result<serde_json::Value, String> {
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
        .map_err(|e| e.to_string())?;
    Ok(serde_json::to_value(data).unwrap_or_default())
}

/// GET /api/telegraph
pub async fn get_telegraph_all_data(
    state: &Arc<AppState>,
    last_time: Option<i64>,
    count: Option<u32>,
) -> Result<serde_json::Value, String> {
    let count = count.unwrap_or(20);

    if let Some(lt) = last_time {
        // Pagination mode: bypass cache, call roll API
        let data = state
            .ctx
            .api
            .cls
            .fetch_telegraph("cls", Some(lt), count)
            .await
            .map_err(|e| e.to_string())?;
        return Ok(serde_json::to_value(data).unwrap_or_default());
    }

    get_or_fetch_json_tauri(
        &state.cache,
        "telegraph_all",
        CachePolicy::telegraph(),
        || async {
            let data = state
                .ctx
                .api
                .cls
                .fetch_telegraph("", None, count)
                .await
                .map_err(|e| e.to_string())?;
            Ok(data)
        },
    )
    .await
}

/// GET /api/ai/roles
pub async fn get_ai_roles_data(state: &Arc<AppState>) -> Result<serde_json::Value, String> {
    let roles = state.roles.read().await;
    let result: Vec<serde_json::Value> = roles
        .iter()
        .map(|r| {
            serde_json::json!({
                "name": r.name,
                "display_name": r.display_name,
                "readonly": r.readonly,
            })
        })
        .collect();
    Ok(serde_json::to_value(result).unwrap_or_default())
}

/// GET /api/ai/commands
pub async fn get_ai_commands_data() -> Result<serde_json::Value, String> {
    let commands = crate::chat::get_ai_commands_inner().await;
    Ok(serde_json::to_value(commands).unwrap_or_default())
}

/// GET /api/ai/sessions
pub async fn list_sessions_data(state: &Arc<AppState>) -> Result<serde_json::Value, String> {
    let sessions = crate::chat::list_sessions_inner(state).await;
    Ok(serde_json::to_value(sessions).unwrap_or_default())
}

/// GET /api/sentiment-history
pub async fn get_sentiment_history_data(
    state: &Arc<AppState>,
) -> Result<serde_json::Value, String> {
    use openatrading_core::api::xuangubao::XuanguBaoClient;

    get_or_fetch_json_tauri(
        &state.cache,
        "sentiment_history",
        CachePolicy::realtime(),
        || async {
            let client = XuanguBaoClient::new();
            let trading_days = XuanguBaoClient::last_n_trading_days(10);
            let mut days: Vec<serde_json::Value> = Vec::with_capacity(trading_days.len());
            for date in &trading_days {
                match client.fetch_market_overview_for_date(date).await {
                    Ok(Some(d)) => {
                        let rise_ratio = f64::from(d.rise_count) / f64::from(d.fall_count.max(1));
                        days.push(serde_json::json!({
                            "date": d.date,
                            "sentiment": (d.market_temperature * 10.0).round() / 10.0,
                            "limit_up_count": d.limit_up_count,
                            "bomb_rate": (d.bomb_rate * 100.0).round() / 100.0,
                            "rise_ratio": (rise_ratio * 100.0).round() / 100.0,
                        }));
                    }
                    Ok(None) => {
                        openatrading_core::log::error!("sentiment_history: no data for {date}");
                    }
                    Err(e) => {
                        openatrading_core::log::error!("sentiment_history error for {date}: {e}");
                    }
                }
            }
            days.reverse();
            Ok(days)
        },
    )
    .await
}

/// GET /api/plate-abnormal
pub async fn get_plate_abnormal_data(
    state: &Arc<AppState>,
    date: Option<String>,
) -> Result<serde_json::Value, String> {
    use openatrading_core::api::xuangubao::XuanguBaoClient;

    let date = date.unwrap_or_else(|| {
        openatrading_core::api::utils::last_n_trading_days(1)
            .first()
            .cloned()
            .unwrap_or_default()
    });

    get_or_fetch_json_tauri(
        &state.cache,
        &format!("plate_abnormal_{date}"),
        CachePolicy::realtime(),
        || async {
            let client = XuanguBaoClient::new();
            let data = client
                .fetch_plate_abnormal_str(&date)
                .await
                .map_err(|e| e.to_string())?;
            Ok(data)
        },
    )
    .await
}

/// GET /api/field-items
pub async fn get_field_items_data(
    state: &Arc<AppState>,
    date: Option<String>,
    field_type: Option<String>,
) -> Result<serde_json::Value, String> {
    let date = date.unwrap_or_else(|| {
        openatrading_core::utils::time::beijing_now()
            .format("%Y-%m-%d")
            .to_string()
    });
    let _field_type = field_type.unwrap_or_else(|| "涨停板".to_string());

    get_or_fetch_json_tauri(
        &state.cache,
        &format!("field_items_{date}"),
        CachePolicy::realtime(),
        || async {
            // 复用 AppContext 里的 Jiuyan session：每次新建 client 都会重新登录，
            // 既慢，也可能让旧会话失效。
            let data = state
                .ctx
                .api
                .jiuyan
                .fetch_field(&date, 1)
                .await
                .map_err(|e| e.to_string())?;
            Ok(data)
        },
    )
    .await
}

/// GET /api/market-trend/:symbol
pub async fn get_market_trend_data(
    state: &Arc<AppState>,
    symbol: &str,
) -> Result<serde_json::Value, String> {
    use openatrading_core::api::xuangubao::XuanguBaoClient;

    get_or_fetch_json_tauri(
        &state.cache,
        &format!("market_trend_{symbol}"),
        CachePolicy::realtime(),
        || async {
            let client = XuanguBaoClient::new();
            let data = client
                .fetch_market_trend(symbol)
                .await
                .map_err(|e| e.to_string())?;
            Ok(data)
        },
    )
    .await
}

/// GET /api/plates-by-any-stock/:symbol
pub async fn get_plates_by_any_stock_data(
    state: &Arc<AppState>,
    symbol: &str,
) -> Result<serde_json::Value, String> {
    use openatrading_core::api::xuangubao::XuanguBaoClient;

    get_or_fetch_json_tauri(
        &state.cache,
        &format!("plates_by_any_stock_{symbol}"),
        CachePolicy::realtime(),
        || async {
            let client = XuanguBaoClient::new();
            let data = client
                .plates_by_any_stock(symbol)
                .await
                .map_err(|e| e.to_string())?;
            Ok(data)
        },
    )
    .await
}

// ── Kline ─────────────────────────────────────────────────────────────────

/// GET /api/kline/:symbol
pub async fn get_kline_data(
    state: &Arc<AppState>,
    symbol: &str,
    period: &str,
    count: u32,
) -> Result<serde_json::Value, String> {
    use openatrading_core::api::xuangubao::{KlinePeriod, XuanguBaoClient};

    let kline_period = match period {
        "week" => KlinePeriod::Week,
        "month" => KlinePeriod::Month,
        "minute" => KlinePeriod::Minute,
        _ => KlinePeriod::Day,
    };
    let cache_key = format!("kline_{symbol}_{period}_{count}");
    get_or_fetch_json_tauri(&state.cache, &cache_key, CachePolicy::daily(), || async {
        let client = XuanguBaoClient::new();
        let data = client
            .fetch_kline(symbol, kline_period, count)
            .await
            .map_err(|e| format!("kline error: {e}"))?;
        Ok(data)
    })
    .await
}

/// GET /api/realtime/:symbol
pub async fn get_realtime_data(
    state: &Arc<AppState>,
    symbol: &str,
) -> Result<serde_json::Value, String> {
    use openatrading_core::api::xuangubao::XuanguBaoClient;

    let cache_key = format!("realtime_{symbol}");
    get_or_fetch_json_tauri(
        &state.cache,
        &cache_key,
        CachePolicy::realtime(),
        || async {
            let client = XuanguBaoClient::new();
            let data = client
                .fetch_realtime(symbol)
                .await
                .map_err(|e| format!("realtime error: {e}"))?;
            Ok(data)
        },
    )
    .await
}

/// GET /api/telegraph/:category
pub async fn get_telegraph_data(
    state: &Arc<AppState>,
    category: &str,
) -> Result<serde_json::Value, String> {
    let cache_key = format!("telegraph_{category}");
    get_or_fetch_json_tauri(
        &state.cache,
        &cache_key,
        CachePolicy::telegraph(),
        || async {
            let data = state
                .ctx
                .api
                .cls
                .fetch_telegraph(category, None, 20)
                .await
                .map_err(|e| e.to_string())?;
            Ok(data)
        },
    )
    .await
}

// ── Roles ─────────────────────────────────────────────────────────────────

/// GET /api/roles
pub async fn get_roles_data(state: &Arc<AppState>) -> Result<serde_json::Value, String> {
    let roles_guard = state.roles.read().await;
    Ok(serde_json::to_value(roles_guard.clone()).unwrap_or_default())
}

/// 解析角色的初始资金；角色不存在或未声明时用默认值。
async fn initial_capital_of(state: &Arc<AppState>, name: &str) -> f64 {
    state
        .roles
        .read()
        .await
        .iter()
        .find(|r| r.name == name)
        .map_or(openatrading_core::roles::DEFAULT_INITIAL_CAPITAL, |r| {
            r.initial_capital
        })
}

/// GET /api/roles/:name/positions
pub async fn get_role_positions_data(
    state: &Arc<AppState>,
    name: &str,
) -> Result<serde_json::Value, String> {
    let (positions, _valuation_complete) = state
        .ctx
        .trade
        .positions(name)
        .await
        .map_err(|e| e.to_string())?;
    Ok(serde_json::to_value(positions).unwrap_or_default())
}

/// GET /api/roles/:name/account
pub async fn get_role_account_data(
    state: &Arc<AppState>,
    name: &str,
) -> Result<serde_json::Value, String> {
    let capital = initial_capital_of(state, name).await;
    let account = state
        .ctx
        .trade
        .account(name, capital)
        .await
        .map_err(|e| e.to_string())?;
    Ok(serde_json::to_value(account).unwrap_or_default())
}

/// GET /api/roles/:name/trades
pub async fn get_role_trades_data(
    state: &Arc<AppState>,
    name: &str,
    limit: u32,
) -> Result<serde_json::Value, String> {
    let trades = state
        .ctx
        .trade
        .trades(name, limit)
        .map_err(|e| e.to_string())?;
    Ok(serde_json::to_value(trades).unwrap_or_default())
}

/// GET /api/roles/:name/pnl-history
pub async fn get_role_pnl_history_data(
    state: &Arc<AppState>,
    name: &str,
    days: usize,
) -> Result<serde_json::Value, String> {
    let capital = initial_capital_of(state, name).await;
    let points = state
        .ctx
        .trade
        .pnl_history(name, capital, days)
        .await
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"points": points, "initial_capital": capital}))
}

/// GET /api/roles/:name/pnl-calendar?from=&to=&granularity=
///
/// `granularity` 取 `day` / `month` / `year`；`from`/`to` 为 `YYYY-MM-DD`（含当天）。
/// 缺省时返回「本月」，与前端默认维度一致。
pub async fn get_role_pnl_calendar_data(
    state: &Arc<AppState>,
    name: &str,
    from: Option<&str>,
    to: Option<&str>,
    granularity: Option<&str>,
) -> Result<serde_json::Value, (StatusCode, String)> {
    use openatrading_core::trade::{Granularity, PnlCalendarRequest};

    let today = openatrading_core::utils::time::beijing_now()
        .format("%Y-%m-%d")
        .to_string();
    let to = to.map_or(today.clone(), str::to_string);
    let from = from.map_or_else(|| month_start(&today), str::to_string);
    let granularity = granularity
        .map_or(Some(Granularity::Day), Granularity::parse)
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "granularity 只支持 day / month / year".to_string(),
            )
        })?;

    for (label, date) in [("from", &from), ("to", &to)] {
        if !is_iso_date(date) {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("{label} 需要 YYYY-MM-DD 格式，收到 {date}"),
            ));
        }
    }
    if from > to {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("起始日期 {from} 晚于结束日期 {to}"),
        ));
    }

    let capital = initial_capital_of(state, name).await;
    let calendar = state
        .ctx
        .trade
        .pnl_calendar(&PnlCalendarRequest {
            role: name.to_string(),
            initial_capital: capital,
            from,
            to,
            granularity,
        })
        .await
        .map_err(|e| {
            openatrading_core::log::error!("区间收益分析失败: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "区间收益分析失败，请查看服务端日志".to_string(),
            )
        })?;

    Ok(serde_json::json!({
        "initial_capital": capital,
        "calendar": calendar,
    }))
}

/// `2026-10-21` → `2026-10-01`
fn month_start(date: &str) -> String {
    format!("{}-01", date.get(..7).unwrap_or(date))
}

fn is_iso_date(value: &str) -> bool {
    value.len() == 10
        && value.as_bytes()[4] == b'-'
        && value.as_bytes()[7] == b'-'
        && chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok()
}

/// GET /api/roles/:name/history
pub async fn get_role_history_data(
    state: &Arc<AppState>,
    name: &str,
    limit: u32,
) -> Result<serde_json::Value, String> {
    let closed = state
        .ctx
        .trade
        .closed_trades(name, limit)
        .map_err(|e| e.to_string())?;
    Ok(serde_json::to_value(closed).unwrap_or_default())
}

// ── Replay ────────────────────────────────────────────────────────────────

/// GET /api/replay/snapshots
pub async fn get_replay_snapshots_data() -> Result<serde_json::Value, String> {
    use openatrading_core::api::xuangubao::XuanguBaoClient;
    let client = XuanguBaoClient::new();
    let events = client
        .fetch_all_day_events(0.03)
        .await
        .map_err(|e| e.to_string())?;
    let snapshots = openatrading_core::api::xuangubao::build_replay_snapshots(&events);
    Ok(serde_json::to_value(snapshots).unwrap_or_default())
}

/// GET /api/replay/index
pub async fn get_replay_index_data() -> Result<serde_json::Value, String> {
    use openatrading_core::api::xuangubao::{fetch_index_trends, XuanguBaoClient};
    let client = XuanguBaoClient::new();
    let data = fetch_index_trends(&client)
        .await
        .map_err(|e| e.to_string())?;
    Ok(serde_json::to_value(data).unwrap_or_default())
}

/// GET /api/replay/events
pub async fn get_replay_events_data(min_pcp: f64) -> Result<serde_json::Value, String> {
    use openatrading_core::api::xuangubao::{ReplayEvent, XuanguBaoClient};
    let client = XuanguBaoClient::new();
    let all_events = client
        .fetch_all_day_events(min_pcp)
        .await
        .map_err(|e| e.to_string())?;
    let events: Vec<ReplayEvent> = all_events.iter().map(ReplayEvent::from).collect();
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
    Ok(serde_json::to_value(page).unwrap_or_default())
}

/// GET /api/replay/news
pub async fn get_replay_news_data(
    tailmark: Option<i64>,
    msg_id_mark: Option<String>,
) -> Result<serde_json::Value, String> {
    use openatrading_core::api::xuangubao::XuanguBaoClient;
    let client = XuanguBaoClient::new();
    let (items, next_tail, next_msg) = client
        .fetch_news_stream(tailmark, msg_id_mark.as_deref())
        .await
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "items": items,
        "tailmark": next_tail,
        "msgIdMark": next_msg,
    }))
}

// ── Dragon Tiger ─────────────────────────────────────────────────────────

/// 单个交易日的龙虎榜个股
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DragonTigerStock {
    #[serde(flatten)]
    pub detail: openatrading_core::api::eastmoney::LhbDetail,
    /// 该股上榜的交易日（单日接口下只有一个）
    pub trade_dates: Vec<String>,
    /// 去重后的上榜原因
    pub reasons: Vec<String>,
    /// 当日参与该股的营业部（买卖席位合并）
    pub dept_names: Vec<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct DragonTigerDataset {
    items: Vec<DragonTigerStock>,
    depts: Vec<String>,
}

#[derive(Debug)]
struct DragonTigerDay {
    date: String,
    detail: openatrading_core::api::eastmoney::LhbDetail,
    reasons: Vec<String>,
}

/// `2026-10-21 00:00:00` → `2026-10-21`
fn dragon_tiger_day(value: &str) -> String {
    value.split(' ').next().unwrap_or(value).to_string()
}

/// 「连续 N 个交易日」「累计」属于跨日口径，和当日口径的净买入不是同一个数。
fn is_multi_day_reason(reason: &str) -> bool {
    reason.contains("连续") || reason.contains("累计")
}

/// 同一天同一只股票可能有多条上榜原因，金额字段可能重复。
/// 优先取当日口径的那条，避免把跨日累计金额和当日金额混在一起。
fn pick_daily_detail(
    rows: &[openatrading_core::api::eastmoney::LhbDetail],
) -> Option<&openatrading_core::api::eastmoney::LhbDetail> {
    rows.iter()
        .find(|row| {
            row.explanation
                .as_deref()
                .map_or(true, |reason| !is_multi_day_reason(reason))
        })
        .or_else(|| rows.first())
}

fn split_reasons(explanation: Option<&str>) -> impl Iterator<Item = &str> {
    explanation
        .into_iter()
        .flat_map(|text| text.split([';', '；']))
        .map(str::trim)
        .filter(|reason| !reason.is_empty())
}

fn sum_days(
    days: &[DragonTigerDay],
    get: impl Fn(&openatrading_core::api::eastmoney::LhbDetail) -> Option<f64>,
) -> Option<f64> {
    let mut total = 0.0;
    let mut found = false;
    for day in days {
        if let Some(value) = get(&day.detail) {
            total += value;
            found = true;
        }
    }
    found.then_some(total)
}

/// 把龙虎榜明细和席位合并成「一只股票一行」。
///
/// 同一只股票在同一交易日的多条上榜原因会去重合并；金额只取当日口径，
/// 避免同日重复行把净买入翻倍。
fn build_dragon_tiger_dataset(
    details: Vec<openatrading_core::api::eastmoney::LhbDetail>,
    seats: Vec<openatrading_core::api::eastmoney::LhbSeatRecord>,
) -> DragonTigerDataset {
    use std::collections::{BTreeMap, BTreeSet, HashMap};

    // (code, date) → 参与该股的营业部
    let mut seats_by_day: HashMap<String, HashMap<String, BTreeSet<String>>> = HashMap::new();
    for seat in seats {
        if seat.dept_name.is_empty() {
            continue;
        }
        seats_by_day
            .entry(seat.code)
            .or_default()
            .entry(seat.trade_date)
            .or_default()
            .insert(seat.dept_name);
    }

    // 先按 (code, date) 归组，再按 code 合并
    let mut by_day: BTreeMap<(String, String), Vec<_>> = BTreeMap::new();
    for item in details {
        let date = dragon_tiger_day(&item.trade_date);
        by_day
            .entry((item.code.clone(), date))
            .or_default()
            .push(item);
    }

    let mut by_stock: BTreeMap<String, Vec<DragonTigerDay>> = BTreeMap::new();
    for ((code, date), rows) in by_day {
        let Some(canonical) = pick_daily_detail(&rows) else {
            continue;
        };
        let mut reasons: Vec<String> = Vec::new();
        for row in &rows {
            for reason in split_reasons(row.explanation.as_deref()) {
                if !reasons.iter().any(|existing| existing == reason) {
                    reasons.push(reason.to_string());
                }
            }
        }
        by_stock.entry(code).or_default().push(DragonTigerDay {
            date,
            detail: canonical.clone(),
            reasons,
        });
    }

    let mut items: Vec<DragonTigerStock> = by_stock
        .into_iter()
        .filter_map(|(code, mut days)| {
            days.sort_by(|a, b| b.date.cmp(&a.date));
            let latest = days.first()?;
            let mut detail = latest.detail.clone();
            detail.trade_date.clone_from(&latest.date);
            detail.billboard_net_amt = sum_days(&days, |d| d.billboard_net_amt);
            detail.billboard_buy_amt = sum_days(&days, |d| d.billboard_buy_amt);
            detail.billboard_sell_amt = sum_days(&days, |d| d.billboard_sell_amt);
            detail.billboard_deal_amt = sum_days(&days, |d| d.billboard_deal_amt);

            let mut reasons: Vec<String> = Vec::new();
            let mut dept_names: BTreeSet<String> = BTreeSet::new();
            for day in &days {
                for reason in &day.reasons {
                    if !reasons.contains(reason) {
                        reasons.push(reason.clone());
                    }
                }
                if let Some(depts) = seats_by_day.get(&code).and_then(|m| m.get(&day.date)) {
                    dept_names.extend(depts.iter().cloned());
                }
            }
            detail.explanation = if reasons.is_empty() {
                None
            } else {
                Some(reasons.join("；"))
            };

            Some(DragonTigerStock {
                detail,
                trade_dates: days.iter().map(|day| day.date.clone()).collect(),
                reasons,
                dept_names: dept_names.into_iter().collect(),
            })
        })
        .collect();

    items.sort_by(|a, b| {
        b.detail
            .trade_date
            .cmp(&a.detail.trade_date)
            .then_with(|| {
                b.detail
                    .billboard_net_amt
                    .unwrap_or(0.0)
                    .total_cmp(&a.detail.billboard_net_amt.unwrap_or(0.0))
            })
            .then_with(|| a.detail.code.cmp(&b.detail.code))
    });

    let mut dept_counts: HashMap<String, usize> = HashMap::new();
    for item in &items {
        for dept in &item.dept_names {
            *dept_counts.entry(dept.clone()).or_default() += 1;
        }
    }
    let mut depts: Vec<String> = dept_counts.keys().cloned().collect();
    depts.sort_by(|a, b| {
        dept_counts
            .get(b.as_str())
            .copied()
            .unwrap_or(0)
            .cmp(&dept_counts.get(a.as_str()).copied().unwrap_or(0))
            .then_with(|| a.cmp(b))
    });

    DragonTigerDataset { items, depts }
}

async fn fetch_dragon_tiger_dataset(
    state: &Arc<AppState>,
    date: &str,
) -> Result<DragonTigerDataset, String> {
    use openatrading_core::api::eastmoney::SeatFlag;

    let client = &state.ctx.api.eastmoney;

    let (details, buy_seats, sell_seats) = tokio::try_join!(
        client.lhb_detail(date, date),
        client.lhb_seats_range(date, date, SeatFlag::Buy),
        client.lhb_seats_range(date, date, SeatFlag::Sell),
    )
    .map_err(|e| e.to_string())?;

    let mut seats = buy_seats;
    seats.extend(sell_seats);
    Ok(build_dragon_tiger_dataset(details, seats))
}

/// GET /api/dragon-tiger/detail?date=YYYY-MM-DD
///
/// 只返回指定交易日的龙虎榜数据；不传 `date` 时使用最近一个交易日。
pub async fn get_dragon_tiger_detail_data(
    state: &Arc<AppState>,
    date: Option<&str>,
) -> Result<serde_json::Value, String> {
    use openatrading_core::api::utils::{compact_date, last_n_trading_days};

    let latest = last_n_trading_days(1);
    let latest = latest
        .first()
        .cloned()
        .ok_or_else(|| "no trading days".to_string())?;
    let date = match date.map(str::trim).filter(|date| !date.is_empty()) {
        Some(date) => compact_date(date).map_err(|e| e.to_string())?,
        None => latest.clone(),
    };

    // 当日龙虎榜在收盘后才会陆续发布：短 TTL 避免盘中把空结果缓存 6 小时。
    // 历史日期数据稳定，沿用 daily 缓存。
    let policy = if date == latest {
        CachePolicy::Always(std::time::Duration::from_secs(60))
    } else {
        CachePolicy::daily()
    };

    let cache_key = format!("dragon_tiger_dataset_v3_{date}");
    let cached = get_or_fetch_json_tauri(&state.cache, &cache_key, policy, || {
        fetch_dragon_tiger_dataset(state, &date)
    })
    .await?;
    let dataset: DragonTigerDataset = serde_json::from_value(cached)
        .map_err(|e| format!("dragon tiger cache decode error: {e}"))?;

    let date_label = format!("{}-{}-{}", &date[..4], &date[4..6], &date[6..]);
    Ok(serde_json::json!({
        "date": date_label,
        "data": dataset.items,
        "depts": dataset.depts,
    }))
}

/// GET /api/dragon-tiger/stock-seat
pub async fn get_dragon_tiger_stock_seat_data(
    state: &Arc<AppState>,
    code: &str,
    date: &str,
    flag: &str,
) -> Result<serde_json::Value, String> {
    use openatrading_core::api::eastmoney::SeatFlag;
    let seat_flag = match flag {
        "buy" => SeatFlag::Buy,
        "sell" => SeatFlag::Sell,
        _ => return Err("flag must be 'buy' or 'sell'".to_string()),
    };
    let seats = state
        .ctx
        .api
        .eastmoney
        .lhb_stock_seat(code, date, seat_flag)
        .await
        .map_err(|e| e.to_string())?;
    Ok(serde_json::to_value(seats).unwrap_or_default())
}

// ── Search ────────────────────────────────────────────────────────────────

/// GET /api/search-stocks?q=keyword
pub async fn search_stocks_data(q: &str) -> Result<serde_json::Value, String> {
    use openatrading_core::api::eastmoney::LhbClient;
    if q.is_empty() {
        return Err("missing q".to_string());
    }
    let client = LhbClient::new();
    let items = client.search_stocks(q).await.map_err(|e| e.to_string())?;
    Ok(serde_json::to_value(items).unwrap_or_default())
}

// ── Watchlist ────────────────────────────────────────────────────────────

fn watchlist_path() -> std::path::PathBuf {
    openatrading_core::atrading_home().join("watchlist.json")
}

/// 把自选股代码统一为 Yahoo 格式（`XXXXXX.SS/.SZ/.BJ`）。
///
/// 前端各入口传入的 code 可能是裸数字（`600519`）、带交易所前缀（`sh600519`）
/// 或已是 Yahoo 格式（`600519.SS`），不统一会导致同一股票重复出现在自选股里。
/// 归一化失败（如非 A 股代码）时回退原始输入，保持容错。
fn normalize_watchlist_code(raw: &str) -> String {
    openatrading_core::api::utils::to_yahoo_symbol(raw).unwrap_or_else(|_| raw.trim().to_string())
}

fn load_watchlist() -> Vec<serde_json::Value> {
    let path = watchlist_path();
    if !path.exists() {
        return Vec::new();
    }
    let items: Vec<serde_json::Value> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let (items, changed) = normalize_watchlist_items(items);
    if changed {
        // 迁移落盘为尽力而为：失败不影响本次读取
        if let Err(e) = save_watchlist(&items) {
            openatrading_core::log::warn!("watchlist migration save failed: {e}");
        }
    }
    items
}

/// 迁移自选股条目：统一为 Yahoo 格式并按规范后的 code 去重。
///
/// 返回 `(去重后的条目, 是否有改动)`。同一股票以多种格式（如 `600519` 与
/// `600519.SS`）同时存在时只保留先添加的一条。
fn normalize_watchlist_items(mut items: Vec<serde_json::Value>) -> (Vec<serde_json::Value>, bool) {
    let mut seen = std::collections::HashSet::new();
    let mut changed = false;
    items.retain_mut(|item| {
        let Some(raw_code) = item.get("code").and_then(|v| v.as_str()) else {
            return true;
        };
        let canonical = normalize_watchlist_code(raw_code);
        if canonical != raw_code {
            item["code"] = serde_json::Value::String(canonical.clone());
            changed = true;
        }
        seen.insert(canonical)
    });
    (items, changed)
}

fn save_watchlist(items: &[serde_json::Value]) -> Result<(), String> {
    let path = watchlist_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(
        &path,
        serde_json::to_string_pretty(items).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

/// GET /api/watchlist
pub async fn get_watchlist_data() -> Result<serde_json::Value, String> {
    let items = load_watchlist();
    Ok(serde_json::to_value(items).unwrap_or_default())
}

/// POST /api/watchlist
pub async fn add_watchlist_data(code: &str, name: &str) -> Result<serde_json::Value, String> {
    let canonical_code = normalize_watchlist_code(code);
    let mut items = load_watchlist();
    if items
        .iter()
        .any(|i| i.get("code").and_then(|v| v.as_str()) == Some(canonical_code.as_str()))
    {
        return Err("already exists".to_string());
    }
    let now = chrono::Utc::now().to_rfc3339();
    items.push(serde_json::json!({
        "code": canonical_code,
        "name": name,
        "added_at": now,
    }));
    save_watchlist(&items)?;
    Ok(serde_json::to_value(items).unwrap_or_default())
}

/// DELETE /api/watchlist/:code
pub async fn delete_watchlist_data(code: &str) -> Result<serde_json::Value, String> {
    let canonical_code = normalize_watchlist_code(code);
    let mut items = load_watchlist();
    let before = items.len();
    items.retain(|i| i.get("code").and_then(|v| v.as_str()) != Some(canonical_code.as_str()));
    if items.len() == before {
        return Err(format!("not found: {code}"));
    }
    save_watchlist(&items)?;
    Ok(serde_json::to_value(items).unwrap_or_default())
}

// ── Role CRUD ────────────────────────────────────────────────────────────

/// POST /api/roles — create a new role
///
/// 角色名是账户主键，已存在时直接报错而不是静默更新：否则前端把两个
/// 不同的显示名归一到同一个 slug 时，会悄悄合并成同一个账户。
pub async fn create_role_data(
    state: &Arc<AppState>,
    name: &str,
    display_name: &str,
    initial_capital: Option<f64>,
) -> Result<serde_json::Value, String> {
    let exists_in_state = {
        let roles = state.roles.read().await;
        roles.iter().any(|r| r.name == name)
    };
    let exists = exists_in_state
        || openatrading_core::roles::list_roles_from_disk()
            .iter()
            .any(|r| r.name == name);
    if exists {
        return Err(format!("角色已存在：{name}，请换一个名称"));
    }

    let capital = initial_capital.unwrap_or(openatrading_core::roles::DEFAULT_INITIAL_CAPITAL);
    if !capital.is_finite() || capital <= 0.0 {
        return Err("初始资金必须是正数".to_string());
    }

    // 名字可能来自一次「删除失败但文件已删」的旧角色：先清掉同名残留流水，
    // 保证新建账户不会继承旧数据。
    state
        .ctx
        .trade
        .reset(name)
        .map_err(|e| format!("清理同名旧交易数据失败: {e}"))?;
    openatrading_core::roles::save_role(&openatrading_core::roles::RoleSave {
        name,
        display_name,
        initial_capital: capital,
    })
    .map_err(|e| format!("保存角色失败: {e}"))?;

    let mut roles = state.roles.write().await;
    roles.push(crate::RoleEntry {
        name: name.to_string(),
        display_name: display_name.to_string(),
        readonly: false,
        initial_capital: capital,
    });
    Ok(serde_json::json!({"ok": true}))
}

/// DELETE /api/roles/:name — delete role and all its data
pub async fn delete_role_data(
    state: &Arc<AppState>,
    name: &str,
) -> Result<serde_json::Value, String> {
    // Reject deletion of config-toml readonly roles
    {
        let roles = state.roles.read().await;
        if let Some(role) = roles.iter().find(|r| r.name == name) {
            if role.readonly {
                return Err("角色为只读，不能删除".to_string());
            }
        }
    }

    // 先删角色文件再看交易库：文件删成功后即使清流水失败，也不会在重启后
    // 以「删除了但没删干净」的状态复活；失败时保留内存角色让用户可以重试。
    openatrading_core::roles::delete_role(name).map_err(|e| format!("删除角色文件失败: {e}"))?;
    state
        .ctx
        .trade
        .reset(name)
        .map_err(|e| format!("清空交易数据失败: {e}"))?;
    {
        let mut roles = state.roles.write().await;
        roles.retain(|r| r.name != name);
    }
    Ok(serde_json::json!({"ok": true}))
}

/// POST /api/roles/:name/reset — 清空该角色的持仓与流水
pub async fn reset_role_data(
    state: &Arc<AppState>,
    name: &str,
) -> Result<serde_json::Value, String> {
    state
        .ctx
        .trade
        .reset(name)
        .map_err(|e| format!("重置交易数据失败: {e}"))?;
    Ok(serde_json::json!({"ok": true}))
}

/// POST /api/roles/:name/orders — 手动下单
///
/// 返回 [`openatrading_core::trade::OrderError`] 而非 `String`，让 web 层能按
/// `is_client_error()` 区分 400（输入有误，原因要展示给用户）与 500。
pub async fn place_order_data(
    state: &Arc<AppState>,
    name: &str,
    body: serde_json::Value,
) -> Result<serde_json::Value, openatrading_core::trade::OrderError> {
    let request: openatrading_core::trade::OrderRequest = serde_json::from_value(body)
        .map_err(|e| openatrading_core::trade::OrderError::InvalidSymbol(e.to_string()))?;
    let capital = initial_capital_of(state, name).await;
    let outcome = state.ctx.trade.place_order(name, capital, &request).await?;
    Ok(serde_json::json!({
        "trade": outcome.trade,
        "position": outcome.position,
    }))
}

// ── Session CRUD ─────────────────────────────────────────────────────────

/// GET /api/ai/sessions/:id/messages — read messages from a session.
///
/// Thin Tauri-side wrapper that delegates to the shared history reader
/// (`chat::read_messages_from_disk` + `chat::messages_to_session_messages`),
/// so the web and Tauri paths produce identical history shapes and neither
/// materializes an `AgentSessionRuntime` in the LRU pool just to read past
/// messages. Tool results are joined onto their `ToolUse` block here too.
pub async fn get_session_messages_data(
    _state: &Arc<AppState>,
    id: &str,
) -> Result<serde_json::Value, String> {
    use crate::chat;
    use openatrading_core::log;

    // File parsing is synchronous; offload it so we don't stall the executor.
    let id = id.to_string();
    let messages =
        match tokio::task::spawn_blocking(move || chat::read_messages_from_disk(&id)).await {
            Ok(m) => m,
            Err(join_err) => {
                log::error!("get_session_messages_data: read task failed: {join_err}");
                Vec::new()
            }
        };
    let session_messages = chat::messages_to_session_messages(&messages);
    Ok(serde_json::to_value(session_messages).unwrap_or_default())
}

/// POST /api/ai/sessions — create a new session
pub async fn create_session_data(
    state: &Arc<AppState>,
    _name: Option<&str>,
    session_id: Option<&str>,
    _message: &str,
) -> Result<serde_json::Value, String> {
    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let sid = state
        .ctx
        .pool
        .create_session_with_id(&cwd, session_id)
        .await
        .map_err(|e| e.to_string())?;

    // Ask the actor to flush the session JSONL to disk so list_sessions can
    // find it, and return its metadata.
    let (session_file, session_name) = match state.ctx.pool.get_session(&sid).await {
        Some(handle) => match handle.flush_to_disk().await {
            Ok((file, name)) => (file, name),
            Err(e) => {
                openatrading_core::log::error!("create_session_data: flush failed: {e}");
                (None, None)
            }
        },
        None => (None, None),
    };

    Ok(serde_json::json!({
        "success": true,
        "id": sid,
        "name": session_name,
        "path": session_file.map(|p| p.to_string_lossy().to_string()),
        "message_count": 0,
    }))
}

/// DELETE /api/ai/sessions/:id — delete a saved session
pub async fn delete_session_data(
    state: &Arc<AppState>,
    id: &str,
) -> Result<serde_json::Value, String> {
    // Remove the live facade from the shared pool so the agent stops.
    state.ctx.pool.close_session(id).await;

    let path = crate::chat::find_session_file(id).ok_or_else(|| "not found".to_string())?;
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        Ok(serde_json::json!({"ok": true}))
    } else {
        Err("not found".to_string())
    }
}

/// PATCH /api/ai/sessions/:id — rename a session
pub async fn rename_session_data(id: &str, new_name: &str) -> Result<serde_json::Value, String> {
    let path = crate::chat::find_session_file(id).ok_or_else(|| "not found".to_string())?;
    if !path.exists() {
        return Err("not found".to_string());
    }

    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return Err("empty session file".to_string());
    }

    let mut header: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(lines[0]).map_err(|e| e.to_string())?;
    header.insert(
        "name".to_string(),
        serde_json::Value::String(new_name.to_string()),
    );

    let new_header = serde_json::to_string(&header).map_err(|e| e.to_string())?;
    lines[0] = &new_header;

    std::fs::write(&path, lines.join("\n")).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"ok": true}))
}

/// GET /api/ai/sessions/:id/tool-results/:tool_call_id — fetch full tool result content
pub async fn get_tool_result_data(
    session_id: &str,
    tool_call_id: &str,
) -> Result<serde_json::Value, String> {
    let path = crate::chat::find_session_file(session_id).ok_or_else(|| "not found".to_string())?;
    let content = std::fs::read_to_string(&path).map_err(|_| "not found".to_string())?;

    for line in content.lines().skip(1) {
        if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
            if entry.get("type").and_then(|v| v.as_str()) != Some("message") {
                continue;
            }
            if let Some(msg) = entry.get("message") {
                if msg.get("role").and_then(|v| v.as_str()) != Some("toolResult") {
                    continue;
                }
                if msg.get("tool_call_id").and_then(|v| v.as_str()) != Some(tool_call_id) {
                    continue;
                }
                let text: String = msg
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
                return Ok(serde_json::json!({
                    "tool_call_id": tool_call_id,
                    "content": text,
                }));
            }
        }
    }

    Err("not found".to_string())
}

// ── Skills (Tauri-compatible wrappers) ────────────────────────────────────

pub fn get_skills_data() -> Vec<openatrading_core::skills::SkillEntry> {
    openatrading_core::skills::list_skill_entries()
}

pub fn save_skill_data(
    req: openatrading_core::skills::SkillRequest,
) -> Result<openatrading_core::skills::SkillEntry, String> {
    openatrading_core::skills::validate_skill_name(&req.name)?;
    openatrading_core::skills::install_skill(&req)
        .map_err(|e| format!("failed to save skill: {e}"))?;
    openatrading_core::skills::list_skill_entries()
        .into_iter()
        .find(|s| s.name == req.name)
        .ok_or_else(|| "skill not found after save".to_string())
}

pub fn delete_skill_data(name: &str) -> Result<(), String> {
    openatrading_core::skills::uninstall_skill(name)
        .map_err(|e| format!("failed to delete skill '{name}': {e}"))
}

// ── pi-rs agent settings (default provider / model / thinking level) ─────

fn pi_settings_manager() -> pi_coding_agent::core::settings_manager::SettingsManager {
    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    pi_coding_agent::core::settings_manager::SettingsManager::create(&cwd, None)
}

/// Read pi-rs `settings.json` defaults: `defaultProvider` / `defaultModel` /
/// `thinkingLevel`.
pub fn get_pi_agent_settings_data() -> Result<serde_json::Value, String> {
    let mgr = pi_settings_manager();
    Ok(serde_json::json!({
        "defaultProvider": mgr.get_default_provider(),
        "defaultModel": mgr.get_default_model(),
        "thinkingLevel": mgr.get_default_thinking_level(),
    }))
}

/// Update pi-rs `settings.json` defaults. Only non-empty fields are written.
pub fn update_pi_agent_settings_data(
    provider: Option<&str>,
    model: Option<&str>,
    thinking_level: Option<&str>,
) -> Result<(), String> {
    let mut mgr = pi_settings_manager();
    if let Some(p) = provider.filter(|s| !s.is_empty()) {
        mgr.set_default_provider(p);
    }
    if let Some(m) = model.filter(|s| !s.is_empty()) {
        mgr.set_default_model(m);
    }
    if let Some(t) = thinking_level.filter(|s| !s.is_empty()) {
        mgr.set_default_thinking_level(t);
    }
    Ok(())
}

/// GET /api/agent/models — pi-rs 可选模型列表（按 provider 分组）。
///
/// 复用 pi-coding-agent 的 `ModelRegistry`（与会话工厂同一来源），返回
/// provider + 模型元数据，供前端 ModelPicker 选择默认 provider/model。
pub fn get_pi_agent_models_data() -> Result<serde_json::Value, String> {
    // `wired_model_registry` 内部已 `wire_auth_resolver`——少了这一步，
    // `has_configured_auth` 看不到 auth.json 里存的 key，模型列表会把已配置的
    // provider 整片判成未配置（全部置灰）。
    let registry = openatrading_core::llm::wired_model_registry();

    let mut providers: Vec<serde_json::Value> = Vec::new();
    for provider in registry.get_providers() {
        let mut models: Vec<serde_json::Value> = registry
            .get_models_for_provider(&provider)
            .into_iter()
            .map(|m| {
                serde_json::json!({
                    "id": m.id,
                    "name": m.name,
                    "reasoning": m.reasoning,
                    "contextWindow": m.context_window,
                    "maxTokens": m.max_tokens,
                    "available": registry.has_configured_auth(&m),
                })
            })
            .collect();
        models.sort_by(|a, b| {
            a.get("id")
                .and_then(|v| v.as_str())
                .cmp(&b.get("id").and_then(|v| v.as_str()))
        });
        providers.push(serde_json::json!({ "name": provider, "models": models }));
    }
    Ok(serde_json::json!({ "providers": providers }))
}

// ── Provider API Key（pi-rs `auth.json`）──────────────────────────────────

/// GET /api/agent/auth — 已知 provider 及其凭据状态。
///
/// 只返回状态（`source` / `label`），**不回显密钥原文**。
pub fn get_pi_agent_auth_data() -> Result<serde_json::Value, String> {
    let store = openatrading_core::llm::AgentAuthStore::from_pi_rs();
    let providers = serde_json::to_value(store.providers())
        .map_err(|e| format!("序列化 provider 凭据状态失败: {e}"))?;
    Ok(serde_json::json!({ "providers": providers }))
}

/// POST /api/agent/auth — 写入某个 provider 的 API Key。
pub fn set_pi_agent_api_key_data(
    provider: &str,
    key: &str,
) -> Result<(), openatrading_core::llm::AgentAuthError> {
    openatrading_core::llm::AgentAuthStore::from_pi_rs().set_api_key(provider, key)
}

/// DELETE /api/agent/auth/{provider} — 删除某个 provider 已保存的 API Key。
pub fn delete_pi_agent_api_key_data(
    provider: &str,
) -> Result<(), openatrading_core::llm::AgentAuthError> {
    openatrading_core::llm::AgentAuthStore::from_pi_rs().delete_api_key(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(code: &str, name: &str) -> serde_json::Value {
        serde_json::json!({ "code": code, "name": name, "added_at": "2026-01-01T00:00:00+00:00" })
    }

    fn codes(items: &[serde_json::Value]) -> Vec<String> {
        items
            .iter()
            .filter_map(|i| i.get("code").and_then(|v| v.as_str()).map(str::to_string))
            .collect()
    }

    #[test]
    fn normalize_watchlist_items_unifies_to_yahoo_and_dedups() {
        // 同一股票以裸数字、交易所前缀、Yahoo 格式三种形式出现，只保留先添加的一条
        let items = vec![
            item("603773", "沃格光电"),
            item("sh600721", "百花医药"),
            item("600519.SS", "贵州茅台"),
            item("603773.SS", "沃格光电(重复)"),
        ];
        let (items, changed) = normalize_watchlist_items(items);
        assert!(changed, "duplicates must be detected");
        assert_eq!(
            codes(&items),
            vec!["603773.SS", "600721.SS", "600519.SS"],
            "格式应统一为 Yahoo 且按 code 去重"
        );
    }

    #[test]
    fn normalize_watchlist_items_keeps_canonical_entries_unchanged() {
        let items = vec![item("600519.SS", "贵州茅台"), item("300308.SZ", "中际旭创")];
        let (items, changed) = normalize_watchlist_items(items);
        assert!(!changed);
        assert_eq!(codes(&items), vec!["600519.SS", "300308.SZ"]);
    }

    #[test]
    fn normalize_watchlist_code_falls_back_on_non_ashare() {
        // 非 A 股 / 无法推断交易所的输入回退原样，不破坏已有条目
        assert_eq!(normalize_watchlist_code("AAPL"), "AAPL");
        assert_eq!(normalize_watchlist_code("600000.SS"), "600000.SS");
    }

    fn lhb_detail(
        code: &str,
        date: &str,
        reason: &str,
        net: f64,
    ) -> openatrading_core::api::eastmoney::LhbDetail {
        openatrading_core::api::eastmoney::LhbDetail {
            code: code.into(),
            name: "测试股".into(),
            trade_date: format!("{date} 00:00:00"),
            close_price: Some(10.0),
            change_rate: Some(1.0),
            billboard_net_amt: Some(net),
            billboard_buy_amt: Some(net + 10.0),
            billboard_sell_amt: Some(10.0),
            billboard_deal_amt: Some(net.abs() + 20.0),
            accum_amount: Some(1000.0),
            turnover_rate: Some(1.0),
            free_market_cap: Some(100.0),
            explanation: Some(reason.into()),
            d1_chg: None,
            d2_chg: None,
            d5_chg: None,
            d10_chg: None,
        }
    }

    fn lhb_seat(
        code: &str,
        date: &str,
        dept: &str,
    ) -> openatrading_core::api::eastmoney::LhbSeatRecord {
        openatrading_core::api::eastmoney::LhbSeatRecord {
            code: code.into(),
            trade_date: date.into(),
            dept_name: dept.into(),
        }
    }

    #[test]
    fn dragon_tiger_merges_same_day_reasons_and_amounts() {
        let details = vec![
            // 同一只股票同一天多条原因：金额取当日口径，不能把多行金额相加
            lhb_detail(
                "600241",
                "2026-10-21",
                "连续三个交易日内，涨幅偏离值累计达到20%的证券",
                999.0,
            ),
            lhb_detail("600241", "2026-10-21", "日换手率达到20%的前5只证券", 50.0),
            lhb_detail(
                "600241",
                "2026-10-21",
                "日涨幅偏离值达到7%的前5只证券",
                50.0,
            ),
            lhb_detail(
                "000001",
                "2026-10-21",
                "日涨幅偏离值达到7%的前5只证券",
                -30.0,
            ),
        ];
        let seats = vec![
            lhb_seat("600241", "2026-10-21", "华鑫证券上海分公司"),
            lhb_seat("600241", "2026-10-21", "机构专用"),
            lhb_seat("000001", "2026-10-21", "机构专用"),
        ];

        let dataset = build_dragon_tiger_dataset(details, seats);
        assert_eq!(dataset.items.len(), 2, "同一只股票应合并成一行");

        let item = dataset
            .items
            .iter()
            .find(|item| item.detail.code == "600241")
            .unwrap_or_else(|| panic!("600241 should exist"));
        assert_eq!(item.trade_dates, vec!["2026-10-21"]);
        assert_eq!(item.detail.trade_date, "2026-10-21");
        assert_eq!(
            item.detail.billboard_net_amt,
            Some(50.0),
            "同日重复行不能把净买入相加，且不能取跨日累计口径的金额"
        );
        assert_eq!(item.reasons.len(), 3, "重复原因需要去重");
        assert!(item
            .reasons
            .contains(&"日涨幅偏离值达到7%的前5只证券".to_string()));
        assert_eq!(
            item.dept_names,
            vec!["华鑫证券上海分公司".to_string(), "机构专用".to_string()],
            "买卖席位里的营业部应合并到个股上"
        );

        assert_eq!(dataset.depts.first().map(String::as_str), Some("机构专用"));
    }

    #[test]
    fn pi_agent_models_returns_providers_with_models() {
        // pi-rs 内置模型数据在编译期嵌入，注册表必然非空
        let data = get_pi_agent_models_data().unwrap_or_else(|e| panic!("models data: {e}"));
        let providers = data["providers"]
            .as_array()
            .unwrap_or_else(|| panic!("providers array"));
        assert!(!providers.is_empty(), "至少有一个 provider");
        let first = &providers[0];
        assert!(first["name"].is_string());
        let models = first["models"]
            .as_array()
            .unwrap_or_else(|| panic!("models array"));
        assert!(!models.is_empty(), "每个 provider 至少有一个模型");
        let model = &models[0];
        assert!(model["id"].is_string());
        assert!(model["name"].is_string());
        assert!(model["available"].is_boolean());
    }
}
