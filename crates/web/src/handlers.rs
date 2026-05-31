use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use trading_agent_core::api::xuangubao::{KlinePeriod, XuanguBaoClient};
use trading_agent_core::api::Client as ApiClient;
use trading_agent_core::trade::TradeManager;
use trading_agent_core::{agents::sentiment::analyze_sentiment, api::utils::last_n_trading_days};

use crate::cache::{self, CachePolicy};
use crate::{AppState, RoleEntry};
use trading_agent_core::roles::pnl::compute_pnl_history;

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
    pub points: Vec<trading_agent_core::roles::pnl::PnlPoint>,
    pub initial_capital: f64,
}

// ── Handlers ──────────────────────────────────────────

/// GET /api/overview — market overview + risk level + decision summary
pub async fn get_overview(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    let cached = cache::get_or_fetch(
        &state.cache,
        "overview",
        CachePolicy::overview(),
        || async {
            let client = XuanguBaoClient::new();
            let overview = client.fetch_market_overview().await.map_err(|e| {
                eprintln!("overview error: {}", e);
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
                date: trading_agent_core::utils::time::beijing_now()
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

            Ok(serde_json::to_string(&resp).unwrap_or_default())
        },
    )
    .await?;

    Ok(Json(
        serde_json::from_str::<serde_json::Value>(&cached).unwrap_or_default(),
    ))
}

/// GET /api/sentiment-history
pub async fn get_sentiment_history(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    let cached = cache::get_or_fetch(
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
                    }
                    Ok(None) => {
                        eprintln!("sentiment_history: no data for {date}");
                    }
                    Err(e) => {
                        eprintln!("sentiment_history error for {date}: {e}");
                    }
                }
            }

            days.reverse();
            Ok(serde_json::to_string(&days).unwrap_or_default())
        },
    )
    .await?;

    Ok(Json(
        serde_json::from_str::<serde_json::Value>(&cached).unwrap_or_default(),
    ))
}

/// GET /api/limit-up
pub async fn get_limit_up(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    let cached = cache::get_or_fetch(
        &state.cache,
        "limit_up",
        CachePolicy::snapshot(),
        || async {
            let client = XuanguBaoClient::new();
            let data = client.fetch_limit_up().await.map_err(|e| {
                eprintln!("limit_up error: {}", e);
                StatusCode::SERVICE_UNAVAILABLE
            })?;
            Ok(serde_json::to_string(&data).unwrap_or_default())
        },
    )
    .await?;
    Ok(Json(
        serde_json::from_str::<serde_json::Value>(&cached).unwrap_or_default(),
    ))
}

/// GET /api/limit-down
pub async fn get_limit_down(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    let cached = cache::get_or_fetch(
        &state.cache,
        "limit_down",
        CachePolicy::snapshot(),
        || async {
            let client = XuanguBaoClient::new();
            let data = client.fetch_limit_down().await.map_err(|e| {
                eprintln!("limit_down error: {}", e);
                StatusCode::SERVICE_UNAVAILABLE
            })?;
            Ok(serde_json::to_string(&data).unwrap_or_default())
        },
    )
    .await?;
    Ok(Json(
        serde_json::from_str::<serde_json::Value>(&cached).unwrap_or_default(),
    ))
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
    let cached = cache::get_or_fetch(
        &state.cache,
        &cache_key,
        CachePolicy::snapshot(),
        || async {
            let client = XuanguBaoClient::new();
            let data = client.fetch_plate_abnormal(q.count).await.map_err(|e| {
                eprintln!("plate_abnormal error: {}", e);
                StatusCode::SERVICE_UNAVAILABLE
            })?;
            Ok(serde_json::to_string(&data).unwrap_or_default())
        },
    )
    .await?;
    Ok(Json(
        serde_json::from_str::<serde_json::Value>(&cached).unwrap_or_default(),
    ))
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
        trading_agent_core::utils::time::beijing_now()
            .format("%Y-%m-%d")
            .to_string()
    });
    let cache_key = format!("field_items_{}", date);
    let cached = cache::get_or_fetch(&state.cache, &cache_key, CachePolicy::daily(), || async {
        let client = ApiClient::new();
        let data = client.jiuyan.fetch_field(&date, 1).await.map_err(|e| {
            eprintln!("field_items error: {}", e);
            StatusCode::SERVICE_UNAVAILABLE
        })?;
        Ok(serde_json::to_string(&data).unwrap_or_default())
    })
    .await?;
    Ok(Json(
        serde_json::from_str::<serde_json::Value>(&cached).unwrap_or_default(),
    ))
}

/// GET /api/timeline
pub async fn get_timeline(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    if std::env::var("JIUYAN_PHONE").is_err() {
        return Ok(Json(serde_json::json!([])));
    }
    let month = trading_agent_core::utils::time::beijing_now()
        .format("%Y-%m")
        .to_string();
    let cache_key = format!("timeline_{}", month);
    let cached = cache::get_or_fetch(&state.cache, &cache_key, CachePolicy::daily(), || async {
        let client = ApiClient::new();
        let data = client.jiuyan.fetch_timeline(&month).await.map_err(|e| {
            eprintln!("timeline error: {}", e);
            StatusCode::SERVICE_UNAVAILABLE
        })?;
        Ok(serde_json::to_string(&data).unwrap_or_default())
    })
    .await?;
    Ok(Json(
        serde_json::from_str::<serde_json::Value>(&cached).unwrap_or_default(),
    ))
}

/// GET /api/sentiment
pub async fn get_sentiment(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    if std::env::var("JIUYAN_PHONE").is_err() {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    let cached = cache::get_or_fetch(
        &state.cache,
        "sentiment",
        CachePolicy::snapshot(),
        || async {
            let client = ApiClient::new();
            let data = analyze_sentiment(&client).await.map_err(|e| {
                eprintln!("sentiment error: {}", e);
                StatusCode::SERVICE_UNAVAILABLE
            })?;
            Ok(serde_json::to_string(&data).unwrap_or_default())
        },
    )
    .await?;
    Ok(Json(
        serde_json::from_str::<serde_json::Value>(&cached).unwrap_or_default(),
    ))
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
    let cached = cache::get_or_fetch(&state.cache, &cache_key, CachePolicy::daily(), || async {
        let client = XuanguBaoClient::new();
        let data = client
            .fetch_kline(&symbol, period, q.count)
            .await
            .map_err(|e| {
                eprintln!("kline error: {}", e);
                StatusCode::SERVICE_UNAVAILABLE
            })?;
        Ok(serde_json::to_string(&data).unwrap_or_default())
    })
    .await?;
    Ok(Json(
        serde_json::from_str::<serde_json::Value>(&cached).unwrap_or_default(),
    ))
}

/// GET /api/realtime/:symbol
pub async fn get_realtime(
    State(state): State<Arc<AppState>>,
    Path(symbol): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let cache_key = format!("realtime_{}", symbol);
    let cached = cache::get_or_fetch(
        &state.cache,
        &cache_key,
        CachePolicy::realtime(),
        || async {
            let client = XuanguBaoClient::new();
            let data = client.fetch_realtime(&symbol).await.map_err(|e| {
                eprintln!("realtime error: {}", e);
                StatusCode::SERVICE_UNAVAILABLE
            })?;
            Ok(serde_json::to_string(&data).unwrap_or_default())
        },
    )
    .await?;
    Ok(Json(
        serde_json::from_str::<serde_json::Value>(&cached).unwrap_or_default(),
    ))
}

/// GET /api/yesterday-limit-up
pub async fn get_yesterday_limit_up(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    let cached = cache::get_or_fetch(
        &state.cache,
        "yesterday_limit_up",
        CachePolicy::snapshot(),
        || async {
            let client = XuanguBaoClient::new();
            let data = client.fetch_yesterday_limit_up().await.map_err(|e| {
                eprintln!("yesterday_limit_up error: {:#}", e);
                StatusCode::SERVICE_UNAVAILABLE
            })?;
            Ok(serde_json::to_string(&data).unwrap_or_default())
        },
    )
    .await?;
    Ok(Json(
        serde_json::from_str::<serde_json::Value>(&cached).unwrap_or_default(),
    ))
}

/// GET /api/super-stock
pub async fn get_super_stock(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    let cached = cache::get_or_fetch(
        &state.cache,
        "super_stock",
        CachePolicy::snapshot(),
        || async {
            let client = XuanguBaoClient::new();
            let data = client.fetch_super_stock().await.map_err(|e| {
                eprintln!("super_stock error: {:#}", e);
                StatusCode::SERVICE_UNAVAILABLE
            })?;
            Ok(serde_json::to_string(&data).unwrap_or_default())
        },
    )
    .await?;
    Ok(Json(
        serde_json::from_str::<serde_json::Value>(&cached).unwrap_or_default(),
    ))
}

/// GET /api/index-quotes
pub async fn get_index_quotes(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    let cached = cache::get_or_fetch(
        &state.cache,
        "index_quotes",
        CachePolicy::realtime(),
        || async {
            let client = XuanguBaoClient::new();
            let data = client
                .fetch_market_real(&["000001.SS", "399001.SZ", "399006.SZ"])
                .await
                .map_err(|e| {
                    eprintln!("index_quotes error: {}", e);
                    StatusCode::SERVICE_UNAVAILABLE
                })?;
            Ok(serde_json::to_string(&data).unwrap_or_default())
        },
    )
    .await?;
    Ok(Json(
        serde_json::from_str::<serde_json::Value>(&cached).unwrap_or_default(),
    ))
}

/// GET /api/telegraph — all categories
pub async fn get_telegraph_all(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    let cached = cache::get_or_fetch(
        &state.cache,
        "telegraph_all",
        CachePolicy::telegraph(),
        || async {
            let client = ApiClient::new();
            let data = client.cls.fetch_telegraph("", None, 20).await.map_err(|e| {
                eprintln!("[telegraph] fetch failed: {:#}", e);
                eprintln!("[telegraph] error chain:");
                for cause in e.chain() {
                    eprintln!("[telegraph]   caused by: {}", cause);
                }
                StatusCode::SERVICE_UNAVAILABLE
            })?;
            Ok(serde_json::to_string(&data).unwrap_or_default())
        },
    )
    .await?;
    Ok(Json(
        serde_json::from_str::<serde_json::Value>(&cached).unwrap_or_default(),
    ))
}

/// GET /api/telegraph/:category
pub async fn get_telegraph(
    State(state): State<Arc<AppState>>,
    Path(category): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let cache_key = format!("telegraph_{}", category);
    let cached = cache::get_or_fetch(
        &state.cache,
        &cache_key,
        CachePolicy::telegraph(),
        || async {
            let client = ApiClient::new();
            let data = client.cls.fetch_telegraph(&category, None, 20).await.map_err(|e| {
                eprintln!("[telegraph/{}] fetch failed: {:#}", category, e);
                StatusCode::SERVICE_UNAVAILABLE
            })?;
            Ok(serde_json::to_string(&data).unwrap_or_default())
        },
    )
    .await?;
    Ok(Json(
        serde_json::from_str::<serde_json::Value>(&cached).unwrap_or_default(),
    ))
}

/// GET /api/dapan-abnormal
pub async fn get_dapan_abnormal(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    let cached = cache::get_or_fetch(
        &state.cache,
        "dapan_abnormal",
        CachePolicy::snapshot(),
        || async {
            let client = ApiClient::new();
            let data = client.xuangubao.fetch_dapan_abnormal().await.map_err(|e| {
                eprintln!("dapan_abnormal error: {:#}", e);
                StatusCode::SERVICE_UNAVAILABLE
            })?;
            Ok(serde_json::to_string(&data).unwrap_or_default())
        },
    )
    .await?;
    Ok(Json(
        serde_json::from_str::<serde_json::Value>(&cached).unwrap_or_default(),
    ))
}

/// GET /api/market-trend/:symbol
pub async fn get_market_trend(
    State(state): State<Arc<AppState>>,
    Path(symbol): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let cache_key = format!("market_trend_{}", symbol);
    let cached = cache::get_or_fetch(
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
                    eprintln!("market_trend error: {:#}", e);
                    StatusCode::SERVICE_UNAVAILABLE
                })?;
            Ok(serde_json::to_string(&data).unwrap_or_default())
        },
    )
    .await?;
    Ok(Json(
        serde_json::from_str::<serde_json::Value>(&cached).unwrap_or_default(),
    ))
}

pub async fn get_plates_by_any_stock(
    State(state): State<Arc<AppState>>,
    Path(symbol): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let cache_key = format!("plates_{}", symbol);
    let cached = cache::get_or_fetch(
        &state.cache,
        &cache_key,
        CachePolicy::snapshot(),
        || async {
            let plates = state
                .api
                .xuangubao
                .plates_by_any_stock(&symbol)
                .await
                .map_err(|e| {
                    eprintln!("get_plates_by_any_stock: error: {}", e);
                    StatusCode::SERVICE_UNAVAILABLE
                })?;
            Ok(serde_json::to_string(&plates).unwrap_or_default())
        },
    )
    .await?;
    Ok(Json(
        serde_json::from_str::<serde_json::Value>(&cached).unwrap_or_default(),
    ))
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
    trading_agent_core::roles::save_role_strategy(
        &state.strategies_dir,
        &body.name,
        &body.strategy,
    )
    .map_err(|e| {
        eprintln!("create_role error: {}", e);
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
    trading_agent_core::roles::save_role_strategy(&state.strategies_dir, &name, &body.strategy)
        .map_err(|e| {
            eprintln!("update_role_strategy error: {}", e);
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
    state.trade.delete_positions_by_role(&name).ok();
    state.trade.delete_trade_logs_by_role(&name).ok();
    trading_agent_core::roles::delete_role_dir(&state.strategies_dir, &name).ok();
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
        .trade
        .get_closed_positions(&name, limit)
        .map_err(|e| {
            eprintln!("get_role_history error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(positions))
}

// ── Replay handlers ────────────────────────────────────

/// GET /api/replay/events — fetch all events for latest trading day
pub async fn get_replay_events(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    let cached = cache::get_or_fetch(
        &state.cache,
        "replay_events",
        CachePolicy::replay(),
        || async {
            let client = XuanguBaoClient::new();
            let all_events = client.fetch_all_day_events().await.map_err(|e| {
                eprintln!("[replay] events error: {:#}", e);
                StatusCode::SERVICE_UNAVAILABLE
            })?;
            let events: Vec<trading_agent_core::api::xuangubao::ReplayEvent> = all_events
                .iter()
                .map(trading_agent_core::api::xuangubao::ReplayEvent::from)
                .collect();
            let date = XuanguBaoClient::last_n_trading_days(1)
                .first()
                .cloned()
                .unwrap_or_default();
            let page = trading_agent_core::api::xuangubao::ReplayPage {
                date,
                market_phase: String::new(),
                events,
                next_cursor: None,
            };
            Ok(serde_json::to_string(&page).unwrap_or_default())
        },
    )
    .await?;
    Ok(Json(
        serde_json::from_str::<serde_json::Value>(&cached).unwrap_or_default(),
    ))
}

/// GET /api/replay/index — three-index trend data
pub async fn get_replay_index(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    let cached = cache::get_or_fetch(
        &state.cache,
        "replay_index",
        CachePolicy::replay(),
        || async {
            let client = XuanguBaoClient::new();
            let data = trading_agent_core::api::xuangubao::fetch_index_trends(&client)
                .await
                .map_err(|e| {
                    eprintln!("[replay] index trend error: {e}");
                    StatusCode::SERVICE_UNAVAILABLE
                })?;
            Ok(serde_json::to_string(&data).unwrap_or_default())
        },
    )
    .await?;

    Ok(Json(
        serde_json::from_str::<serde_json::Value>(&cached).unwrap_or_default(),
    ))
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
                        eprintln!("[replay] news error: {e}");
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
            eprintln!("[replay] news error: {e}");
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
    let cached = cache::get_or_fetch(
        &state.cache,
        "replay_snapshots",
        CachePolicy::replay(),
        || async {
            let client = XuanguBaoClient::new();
            let events = client.fetch_all_day_events().await.map_err(|e| {
                eprintln!("[replay] snapshots fetch error: {e}");
                StatusCode::SERVICE_UNAVAILABLE
            })?;
            let snaps = trading_agent_core::api::xuangubao::build_replay_snapshots(&events);
            Ok(serde_json::to_string(&snaps).unwrap_or_default())
        },
    )
    .await?;
    Ok(Json(
        serde_json::from_str::<serde_json::Value>(&cached).unwrap_or_default(),
    ))
}

/// POST /api/replay/analyze — fetch data upfront, build into prompt, single SSE request.
pub async fn ai_replay_analyze(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    eprintln!("[replay] POST /api/replay/analyze start");
    let llm = state.llm.clone();

    let date = XuanguBaoClient::last_n_trading_days(1)
        .first()
        .cloned()
        .unwrap_or_default();

    // Fetch snapshot data upfront
    let client = XuanguBaoClient::new();
    let events = client.fetch_all_day_events().await.map_err(|e| {
        eprintln!("[replay] analyze fetch error: {e}");
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    let snapshots = trading_agent_core::api::xuangubao::build_replay_snapshots(&events);

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

    let system_prompt = format!(
        r#"你是A股短线复盘专家。以下是{date}全天5分钟级别行情快照数据：

{data_table}

请基于以上数据输出结构化复盘报告，严格遵循以下格式：

## 一、情绪节奏
- 开盘情绪（09:30-10:00）：情绪温度变化趋势，多空比特征
- 盘中变化（10:00-14:00）：关键转折点及时间
- 尾盘走势（14:00-15:00）：收盘前情绪动向
- 全天趋势判断：单边多/单边空/震荡/反转

## 二、主线板块
- 开盘主线（前30分钟最强板块）
- 盘中切换（如有新板块崛起，时间及原因）
- 尾盘赢家（全天累计涨幅最大板块）
- 主线持续性评价

## 三、关键节点
- 列出2-3个情绪突变时间点，分析可能原因

## 四、轮动特征
- 资金流向判断
- 高低切换迹象

## 五、复盘结论
- 日内最优策略
- 次日关注方向
- 主要风险提示

格式要求：纯 Markdown，禁止 emoji/字符画/Unicode 图表，数据引用用数字。"#,
        date = date,
        data_table = data_table,
    );

    let messages = serde_json::json!([
        {"role": "user", "content": system_prompt}
    ]);

    let mut llm_stream = llm
        .chat_stream_with_messages(&serde_json::to_string(&messages).unwrap_or_default())
        .await
        .map_err(|e| {
            eprintln!("[replay] analyze llm error: {e}");
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
                    eprintln!("[replay] analyze stream error after {chunk_count} chunks: {e}");
                    break;
                }
            }
        }
        eprintln!("[replay] analyze stream done: {chunk_count} chunks sent");
        yield Ok(axum::response::sse::Event::default().data("[DONE]"));
    };

    Ok(axum::response::Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("keepalive"),
    ))
}

/// GET /api/dragon-tiger
pub async fn get_dragon_tiger(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    let cached = cache::get_or_fetch(
        &state.cache,
        "dragon_tiger",
        CachePolicy::daily(),
        || async {
            let data = state
                .api
                .eastmoney
                .lhb_detail(
                    last_n_trading_days(1)[0].as_str(),
                    last_n_trading_days(1)[0].as_str(),
                )
                .await
                .map_err(|e| {
                    eprintln!("[dragon-tiger] fetch error: {:#}", e);
                    StatusCode::SERVICE_UNAVAILABLE
                })?;
            Ok(serde_json::to_string(&data).unwrap_or_default())
        },
    )
    .await?;
    Ok(Json(
        serde_json::from_str::<serde_json::Value>(&cached).unwrap_or_default(),
    ))
}

/// GET /api/dragon-tiger/combined
pub async fn get_dragon_tiger_combined(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    let cached = cache::get_or_fetch(
        &state.cache,
        "dragon_tiger_combined",
        CachePolicy::daily(),
        || async {
            let date = last_n_trading_days(1)[0].clone();
            let data = state
                .api
                .eastmoney
                .lhb_combined(&date, &date)
                .await
                .map_err(|e| {
                    eprintln!("[dragon-tiger] combined fetch error: {:#}", e);
                    StatusCode::SERVICE_UNAVAILABLE
                })?;
            Ok(serde_json::to_string(&data).unwrap_or_default())
        },
    )
    .await?;
    Ok(Json(
        serde_json::from_str::<serde_json::Value>(&cached).unwrap_or_default(),
    ))
}
