use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Serialize;
use tower_http::cors::{Any, CorsLayer};

use trending_agent_core::agents::sentiment::analyze_sentiment;
use trending_agent_core::api::xuangubao::{KlinePeriod, XuanguBaoClient};
use trending_agent_core::api::Client as ApiClient;

// ── Response types ────────────────────────────────────

#[derive(Serialize)]
struct OverviewResponse {
    date: String,
    rise_count: u32,
    fall_count: u32,
    limit_up_count: u32,
    limit_down_count: u32,
    bomb_rate: f64,
    risk_level: String,
    decision_summary: String,
}

// ── Handlers ──────────────────────────────────────────

/// GET /api/overview — market overview + risk level + decision summary
async fn get_overview() -> Result<impl IntoResponse, StatusCode> {
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
    }))
}

/// GET /api/limit-up
async fn get_limit_up() -> Result<impl IntoResponse, StatusCode> {
    let client = XuanguBaoClient::new();
    let data = client.fetch_limit_up().await.map_err(|e| {
        eprintln!("limit_up error: {}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    Ok(Json(data))
}

/// GET /api/limit-down
async fn get_limit_down() -> Result<impl IntoResponse, StatusCode> {
    let client = XuanguBaoClient::new();
    let data = client.fetch_limit_down().await.map_err(|e| {
        eprintln!("limit_down error: {}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    Ok(Json(data))
}

/// GET /api/plate-abnormal?count=30
#[derive(serde::Deserialize)]
struct PlateAbnormalQuery {
    #[serde(default = "default_plate_count")]
    count: i32,
}
fn default_plate_count() -> i32 {
    30
}

async fn get_plate_abnormal(
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
struct FieldItemsQuery {
    date: Option<String>,
}

async fn get_field_items(
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
async fn get_timeline() -> Result<impl IntoResponse, StatusCode> {
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
async fn get_sentiment() -> Result<impl IntoResponse, StatusCode> {
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
struct KlineQuery {
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

async fn get_kline(
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
async fn get_realtime(Path(symbol): Path<String>) -> Result<impl IntoResponse, StatusCode> {
    let client = XuanguBaoClient::new();
    let data = client.fetch_realtime(&symbol).await.map_err(|e| {
        eprintln!("realtime error: {}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    Ok(Json(data))
}

/// GET /api/yesterday-limit-up
async fn get_yesterday_limit_up() -> Result<impl IntoResponse, StatusCode> {
    let client = XuanguBaoClient::new();
    let data = client.fetch_yesterday_limit_up().await.map_err(|e| {
        eprintln!("yesterday_limit_up error: {:#}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    Ok(Json(data))
}

/// GET /api/super-stock
async fn get_super_stock() -> Result<impl IntoResponse, StatusCode> {
    let client = XuanguBaoClient::new();
    let data = client.fetch_super_stock().await.map_err(|e| {
        eprintln!("super_stock error: {:#}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    Ok(Json(data))
}

/// GET /api/telegraph/:category
async fn get_telegraph(Path(category): Path<String>) -> Result<impl IntoResponse, StatusCode> {
    let client = ApiClient::new();
    let data = client.cls.fetch_telegraph(&category).await.map_err(|e| {
        eprintln!("telegraph error: {:#}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    Ok(Json(data))
}

/// GET /api/dapan-abnormal
async fn get_dapan_abnormal() -> Result<impl IntoResponse, StatusCode> {
    let client = ApiClient::new();
    let data = client.xuangubao.fetch_dapan_abnormal().await.map_err(|e| {
        eprintln!("dapan_abnormal error: {:#}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    Ok(Json(data))
}
/// GET /api/market-trend/:symbol
async fn get_market_trend(Path(symbol): Path<String>) -> Result<impl IntoResponse, StatusCode> {
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
// ── Server ────────────────────────────────────────────

async fn start_web_server(port: u16) {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/overview", get(get_overview))
        .route("/api/limit-up", get(get_limit_up))
        .route("/api/limit-down", get(get_limit_down))
        .route("/api/plate-abnormal", get(get_plate_abnormal))
        .route("/api/field-items", get(get_field_items))
        .route("/api/timeline", get(get_timeline))
        .route("/api/sentiment", get(get_sentiment))
        .route("/api/kline/:symbol", get(get_kline))
        .route("/api/realtime/:symbol", get(get_realtime))
        .route("/api/yesterday-limit-up", get(get_yesterday_limit_up))
        .route("/api/super-stock", get(get_super_stock))
        .route("/api/telegraph/:category", get(get_telegraph))
        .route("/api/dapan-abnormal", get(get_dapan_abnormal))
        .route("/api/market-trend/:symbol", get(get_market_trend))
        .layer(cors);

    let addr = format!("0.0.0.0:{}", port);
    println!("🌐 Web API: http://localhost:{}", port);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    start_web_server(8080).await;
    Ok(())
}
