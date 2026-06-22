use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing;

use openatrading_core::api::eastmoney::{LhbClient, LhbDetail, SeatFlag};
use openatrading_core::api::xuangubao::{KlinePeriod, XuanguBaoClient};
use openatrading_core::api::Client as ApiClient;
use openatrading_core::{agents::sentiment::analyze_sentiment, api::utils::last_n_trading_days};

use crate::cache::{self, CachePolicy};
use crate::{AppState, RoleEntry};
use openatrading_core::roles::pnl::compute_pnl_history;

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
    pub points: Vec<openatrading_core::roles::pnl::PnlPoint>,
    pub initial_capital: f64,
}

// ── Handlers ──────────────────────────────────────────

/// GET /api/overview — market overview + risk level + decision summary
pub async fn get_overview(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    cache::get_or_fetch_json(
        &state.cache,
        "overview",
        CachePolicy::overview(),
        || async {
            let client = XuanguBaoClient::new();
            let overview = client.fetch_market_overview().await.map_err(|e| {
                tracing::error!("overview error: {}", e);
                StatusCode::SERVICE_UNAVAILABLE
            })?;

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

            let resp = OverviewResponse {
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

/// GET /api/sentiment-history
pub async fn get_sentiment_history(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    cache::get_or_fetch_json(
        &state.cache,
        "sentiment_history",
        CachePolicy::daily(),
        || async {
            let client = XuanguBaoClient::new();
            let trading_days = XuanguBaoClient::last_n_trading_days(10);

            let mut days: Vec<SentimentHistoryDay> = Vec::with_capacity(trading_days.len());

            for date in &trading_days {
                match client.fetch_market_overview_for_date(date).await {
                    Ok(Some(d)) => {
                        let rise_ratio = d.rise_count as f64 / d.fall_count.max(1) as f64;
                        let sentiment = d.market_temperature * 0.5
                            + (1.0 - d.bomb_rate) * 25.0
                            + (rise_ratio / 3.0).min(1.0) * 25.0;
                        days.push(SentimentHistoryDay {
                            date: d.date,
                            sentiment: (sentiment * 10.0).round() / 10.0,
                            limit_up_count: d.limit_up_count,
                            bomb_rate: (d.bomb_rate * 100.0).round() / 100.0,
                            rise_ratio: (rise_ratio * 100.0).round() / 100.0,
                        });
                    },
                    Ok(None) => {
                        tracing::error!("sentiment_history: no data for {date}");
                    },
                    Err(e) => {
                        tracing::error!("sentiment_history error for {date}: {e}");
                    },
                }
            }

            days.reverse();
            Ok(days)
        },
    )
    .await
}

/// GET /api/limit-up
pub async fn get_limit_up(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    cache::get_or_fetch_json(
        &state.cache,
        "limit_up",
        CachePolicy::snapshot(),
        || async {
            let client = XuanguBaoClient::new();
            let data = client.fetch_limit_up().await.map_err(|e| {
                tracing::error!("limit_up error: {}", e);
                StatusCode::SERVICE_UNAVAILABLE
            })?;
            Ok(data)
        },
    )
    .await
}

/// GET /api/limit-down
pub async fn get_limit_down(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    cache::get_or_fetch_json(
        &state.cache,
        "limit_down",
        CachePolicy::snapshot(),
        || async {
            let client = XuanguBaoClient::new();
            let data = client.fetch_limit_down().await.map_err(|e| {
                tracing::error!("limit_down error: {}", e);
                StatusCode::SERVICE_UNAVAILABLE
            })?;
            Ok(data)
        },
    )
    .await
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
    State(state): State<Arc<AppState>>,
    Query(q): Query<PlateAbnormalQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let cache_key = format!("plate_abnormal_{}", q.count);
    cache::get_or_fetch_json(
        &state.cache,
        &cache_key,
        CachePolicy::snapshot(),
        || async {
            let client = XuanguBaoClient::new();
            let data = client.fetch_plate_abnormal(q.count).await.map_err(|e| {
                tracing::error!("plate_abnormal error: {}", e);
                StatusCode::SERVICE_UNAVAILABLE
            })?;
            Ok(data)
        },
    )
    .await
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
    let date = q.date.unwrap_or_else(|| {
        openatrading_core::utils::time::beijing_now()
            .format("%Y-%m-%d")
            .to_string()
    });
    let cache_key = format!("field_items_{}", date);
    cache::get_or_fetch_json(&state.cache, &cache_key, CachePolicy::daily(), || async {
        let client = ApiClient::new();
        let data = client.jiuyan.fetch_field(&date, 1).await.map_err(|e| {
            tracing::error!("field_items error: {}", e);
            StatusCode::SERVICE_UNAVAILABLE
        })?;
        Ok(data)
    })
    .await
}

/// GET /api/timeline
pub async fn get_timeline(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    if std::env::var("JIUYAN_PHONE").is_err() {
        return Ok(Json(serde_json::json!([])));
    }
    let month = openatrading_core::utils::time::beijing_now()
        .format("%Y-%m")
        .to_string();
    let cache_key = format!("timeline_{}", month);
    cache::get_or_fetch_json(&state.cache, &cache_key, CachePolicy::daily(), || async {
        let client = ApiClient::new();
        let data = client.jiuyan.fetch_timeline(&month).await.map_err(|e| {
            tracing::error!("timeline error: {}", e);
            StatusCode::SERVICE_UNAVAILABLE
        })?;
        Ok(data)
    })
    .await
}

/// GET /api/sentiment
pub async fn get_sentiment(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    if std::env::var("JIUYAN_PHONE").is_err() {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    cache::get_or_fetch_json(
        &state.cache,
        "sentiment",
        CachePolicy::snapshot(),
        || async {
            let client = ApiClient::new();
            let data = analyze_sentiment(&client).await.map_err(|e| {
                tracing::error!("sentiment error: {}", e);
                StatusCode::SERVICE_UNAVAILABLE
            })?;
            Ok(data)
        },
    )
    .await
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
    State(state): State<Arc<AppState>>,
    Path(symbol): Path<String>,
    Query(q): Query<KlineQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let period = match q.period.as_str() {
        "week" => KlinePeriod::Week,
        "month" => KlinePeriod::Month,
        "minute" => KlinePeriod::Minute,
        _ => KlinePeriod::Day,
    };
    let cache_key = format!("kline_{}_{}_{}", symbol, q.period, q.count);
    cache::get_or_fetch_json(&state.cache, &cache_key, CachePolicy::daily(), || async {
        let client = XuanguBaoClient::new();
        let data = client
            .fetch_kline(&symbol, period, q.count)
            .await
            .map_err(|e| {
                tracing::error!("kline error: {}", e);
                StatusCode::SERVICE_UNAVAILABLE
            })?;
        Ok(data)
    })
    .await
}

/// GET /api/realtime/:symbol
pub async fn get_realtime(
    State(state): State<Arc<AppState>>,
    Path(symbol): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let cache_key = format!("realtime_{}", symbol);
    cache::get_or_fetch_json(
        &state.cache,
        &cache_key,
        CachePolicy::realtime(),
        || async {
            let client = XuanguBaoClient::new();
            let data = client.fetch_realtime(&symbol).await.map_err(|e| {
                let msg = e.to_string();
                tracing::warn!("realtime unavailable for {symbol}: {msg}");
                if msg.contains("not found") {
                    StatusCode::NOT_FOUND
                } else {
                    StatusCode::SERVICE_UNAVAILABLE
                }
            })?;
            Ok(data)
        },
    )
    .await
}

/// GET /api/yesterday-limit-up
pub async fn get_yesterday_limit_up(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    cache::get_or_fetch_json(
        &state.cache,
        "yesterday_limit_up",
        CachePolicy::snapshot(),
        || async {
            let client = XuanguBaoClient::new();
            let data = client.fetch_yesterday_limit_up().await.map_err(|e| {
                tracing::error!("yesterday_limit_up error: {:#}", e);
                StatusCode::SERVICE_UNAVAILABLE
            })?;
            Ok(data)
        },
    )
    .await
}

/// GET /api/super-stock
pub async fn get_super_stock(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    cache::get_or_fetch_json(
        &state.cache,
        "super_stock",
        CachePolicy::snapshot(),
        || async {
            let client = XuanguBaoClient::new();
            let data = client.fetch_super_stock().await.map_err(|e| {
                tracing::error!("super_stock error: {:#}", e);
                StatusCode::SERVICE_UNAVAILABLE
            })?;
            Ok(data)
        },
    )
    .await
}

/// GET /api/index-quotes
pub async fn get_index_quotes(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    cache::get_or_fetch_json(
        &state.cache,
        "index_quotes",
        CachePolicy::realtime(),
        || async {
            let client = XuanguBaoClient::new();
            let data = client
                .fetch_market_real(&["000001.SS", "399001.SZ", "399006.SZ"])
                .await
                .map_err(|e| {
                    tracing::error!("index_quotes error: {}", e);
                    StatusCode::SERVICE_UNAVAILABLE
                })?;
            Ok(data)
        },
    )
    .await
}

/// GET /api/telegraph — all categories
pub async fn get_telegraph_all(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    cache::get_or_fetch_json(
        &state.cache,
        "telegraph_all",
        CachePolicy::telegraph(),
        || async {
            let client = ApiClient::new();
            let data = client
                .cls
                .fetch_telegraph("", None, 20)
                .await
                .map_err(|e| {
                    tracing::error!("[telegraph] fetch failed: {:#}", e);
                    tracing::error!("[telegraph] error chain:");
                    for cause in e.chain() {
                        tracing::error!("[telegraph]   caused by: {}", cause);
                    }
                    StatusCode::SERVICE_UNAVAILABLE
                })?;
            Ok(data)
        },
    )
    .await
}

/// GET /api/telegraph/:category
pub async fn get_telegraph(
    State(state): State<Arc<AppState>>,
    Path(category): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let cache_key = format!("telegraph_{}", category);
    cache::get_or_fetch_json(
        &state.cache,
        &cache_key,
        CachePolicy::telegraph(),
        || async {
            let client = ApiClient::new();
            let data = client
                .cls
                .fetch_telegraph(&category, None, 20)
                .await
                .map_err(|e| {
                    tracing::error!("[telegraph/{}] fetch failed: {:#}", category, e);
                    StatusCode::SERVICE_UNAVAILABLE
                })?;
            Ok(data)
        },
    )
    .await
}

/// GET /api/dapan-abnormal
pub async fn get_dapan_abnormal(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    cache::get_or_fetch_json(
        &state.cache,
        "dapan_abnormal",
        CachePolicy::snapshot(),
        || async {
            let client = ApiClient::new();
            let data = client.xuangubao.fetch_dapan_abnormal().await.map_err(|e| {
                tracing::error!("dapan_abnormal error: {:#}", e);
                StatusCode::SERVICE_UNAVAILABLE
            })?;
            Ok(data)
        },
    )
    .await
}

/// GET /api/market-trend/:symbol
pub async fn get_market_trend(
    State(state): State<Arc<AppState>>,
    Path(symbol): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let cache_key = format!("market_trend_{}", symbol);
    cache::get_or_fetch_json(
        &state.cache,
        &cache_key,
        CachePolicy::realtime(),
        || async {
            let client = ApiClient::new();
            let data = client
                .xuangubao
                .fetch_market_trend(&symbol)
                .await
                .map_err(|e| {
                    tracing::error!("market_trend error: {:#}", e);
                    StatusCode::SERVICE_UNAVAILABLE
                })?;
            Ok(data)
        },
    )
    .await
}

pub async fn get_plates_by_any_stock(
    State(state): State<Arc<AppState>>,
    Path(symbol): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let cache_key = format!("plates_{}", symbol);
    cache::get_or_fetch_json(
        &state.cache,
        &cache_key,
        CachePolicy::snapshot(),
        || async {
            let plates = state
                .ctx
                .api
                .xuangubao
                .plates_by_any_stock(&symbol)
                .await
                .map_err(|e| {
                    tracing::error!("get_plates_by_any_stock: error: {}", e);
                    StatusCode::SERVICE_UNAVAILABLE
                })?;
            Ok(serde_json::to_string(&plates).unwrap_or_default())
        },
    )
    .await
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
    let positions = state.ctx.trade.list_positions(&name).map_err(|e| {
        tracing::error!("get_role_positions error: {e}");
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
    let logs = state
        .ctx
        .trade
        .get_trade_logs_by_role(&name, 20)
        .map_err(|e| {
            tracing::error!("get_role_logs error: {e}");
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
    openatrading_core::roles::save_role_strategy(&body.name, &body.strategy).map_err(|e| {
        tracing::error!("create_role error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
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
                readonly: false,
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
    // Reject edits on config-toml readonly roles
    {
        let roles = state.roles.read().await;
        if let Some(role) = roles.iter().find(|r| r.name == name) {
            if role.readonly {
                return Err(StatusCode::FORBIDDEN);
            }
        }
    }
    openatrading_core::roles::save_role_strategy(&name, &body.strategy).map_err(|e| {
        tracing::error!("update_role_strategy error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
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
    // Reject deletion of config-toml readonly roles
    {
        let roles = state.roles.read().await;
        if let Some(role) = roles.iter().find(|r| r.name == name) {
            if role.readonly {
                return Err(StatusCode::FORBIDDEN);
            }
        }
    }
    state.ctx.trade.delete_positions_by_role(&name).ok();
    state.ctx.trade.delete_trade_logs_by_role(&name).ok();
    openatrading_core::roles::delete_role_dir(&name).ok();
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
    state
        .ctx
        .trade
        .delete_trade_logs_by_role(&name)
        .map_err(|e| {
            tracing::error!("reset_role error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    state
        .ctx
        .trade
        .delete_positions_by_role(&name)
        .map_err(|e| {
            tracing::error!("reset_role error: {}", e);
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
    let all_logs = state
        .ctx
        .trade
        .get_all_trade_logs_by_role(&name)
        .map_err(|e| {
            tracing::error!("get_role_pnl_history error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Calculate current positions value
    let positions = state.ctx.trade.list_positions(&name).unwrap_or_default();
    let current_value: f64 = positions
        .iter()
        .map(|p| p.current_price.unwrap_or(p.cost_price) * p.shares as f64)
        .sum();

    let mut points = compute_pnl_history(&all_logs, current_value);
    // days=0 means return all points
    if days > 0 {
        let cutoff = points.len().saturating_sub(days);
        points = points.into_iter().skip(cutoff).collect();
    }

    Ok(Json(PnlHistoryResponse {
        points,
        initial_capital: 1_000_000.0,
    }))
}

/// GET /api/roles/:name/history — closed positions history
pub async fn get_role_history(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<impl IntoResponse, StatusCode> {
    let limit: usize = params
        .get("limit")
        .and_then(|d| d.parse().ok())
        .unwrap_or(100);
    let positions = state
        .ctx
        .trade
        .get_closed_positions(&name, limit)
        .map_err(|e| {
            tracing::error!("get_role_history error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(positions))
}

// ── Replay handlers ────────────────────────────────────

/// GET /api/replay/events — fetch all events for latest trading day
pub async fn get_replay_events(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    cache::get_or_fetch_json(
        &state.cache,
        "replay_events",
        CachePolicy::replay(),
        || async {
            let client = XuanguBaoClient::new();
            let all_events = client.fetch_all_day_events().await.map_err(|e| {
                tracing::error!("[replay] events error: {:#}", e);
                StatusCode::SERVICE_UNAVAILABLE
            })?;
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
            Ok(serde_json::to_string(&page).unwrap_or_default())
        },
    )
    .await
}

/// GET /api/replay/index — three-index trend data
pub async fn get_replay_index(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    cache::get_or_fetch_json(
        &state.cache,
        "replay_index",
        CachePolicy::replay(),
        || async {
            let client = XuanguBaoClient::new();
            let data = openatrading_core::api::xuangubao::fetch_index_trends(&client)
                .await
                .map_err(|e| {
                    tracing::error!("[replay] index trend error: {e}");
                    StatusCode::SERVICE_UNAVAILABLE
                })?;
            Ok(data)
        },
    )
    .await
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
    State(state): State<Arc<AppState>>,
    Query(q): Query<ReplayNewsQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let has_pagination = q.tailmark.is_some() || q.msg_id_mark.is_some();

    if !has_pagination {
        let cached = cache::get_or_fetch(
            &state.cache,
            "replay_news",
            CachePolicy::replay(),
            || async {
                let client = XuanguBaoClient::new();
                let (items, next_tail, next_msg) =
                    client.fetch_news_stream(None, None).await.map_err(|e| {
                        tracing::error!("[replay] news error: {e}");
                        StatusCode::SERVICE_UNAVAILABLE
                    })?;
                let resp = serde_json::json!({
                    "items": items,
                    "tailmark": next_tail,
                    "msgIdMark": next_msg,
                });
                Ok(resp.to_string())
            },
        )
        .await?;
        return Ok(Json(
            serde_json::from_str::<serde_json::Value>(&cached).unwrap_or_default(),
        ));
    }

    let client = XuanguBaoClient::new();
    let (items, next_tail, next_msg) = client
        .fetch_news_stream(q.tailmark, q.msg_id_mark.as_deref())
        .await
        .map_err(|e| {
            tracing::error!("[replay] news error: {e}");
            StatusCode::SERVICE_UNAVAILABLE
        })?;
    Ok(Json(serde_json::json!({
        "items": items,
        "tailmark": next_tail,
        "msgIdMark": next_msg,
    })))
}

/// GET /api/replay/snapshots — 5-min bucketed emotion snapshots
pub async fn get_replay_snapshots(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    cache::get_or_fetch_json(
        &state.cache,
        "replay_snapshots",
        CachePolicy::replay(),
        || async {
            let client = XuanguBaoClient::new();
            let events = client.fetch_all_day_events().await.map_err(|e| {
                tracing::error!("[replay] snapshots fetch error: {e}");
                StatusCode::SERVICE_UNAVAILABLE
            })?;
            let snaps = openatrading_core::api::xuangubao::build_replay_snapshots(&events);
            Ok(serde_json::to_string(&snaps).unwrap_or_default())
        },
    )
    .await
}

/// POST /api/replay/analyze — fetch data upfront, build into prompt, single SSE request.
pub async fn ai_replay_analyze(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    tracing::info!("[replay] POST /api/replay/analyze start");
    let llm = state.ctx.llm.clone();

    let date = XuanguBaoClient::last_n_trading_days(1)
        .first()
        .cloned()
        .unwrap_or_default();

    // Fetch snapshot data upfront
    let client = XuanguBaoClient::new();
    let events = client.fetch_all_day_events().await.map_err(|e| {
        tracing::error!("[replay] analyze fetch error: {e}");
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    let snapshots = openatrading_core::api::xuangubao::build_replay_snapshots(&events);

    // Build a compact data table for the prompt
    let mut data_table = String::from("时间    情绪 多  空  主线板块(前3)\n");
    for s in snapshots.iter().step_by(2) {
        let tops: Vec<String> = s
            .top_plates
            .iter()
            .take(3)
            .map(|p| format!("{}({:.1}%)", p.plate_name, p.pcp * 100.0))
            .collect();
        data_table.push_str(&format!(
            "{}  {:.0}   {:>3} {:>3}  {}\n",
            s.time_label,
            s.emotion_score,
            s.bull_count,
            s.bear_count,
            tops.join(" ")
        ));
    }

    let system_prompt =
        openatrading_core::prompts::build_replay_analysis_prompt(&date, &data_table);

    let messages = serde_json::json!([
        {"role": "user", "content": system_prompt}
    ]);

    let mut llm_stream = llm
        .chat_stream_with_messages(&serde_json::to_string(&messages).unwrap_or_default())
        .await
        .map_err(|e| {
            tracing::error!("[replay] analyze llm error: {e}");
            StatusCode::SERVICE_UNAVAILABLE
        })?;

    use futures::StreamExt;
    let stream = async_stream::stream! {
        let mut chunk_count = 0u32;
        while let Some(chunk) = llm_stream.next().await {
            match chunk {
                Ok(text) => {
                    if !text.is_empty() {
                        chunk_count += 1;
                        yield Ok::<_, std::convert::Infallible>(
                            axum::response::sse::Event::default().data(text)
                        );
                    }
                }
                Err(e) => {
                    yield Ok(axum::response::sse::Event::default().data(format!("[ERROR] {e}")));
                    tracing::error!("[replay] analyze stream error after {chunk_count} chunks: {e}");
                    break;
                }
            }
        }
        tracing::error!("[replay] analyze stream done: {chunk_count} chunks sent");
        yield Ok(axum::response::sse::Event::default().data("[DONE]"));
    };

    Ok(axum::response::Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("keepalive"),
    ))
}

/// GET /api/dragon-tiger/detail?page=1&page_size=20
#[derive(Deserialize)]
pub struct DragonTigerDetailQuery {
    #[serde(default = "default_page")]
    pub page: usize,
    #[serde(default = "default_page_size")]
    pub page_size: usize,
}

fn default_page() -> usize {
    1
}
fn default_page_size() -> usize {
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
                    dates.last().unwrap().as_str(),
                    dates.first().unwrap().as_str(),
                )
                .await
                .map_err(|e| {
                    tracing::error!("[dragon-tiger] detail fetch error: {:#}", e);
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
            },
            None => {
                merged.insert(key, item);
            },
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
    let flag = match query.flag.as_str() {
        "buy" => SeatFlag::Buy,
        "sell" => SeatFlag::Sell,
        _ => return Err(StatusCode::BAD_REQUEST),
    };
    let data = state
        .ctx
        .api
        .eastmoney
        .lhb_stock_seat(&query.code, &query.date, flag)
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    Ok(Json(data))
}

/// GET /api/search-stocks?q=keyword — search stocks by name or code
pub async fn search_stocks(
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let q = params
        .get("q")
        .map(|s| s.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if q.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "missing q"})),
        )
            .into_response();
    }
    let client = LhbClient::new();
    match client.search_stocks(&q).await {
        Ok(items) => (StatusCode::OK, Json(items)).into_response(),
        Err(e) => {
            tracing::error!("search_stocks: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "search failed"})),
            )
                .into_response()
        },
    }
}
