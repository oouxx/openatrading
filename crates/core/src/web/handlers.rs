use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::agents::sentiment::analyze_sentiment;
use crate::api::xuangubao::{KlinePeriod, XuanguBaoClient};
use crate::api::Client as ApiClient;
use crate::trade::TradeManager;

use super::{AppState, RoleEntry};
use crate::roles::pnl::compute_pnl_history;

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
    pub strategy: String,
}

#[derive(Deserialize)]
pub struct UpdateStrategyRequest {
    pub strategy: String,
}

#[derive(Serialize)]
pub struct PnlHistoryResponse {
    pub points: Vec<crate::roles::pnl::PnlPoint>,
    pub initial_capital: f64,
}

// ── Handlers ──────────────────────────────────────────

/// GET /api/overview — market overview + risk level + decision summary
pub async fn get_overview() -> Result<impl IntoResponse, StatusCode> {
    let client = XuanguBaoClient::new();
    let overview = client.fetch_market_overview().await.map_err(|e| {
        eprintln!("overview error: {}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    let ratio = overview.rise_count as f64 / overview.fall_count.max(1) as f64;
    let risk_level = if ratio < 0.8 && overview.bomb_rate > 30.0 {
        "高"
    } else if ratio < 1.2 || overview.bomb_rate > 20.0 {
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

    Ok(Json(OverviewResponse {
        date: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
        rise_count: overview.rise_count,
        fall_count: overview.fall_count,
        limit_up_count: overview.limit_up_count,
        limit_down_count: overview.limit_down_count,
        bomb_rate: overview.bomb_rate,
        risk_level: risk_level.to_string(),
        decision_summary,
        market_temperature: overview.market_temperature,
        yesterday_limit_up_avg_pcp: overview.yesterday_limit_up_avg_pcp,
    }))
}

/// GET /api/sentiment-history
pub async fn get_sentiment_history() -> Result<impl IntoResponse, StatusCode> {
    let client = XuanguBaoClient::new();
    let trading_days = XuanguBaoClient::last_n_trading_days(10);

    let mut days: Vec<SentimentHistoryDay> = Vec::with_capacity(trading_days.len());

    for date in &trading_days {
        match client.fetch_market_overview_for_date(date).await {
            Ok(Some(d)) => {
                let rise_ratio = d.rise_count as f64 / d.fall_count.max(1) as f64;
                let sentiment = d.market_temperature * 0.5
                    + (1.0 - d.bomb_rate / 100.0) * 25.0
                    + (rise_ratio / 3.0).min(1.0) * 25.0;
                days.push(SentimentHistoryDay {
                    date: d.date,
                    sentiment: (sentiment * 10.0).round() / 10.0,
                    limit_up_count: d.limit_up_count,
                    bomb_rate: (d.bomb_rate * 100.0).round() / 100.0,
                    rise_ratio: (rise_ratio * 100.0).round() / 100.0,
                });
            }
            Ok(None) => {
                eprintln!("sentiment_history: no data for {date}");
            }
            Err(e) => {
                eprintln!("sentiment_history error for {date}: {e}");
            }
        }
    }

    // Reverse so most recent is last
    days.reverse();

    Ok(Json(days))
}

/// GET /api/limit-up
pub async fn get_limit_up() -> Result<impl IntoResponse, StatusCode> {
    let client = XuanguBaoClient::new();
    let data = client.fetch_limit_up().await.map_err(|e| {
        eprintln!("limit_up error: {}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    Ok(Json(data))
}

/// GET /api/limit-down
pub async fn get_limit_down() -> Result<impl IntoResponse, StatusCode> {
    let client = XuanguBaoClient::new();
    let data = client.fetch_limit_down().await.map_err(|e| {
        eprintln!("limit_down error: {}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    Ok(Json(data))
}

/// GET /api/plate-abnormal?count=30
#[derive(serde::Deserialize)]
pub struct PlateAbnormalQuery {
    #[serde(default = "default_plate_count")]
    count: i32,
}
fn default_plate_count() -> i32 {
    30
}

pub async fn get_plate_abnormal(
    Query(q): Query<PlateAbnormalQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let client = XuanguBaoClient::new();
    let data = client.fetch_plate_abnormal(q.count).await.map_err(|e| {
        eprintln!("plate_abnormal error: {}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    Ok(Json(data))
}

/// GET /api/field-items?date=YYYY-MM-DD
#[derive(serde::Deserialize)]
pub struct FieldItemsQuery {
    date: Option<String>,
}

pub async fn get_field_items(
    Query(q): Query<FieldItemsQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    if std::env::var("JIUYAN_PHONE").is_err() {
        return Ok(Json(vec![]));
    }
    let date = q
        .date
        .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());
    let client = ApiClient::new();
    let data = client.jiuyan.fetch_field(&date, 1).await.map_err(|e| {
        eprintln!("field_items error: {}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    Ok(Json(data))
}

/// GET /api/timeline
pub async fn get_timeline() -> Result<impl IntoResponse, StatusCode> {
    if std::env::var("JIUYAN_PHONE").is_err() {
        return Ok(Json(vec![]));
    }
    let month = chrono::Local::now().format("%Y-%m").to_string();
    let client = ApiClient::new();
    let data = client.jiuyan.fetch_timeline(&month).await.map_err(|e| {
        eprintln!("timeline error: {}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    Ok(Json(data))
}

/// GET /api/sentiment
pub async fn get_sentiment() -> Result<impl IntoResponse, StatusCode> {
    if std::env::var("JIUYAN_PHONE").is_err() {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    let client = ApiClient::new();
    let data = analyze_sentiment(&client).await.map_err(|e| {
        eprintln!("sentiment error: {}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    Ok(Json(data))
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
fn default_kline_count() -> u32 {
    256
}

pub async fn get_kline(
    Path(symbol): Path<String>,
    Query(q): Query<KlineQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let period = match q.period.as_str() {
        "week" => KlinePeriod::Week,
        "month" => KlinePeriod::Month,
        "minute" => KlinePeriod::Minute,
        _ => KlinePeriod::Day,
    };
    let client = XuanguBaoClient::new();
    let data = client
        .fetch_kline(&symbol, period, q.count)
        .await
        .map_err(|e| {
            eprintln!("kline error: {}", e);
            StatusCode::SERVICE_UNAVAILABLE
        })?;
    Ok(Json(data))
}

/// GET /api/realtime/:symbol
pub async fn get_realtime(Path(symbol): Path<String>) -> Result<impl IntoResponse, StatusCode> {
    let client = XuanguBaoClient::new();
    let data = client.fetch_realtime(&symbol).await.map_err(|e| {
        eprintln!("realtime error: {}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    Ok(Json(data))
}

/// GET /api/yesterday-limit-up
pub async fn get_yesterday_limit_up() -> Result<impl IntoResponse, StatusCode> {
    let client = XuanguBaoClient::new();
    let data = client.fetch_yesterday_limit_up().await.map_err(|e| {
        eprintln!("yesterday_limit_up error: {:#}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    Ok(Json(data))
}

/// GET /api/super-stock
pub async fn get_super_stock() -> Result<impl IntoResponse, StatusCode> {
    let client = XuanguBaoClient::new();
    let data = client.fetch_super_stock().await.map_err(|e| {
        eprintln!("super_stock error: {:#}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    Ok(Json(data))
}

/// GET /api/index-quotes
pub async fn get_index_quotes() -> Result<impl IntoResponse, StatusCode> {
    let client = XuanguBaoClient::new();
    let data = client
        .fetch_market_real(&["000001.SS", "399001.SZ", "399006.SZ"])
        .await
        .map_err(|e| {
            eprintln!("index_quotes error: {}", e);
            StatusCode::SERVICE_UNAVAILABLE
        })?;
    Ok(Json(data))
}

/// GET /api/telegraph — all categories
pub async fn get_telegraph_all() -> Result<impl IntoResponse, StatusCode> {
    let client = ApiClient::new();
    let data = client.cls.fetch_telegraph("").await.map_err(|e| {
        eprintln!("telegraph error: {:#}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    Ok(Json(data))
}

/// GET /api/telegraph/:category
pub async fn get_telegraph(Path(category): Path<String>) -> Result<impl IntoResponse, StatusCode> {
    let client = ApiClient::new();
    let data = client.cls.fetch_telegraph(&category).await.map_err(|e| {
        eprintln!("telegraph error: {:#}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    Ok(Json(data))
}

/// GET /api/dapan-abnormal
pub async fn get_dapan_abnormal() -> Result<impl IntoResponse, StatusCode> {
    let client = ApiClient::new();
    let data = client.xuangubao.fetch_dapan_abnormal().await.map_err(|e| {
        eprintln!("dapan_abnormal error: {:#}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    Ok(Json(data))
}

/// GET /api/market-trend/:symbol
pub async fn get_market_trend(Path(symbol): Path<String>) -> Result<impl IntoResponse, StatusCode> {
    let client = ApiClient::new();
    let data = client
        .xuangubao
        .fetch_market_trend(&symbol)
        .await
        .map_err(|e| {
            eprintln!("market_trend error: {:#}", e);
            StatusCode::SERVICE_UNAVAILABLE
        })?;
    Ok(Json(data))
}

pub async fn get_plates_by_any_stock(
    State(state): State<Arc<AppState>>,
    Path(symbol): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let plates = state
        .api
        .xuangubao
        .plates_by_any_stock(&symbol)
        .await
        .map_err(|e| {
            eprintln!("get_plates_by_any_stock: error: {}", e);
            StatusCode::SERVICE_UNAVAILABLE
        })?;
    Ok(Json(plates))
}

/// GET /api/roles
pub async fn get_roles(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    let roles = state.roles.read().await;
    Ok(Json(roles.clone()))
}

/// GET /api/roles/:name/positions
pub async fn get_role_positions(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let positions = state.trade.list_positions(&name).map_err(|e| {
        eprintln!("get_role_positions error: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let result: Vec<serde_json::Value> = positions
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p.id,
                "symbol": p.symbol,
                "stock_name": p.stock_name,
                "cost_price": p.cost_price,
                "current_price": p.current_price.unwrap_or(0.0),
                "shares": p.shares,
                "entry_reason": p.entry_reason,
                "entry_date": p.entry_date,
            })
        })
        .collect();
    Ok(Json(result))
}

/// GET /api/roles/:name/logs
pub async fn get_role_logs(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let logs = state.trade.get_trade_logs_by_role(&name, 20).map_err(|e| {
        eprintln!("get_role_logs error: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let result: Vec<serde_json::Value> = logs
        .iter()
        .map(|l| {
            let date: String = l.created_at.chars().take(10).collect();
            serde_json::json!({
                "date": date,
                "action": l.action,
                "symbol": l.symbol,
                "price": l.price,
                "shares": l.shares,
                "reason": l.reason,
            })
        })
        .collect();
    Ok(Json(result))
}

// ── Role CRUD handlers ─────────────────────────────────

/// POST /api/roles — create a new role
pub async fn create_role(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateRoleRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::roles::save_role_strategy(&state.strategies_dir, &body.name, &body.strategy).map_err(
        |e| {
            eprintln!("create_role error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        },
    )?;
    {
        let mut roles = state.roles.write().await;
        if let Some(existing) = roles.iter_mut().find(|r| r.name == body.name) {
            existing.display_name = body.display_name.clone();
            existing.strategy = body.strategy.clone();
        } else {
            roles.push(RoleEntry {
                name: body.name.clone(),
                display_name: body.display_name.clone(),
                strategy: body.strategy.clone(),
            });
        }
    }
    Ok(Json(serde_json::json!({"ok": true})))
}

/// PUT /api/roles/:name/strategy — update role strategy
pub async fn update_role_strategy(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(body): Json<UpdateStrategyRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::roles::save_role_strategy(&state.strategies_dir, &name, &body.strategy).map_err(
        |e| {
            eprintln!("update_role_strategy error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        },
    )?;
    {
        let mut roles = state.roles.write().await;
        if let Some(entry) = roles.iter_mut().find(|r| r.name == name) {
            entry.strategy = body.strategy.clone();
        }
    }
    Ok(Json(serde_json::json!({"ok": true})))
}

/// DELETE /api/roles/:name — delete role and all its data
pub async fn delete_role(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    state.trade.delete_trade_logs_by_role(&name).ok();
    state.trade.delete_positions_by_role(&name).ok();
    crate::roles::delete_role_dir(&state.strategies_dir, &name).ok();
    {
        let mut roles = state.roles.write().await;
        roles.retain(|r| r.name != name);
    }
    Ok(Json(serde_json::json!({"ok": true})))
}

/// POST /api/roles/:name/reset — reset account to 1,000,000
pub async fn reset_role(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    state.trade.delete_trade_logs_by_role(&name).map_err(|e| {
        eprintln!("reset_role error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    state.trade.delete_positions_by_role(&name).map_err(|e| {
        eprintln!("reset_role error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(serde_json::json!({"ok": true})))
}

/// GET /api/roles/:name/pnl-history?days=30 — P&L curve data
pub async fn get_role_pnl_history(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<impl IntoResponse, StatusCode> {
    let days: usize = params
        .get("days")
        .and_then(|d| d.parse().ok())
        .unwrap_or(30);
    let all_logs = state.trade.get_all_trade_logs_by_role(&name).map_err(|e| {
        eprintln!("get_role_pnl_history error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Calculate current positions value
    let positions = state.trade.list_positions(&name).unwrap_or_default();
    let current_value: f64 = positions
        .iter()
        .map(|p| p.current_price.unwrap_or(p.cost_price) * p.shares as f64)
        .sum();

    let mut points = compute_pnl_history(&all_logs, current_value);
    // Filter to last N days
    let cutoff = points.len().saturating_sub(days);
    points = points.into_iter().skip(cutoff).collect();

    Ok(Json(PnlHistoryResponse {
        points,
        initial_capital: 1_000_000.0,
    }))
}