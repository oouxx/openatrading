use async_stream::stream;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive},
        IntoResponse, Sse,
    },
    Json,
};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

use crate::AppState;

use crate::cache::{self, CachePolicy};
use crate::market_stream::{FetchFn, StreamSpec};
use openatrading_core::api::eastmoney::LhbDetail;
use openatrading_core::api::utils::last_n_trading_days;
use openatrading_core::api::utils::to_yahoo_symbol;
use openatrading_core::api::xuangubao::{KlinePeriod, XuanguBaoClient};

// ── Response types ────────────────────────────────────

#[derive(Serialize)]
pub struct OverviewResponse {
    pub date: String,
    pub rise_count: u32,
    pub fall_count: u32,
    pub limit_up_count: u32,
    pub limit_down_count: u32,
    pub bomb_rate: f64,
    pub risk_level: String,
    pub decision_summary: String,
    pub market_temperature: f64,
    pub yesterday_limit_up_avg_pcp: f64,
}

#[derive(Serialize)]
pub struct SentimentHistoryDay {
    pub date: String,
    pub sentiment: f64,
    pub limit_up_count: u32,
    pub bomb_rate: f64,
    pub rise_ratio: f64,
}

// ── Role CRUD types ────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateRoleRequest {
    pub name: String,
    pub display_name: String,
    /// 模拟盘初始资金；省略时沿用已有值或默认值。
    #[serde(default)]
    pub initial_capital: Option<f64>,
}

#[derive(Serialize)]
pub struct PnlHistoryResponse {
    pub points: Vec<openatrading_core::trade::PnlPoint>,
    pub initial_capital: f64,
}

// ── Handlers ──────────────────────────────────────────

/// GET /api/overview — market overview + risk level + decision summary
pub async fn get_overview(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::handlers::get_overview_data(&state)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /api/sentiment-history
pub async fn get_sentiment_history(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::handlers::get_sentiment_history_data(&state)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /api/limit-up
pub async fn get_limit_up(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::handlers::get_limit_up_data(&state)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /api/limit-down
pub async fn get_limit_down(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::handlers::get_limit_down_data(&state)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /api/plate-abnormal?count=30
#[derive(serde::Deserialize)]
pub struct PlateAbnormalQuery {
    #[serde(default = "default_plate_count")]
    count: i32,
}
const fn default_plate_count() -> i32 {
    30
}

pub async fn get_plate_abnormal(
    State(state): State<Arc<AppState>>,
    Query(q): Query<PlateAbnormalQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let date = q.count.to_string();
    crate::handlers::get_plate_abnormal_data(&state, Some(date))
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /api/field-items?date=YYYY-MM-DD
#[derive(serde::Deserialize)]
pub struct FieldItemsQuery {
    date: Option<String>,
}

pub async fn get_field_items(
    State(state): State<Arc<AppState>>,
    Query(q): Query<FieldItemsQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    if std::env::var("JIUYAN_PHONE").is_err() {
        return Ok(Json(serde_json::json!([])));
    }
    crate::handlers::get_field_items_data(&state, q.date, None)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /api/timeline
pub async fn get_timeline(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    if std::env::var("JIUYAN_PHONE").is_err() {
        return Ok(Json(serde_json::json!([])));
    }
    crate::handlers::get_timeline_data(&state)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /api/kline/stream — SSE, every 3s, comma-separated symbols
#[derive(serde::Deserialize)]
pub struct KlineStreamQuery {
    #[serde(default = "default_kline_period")]
    period: String,
    #[serde(default = "default_kline_count")]
    count: u32,
    /// Comma-separated stock symbols
    symbols: String,
}

/// Build an SSE body from a hub subscription.
///
/// Replays `last` (if any) so a fresh subscriber gets immediate data, then
/// forwards live broadcast messages. The first live message equal to `last`
/// is skipped — the race between reading `last` and subscribing can otherwise
/// duplicate the most recent payload.
fn market_sse(
    rx: broadcast::Receiver<String>,
    last: Option<String>,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    let s = stream! {
        let mut rx = rx;
        let mut skip = last.clone();
        if let Some(json) = last {
            yield Ok::<_, std::convert::Infallible>(Event::default().data(json));
        }
        loop {
            match rx.recv().await {
                Ok(json) => {
                    if let Some(prev) = skip.take() {
                        if prev == json {
                            continue;
                        }
                    }
                    yield Ok::<_, std::convert::Infallible>(Event::default().data(json));
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    };
    Sse::new(s).keep_alive(KeepAlive::new().interval(Duration::from_secs(1)))
}

pub async fn get_kline_stream(
    State(state): State<Arc<AppState>>,
    Query(q): Query<KlineStreamQuery>,
) -> Result<Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>>, StatusCode> {
    let period = match q.period.as_str() {
        "week" => KlinePeriod::Week,
        "month" => KlinePeriod::Month,
        "minute" => KlinePeriod::Minute,
        _ => KlinePeriod::Day,
    };
    let period_str = q.period.clone();
    let count = q.count;
    let symbols: Vec<String> = q
        .symbols
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if symbols.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // One shared upstream poll per symbol set, fanned out to every SSE
    // connection via the hub. The fetch closure also repopulates the
    // per-symbol kline cache, so the single-shot `/api/kline/:symbol`
    // endpoint reads stream-fresh data instead of fetching upstream itself.
    let cache = Arc::clone(&state.cache);
    let client = Arc::new(XuanguBaoClient::new());
    let fetch: FetchFn = Arc::new(move |syms: &[String]| {
        let client = Arc::clone(&client);
        let cache = Arc::clone(&cache);
        let period_str = period_str.clone();
        let syms: Vec<String> = syms.to_vec();
        Box::pin(async move {
            let data = client
                .fetch_kline_batch(&syms, period, count)
                .await
                .map_err(|e| e.to_string())?;
            for sym in &syms {
                if let Ok(ykey) = to_yahoo_symbol(sym) {
                    if let Some(kline) = data.get(&ykey) {
                        if let Ok(serialized) = serde_json::to_string(kline) {
                            let key = format!("kline_{sym}_{period_str}_{count}");
                            cache::set(&cache, &key, &serialized, CachePolicy::daily());
                        }
                    }
                }
            }
            serde_json::to_string(&data).map_err(|e| e.to_string())
        })
    });

    let spec = StreamSpec {
        key: format!("kline:{}:{}", period.as_str(), symbols.join(",")),
        trading_interval: Duration::from_secs(3),
        nontrading_interval: Duration::from_secs(30),
        fetch,
    };
    let (rx, last) = state.market_hub.subscribe(spec, symbols).await;
    Ok(market_sse(rx, last))
}

/// GET /api/kline/:symbol
#[derive(serde::Deserialize)]
pub struct KlineQuery {
    #[serde(default = "default_kline_period")]
    period: String,
    #[serde(default = "default_kline_count")]
    count: u32,
}
fn default_kline_period() -> String {
    "day".into()
}
const fn default_kline_count() -> u32 {
    256
}

pub async fn get_kline(
    State(state): State<Arc<AppState>>,
    Path(symbol): Path<String>,
    Query(q): Query<KlineQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::handlers::get_kline_data(&state, &symbol, &q.period, q.count)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /api/realtime/:symbol — single shot
pub async fn get_realtime(
    State(state): State<Arc<AppState>>,
    Path(symbol): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::handlers::get_realtime_data(&state, &symbol)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /api/realtime/stream — SSE, every 3s, comma-separated symbols
#[derive(serde::Deserialize)]
pub struct RealtimeStreamQuery {
    /// Comma-separated stock symbols
    symbols: String,
}

pub async fn get_realtime_stream(
    State(state): State<Arc<AppState>>,
    Query(q): Query<RealtimeStreamQuery>,
) -> Result<Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>>, StatusCode> {
    let symbols: Vec<String> = q
        .symbols
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if symbols.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // One shared upstream poll per symbol set, fanned out to every SSE
    // connection via the hub. The fetch closure also repopulates the
    // per-symbol realtime cache, so the single-shot `/api/realtime/:symbol`
    // endpoint reads stream-fresh data instead of fetching upstream itself.
    let cache = Arc::clone(&state.cache);
    let client = Arc::new(XuanguBaoClient::new());
    let fetch: FetchFn = Arc::new(move |syms: &[String]| {
        let client = Arc::clone(&client);
        let cache = Arc::clone(&cache);
        let syms: Vec<String> = syms.to_vec();
        Box::pin(async move {
            let data = client
                .fetch_realtime_batch(&syms)
                .await
                .map_err(|e| e.to_string())?;
            for sym in &syms {
                if let Ok(ykey) = to_yahoo_symbol(sym) {
                    if let Some(quote) = data.get(&ykey) {
                        if let Ok(serialized) = serde_json::to_string(quote) {
                            cache::set(
                                &cache,
                                &format!("realtime_{sym}"),
                                &serialized,
                                CachePolicy::realtime(),
                            );
                        }
                    }
                }
            }
            serde_json::to_string(&data).map_err(|e| e.to_string())
        })
    });

    let spec = StreamSpec {
        key: format!("realtime:{}", symbols.join(",")),
        trading_interval: Duration::from_secs(3),
        nontrading_interval: Duration::from_secs(30),
        fetch,
    };
    let (rx, last) = state.market_hub.subscribe(spec, symbols).await;
    Ok(market_sse(rx, last))
}

/// GET /api/yesterday-limit-up
pub async fn get_yesterday_limit_up(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::handlers::get_yesterday_limit_up_data(&state)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /api/super-stock
pub async fn get_super_stock(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::handlers::get_super_stock_data(&state)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /api/index-quotes
pub async fn get_index_quotes(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::handlers::get_index_quotes_data(&state)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /api/telegraph — all categories
///
/// Query params:
/// - `last_time`: cursor for pagination (omit for initial load)
/// - `count`: items per page (default 20)
pub async fn get_telegraph_all(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<impl IntoResponse, StatusCode> {
    let last_time = params.get("last_time").and_then(|v| v.parse::<i64>().ok());
    let count: u32 = params
        .get("count")
        .and_then(|v| v.parse().ok())
        .unwrap_or(20);

    crate::handlers::get_telegraph_all_data(&state, last_time, Some(count))
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /api/telegraph/:category
pub async fn get_telegraph(
    State(state): State<Arc<AppState>>,
    Path(category): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::handlers::get_telegraph_data(&state, &category)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /api/dapan-abnormal
pub async fn get_dapan_abnormal(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::handlers::get_dapan_abnormal_data(&state)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /api/market-trend/:symbol
pub async fn get_market_trend(
    State(state): State<Arc<AppState>>,
    Path(symbol): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::handlers::get_market_trend_data(&state, &symbol)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn get_plates_by_any_stock(
    State(state): State<Arc<AppState>>,
    Path(symbol): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::handlers::get_plates_by_any_stock_data(&state, &symbol)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /api/roles
pub async fn get_roles(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::handlers::get_roles_data(&state)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /api/roles/:name/positions
pub async fn get_role_positions(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::handlers::get_role_positions_data(&state, &name)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /api/roles/:name/account
pub async fn get_role_account(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::handlers::get_role_account_data(&state, &name)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /api/roles/:name/trades?limit=100
pub async fn get_role_trades(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<impl IntoResponse, StatusCode> {
    let limit = parse_u32(&params, "limit", 100);
    crate::handlers::get_role_trades_data(&state, &name, limit)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// 读取 `?key=` 并解析为正整数，缺失/非法时用默认值。
fn parse_u32(params: &std::collections::HashMap<String, String>, key: &str, fallback: u32) -> u32 {
    params
        .get(key)
        .and_then(|v| v.parse().ok())
        .filter(|v| *v > 0)
        .unwrap_or(fallback)
}

// ── Role CRUD handlers ─────────────────────────────────

/// 角色写操作失败时把可读原因返回给前端。
///
/// 角色相关的错误绝大多数是调用方输入问题（重名、只读、非法资金），
/// 既然后端只返回 `String`，这里统一按 400 + `{ "error": … }` 透出；
/// 这样前端的创建/删除/重置弹窗才能显示真正原因，而不是光秃秃的 `500`。
fn role_action_response(result: Result<serde_json::Value, String>) -> axum::response::Response {
    match result {
        Ok(value) => (StatusCode::OK, Json(value)).into_response(),
        Err(e) => {
            openatrading_core::log::warn!("角色操作失败: {e}");
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": e })),
            )
                .into_response()
        }
    }
}

/// POST /api/roles — create a new role
pub async fn create_role(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateRoleRequest>,
) -> axum::response::Response {
    role_action_response(
        crate::handlers::create_role_data(
            &state,
            &body.name,
            &body.display_name,
            body.initial_capital,
        )
        .await,
    )
}

/// POST /api/roles/:name/orders — 手动下单
///
/// 校验失败返回 **400 + 可读中文原因**（前端要展示给用户），存储失败才是 500。
/// 这是本项目第一个区分二者的写接口。
pub async fn place_order(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> axum::response::Response {
    match crate::handlers::place_order_data(&state, &name, body).await {
        Ok(value) => (StatusCode::CREATED, Json(value)).into_response(),
        Err(e) if e.is_client_error() => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
        Err(e) => {
            openatrading_core::log::error!("下单失败: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "下单失败，请查看服务端日志" })),
            )
                .into_response()
        }
    }
}

/// DELETE /api/roles/:name — delete role and all its data
pub async fn delete_role(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> axum::response::Response {
    role_action_response(crate::handlers::delete_role_data(&state, &name).await)
}

/// POST /api/roles/:name/reset — 清空该角色的持仓与流水
pub async fn reset_role(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> axum::response::Response {
    role_action_response(crate::handlers::reset_role_data(&state, &name).await)
}

/// GET /api/roles/:name/pnl-calendar — 区间收益分析（日历/曲线）
#[derive(serde::Deserialize)]
pub struct PnlCalendarQuery {
    pub from: Option<String>,
    pub to: Option<String>,
    pub granularity: Option<String>,
}

/// GET /api/roles/:name/pnl-calendar?from=&to=&granularity=
pub async fn get_role_pnl_calendar(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Query(q): Query<PnlCalendarQuery>,
) -> axum::response::Response {
    use axum::response::IntoResponse;

    match crate::handlers::get_role_pnl_calendar_data(
        &state,
        &name,
        q.from.as_deref(),
        q.to.as_deref(),
        q.granularity.as_deref(),
    )
    .await
    {
        Ok(value) => Json(value).into_response(),
        Err((status, msg)) => (status, Json(serde_json::json!({ "error": msg }))).into_response(),
    }
}

/// GET /api/roles/:name/pnl-history?days=30 — P&L curve data
pub async fn get_role_pnl_history(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<impl IntoResponse, StatusCode> {
    let days = params
        .get("days")
        .and_then(|d| d.parse().ok())
        .unwrap_or(30);
    crate::handlers::get_role_pnl_history_data(&state, &name, days)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /api/roles/:name/history — closed trades history
pub async fn get_role_history(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<impl IntoResponse, StatusCode> {
    let limit = parse_u32(&params, "limit", 100);
    crate::handlers::get_role_history_data(&state, &name, limit)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

// ── Replay handlers ────────────────────────────────────

/// GET /api/replay/events — fetch all events for latest trading day
pub async fn get_replay_events(
    State(_state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<impl IntoResponse, StatusCode> {
    let min_pcp: f64 = params
        .get("min_pcp")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.03);
    crate::handlers::get_replay_events_data(min_pcp)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /api/replay/index — three-index trend data
pub async fn get_replay_index(
    State(_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::handlers::get_replay_index_data()
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /api/replay/news — cursor-paginated 7×24 news
/// ?tailmark=N&msgIdMark=S → older page; no params → latest page
#[derive(serde::Deserialize)]
pub struct ReplayNewsQuery {
    tailmark: Option<i64>,
    #[serde(rename = "msgIdMark")]
    msg_id_mark: Option<String>,
}

pub async fn get_replay_news(
    State(_state): State<Arc<AppState>>,
    Query(q): Query<ReplayNewsQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::handlers::get_replay_news_data(q.tailmark, q.msg_id_mark)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /api/replay/snapshots — 5-min bucketed emotion snapshots
pub async fn get_replay_snapshots(
    State(_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::handlers::get_replay_snapshots_data()
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /api/dragon-tiger/detail?page=1&page_size=20
#[derive(Deserialize)]
pub struct DragonTigerDetailQuery {
    #[serde(default = "default_page")]
    pub page: usize,
    #[serde(default = "default_page_size")]
    pub page_size: usize,
}

const fn default_page() -> usize {
    1
}
const fn default_page_size() -> usize {
    20
}

pub async fn get_dragon_tiger_detail(
    State(state): State<Arc<AppState>>,
    Query(query): Query<DragonTigerDetailQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let cached = cache::get_or_fetch(
        &state.cache,
        "dragon_tiger_detail",
        CachePolicy::daily(),
        || async {
            let dates = last_n_trading_days(3);
            let data = state
                .ctx
                .api
                .eastmoney
                .lhb_detail(
                    dates.last().ok_or(StatusCode::BAD_REQUEST)?.as_str(),
                    dates.first().ok_or(StatusCode::BAD_REQUEST)?.as_str(),
                )
                .await
                .map_err(|e| {
                    openatrading_core::log::error!("[dragon-tiger] detail fetch error: {:#}", e);
                    StatusCode::SERVICE_UNAVAILABLE
                })?;
            Ok(serde_json::to_string(&data).unwrap_or_default())
        },
    )
    .await?;
    let all: Vec<LhbDetail> = serde_json::from_str(&cached).unwrap_or_default();

    // Merge same-stock entries with multiple billboard reasons
    let mut merged: std::collections::HashMap<(String, String), LhbDetail> =
        std::collections::HashMap::new();
    for item in all {
        let key = (item.code.clone(), item.trade_date.clone());
        match merged.get_mut(&key) {
            Some(existing) => {
                // Sum monetary fields
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
                // Append explanation
                if let Some(ref exp) = item.explanation {
                    if !exp.is_empty() {
                        let prefix = existing.explanation.as_deref().unwrap_or("");
                        let sep = if prefix.is_empty() { "" } else { "; " };
                        existing.explanation = Some(format!("{prefix}{sep}{exp}"));
                    }
                }
            }
            None => {
                merged.insert(key, item);
            }
        }
    }
    let merged_all: Vec<LhbDetail> = merged.into_values().collect();

    let total = merged_all.len();
    let total_net_amt: f64 = merged_all.iter().filter_map(|d| d.billboard_net_amt).sum();
    let net_buy_count = merged_all
        .iter()
        .filter(|d| d.billboard_net_amt.is_some_and(|v| v > 0.0))
        .count();
    let net_sell_count = merged_all
        .iter()
        .filter(|d| d.billboard_net_amt.is_some_and(|v| v < 0.0))
        .count();
    let start = (query.page.saturating_sub(1)) * query.page_size;
    let data: Vec<LhbDetail> = merged_all
        .into_iter()
        .skip(start)
        .take(query.page_size)
        .collect();
    Ok(Json(serde_json::json!({
        "data": data,
        "total": total,
        "page": query.page,
        "page_size": query.page_size,
        "total_net_amt": total_net_amt,
        "net_buy_count": net_buy_count,
        "net_sell_count": net_sell_count,
    })))
}

/// GET /api/dragon-tiger/stock-seat?code=000001&date=20240101&flag=buy
#[derive(Deserialize)]
pub struct StockSeatQuery {
    pub code: String,
    pub date: String,
    pub flag: String,
}

pub async fn get_dragon_tiger_stock_seat(
    State(state): State<Arc<AppState>>,
    Query(query): Query<StockSeatQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::handlers::get_dragon_tiger_stock_seat_data(&state, &query.code, &query.date, &query.flag)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /api/search-stocks?q=keyword — search stocks by name or code
pub async fn search_stocks(
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let q = params
        .get("q")
        .map_or("", std::string::String::as_str)
        .trim()
        .to_string();
    if q.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "missing q"})),
        )
            .into_response();
    }
    match crate::handlers::search_stocks_data(&q).await {
        Ok(data) => (StatusCode::OK, Json(data)).into_response(),
        Err(e) => {
            openatrading_core::log::error!("search_stocks: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "search failed"})),
            )
                .into_response()
        }
    }
}

// ── Watchlist ─────────────────────────────────────────────

/// GET /api/watchlist
pub async fn get_watchlist() -> impl IntoResponse {
    match crate::handlers::get_watchlist_data().await {
        Ok(data) => (StatusCode::OK, Json(data)).into_response(),
        Err(e) => {
            openatrading_core::log::error!("get_watchlist: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "load failed"})),
            )
                .into_response()
        }
    }
}

/// POST /api/watchlist
pub async fn add_watchlist(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    let code = match body.get("code").and_then(|v| v.as_str()) {
        Some(c) => c.to_string(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "missing code"})),
            )
                .into_response()
        }
    };
    let name = match body.get("name").and_then(|v| v.as_str()) {
        Some(n) => n.to_string(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "missing name"})),
            )
                .into_response()
        }
    };

    match crate::handlers::add_watchlist_data(&code, &name).await {
        Ok(data) => (StatusCode::CREATED, Json(data)).into_response(),
        Err(e) => {
            if e == "already exists" {
                return (
                    StatusCode::CONFLICT,
                    Json(serde_json::json!({"error": "already exists"})),
                )
                    .into_response();
            }
            openatrading_core::log::error!("add_watchlist: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "save failed"})),
            )
                .into_response()
        }
    }
}

/// GET /api/about — application version info
pub async fn get_about() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "app_name": "OpenATrading",
        "version": env!("CARGO_PKG_VERSION"),
        "git_version": option_env!("GIT_COMMIT_SHORT").unwrap_or("unknown"),
        "engine": format!("pi-rs {}", option_env!("PI_RS_VERSION").unwrap_or("unknown")),
    }))
}

/// DELETE /api/watchlist/:code
pub async fn delete_watchlist(Path(code): Path<String>) -> impl IntoResponse {
    match crate::handlers::delete_watchlist_data(&code).await {
        Ok(data) => (StatusCode::OK, Json(data)).into_response(),
        Err(e) => {
            if e.starts_with("not found") {
                return (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({"error": "not found"})),
                )
                    .into_response();
            }
            openatrading_core::log::error!("delete_watchlist: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "delete failed"})),
            )
                .into_response()
        }
    }
}
