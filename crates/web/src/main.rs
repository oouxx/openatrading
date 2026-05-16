use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{sse::Event, IntoResponse, Sse},
    routing::{get, post},
    Json, Router,
};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use trending_agent_core::agents::sentiment::analyze_sentiment;
use trending_agent_core::api::xuangubao::{KlinePeriod, XuanguBaoClient};
use trending_agent_core::api::Client as ApiClient;
use trending_agent_core::llm::openrouter::LlmClient;

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
    market_temperature: f64,
    yesterday_limit_up_avg_pcp: f64,
}

#[derive(Serialize)]
struct SentimentHistoryDay {
    date: String,
    sentiment: f64,
    limit_up_count: u32,
    bomb_rate: f64,
    rise_ratio: f64,
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
        market_temperature: overview.market_temperature,
        yesterday_limit_up_avg_pcp: overview.yesterday_limit_up_avg_pcp,
    }))
}

/// GET /api/sentiment-history
async fn get_sentiment_history() -> Result<impl IntoResponse, StatusCode> {
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

/// GET /api/telegraph — all categories
async fn get_telegraph_all() -> Result<impl IntoResponse, StatusCode> {
    let client = ApiClient::new();
    let data = client.cls.fetch_telegraph("").await.map_err(|e| {
        eprintln!("telegraph error: {:#}", e);
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
// ── AI Chat (SSE) ─────────────────────────────────────

#[derive(Deserialize)]
struct ChatRequest {
    message: String,
}

struct AppState {
    llm: LlmClient,
    api: ApiClient,
}

fn build_system_prompt(strategy_text: &str) -> String {
    strategy_text.to_string()
}

fn build_user_message(
    overview: &OverviewResponse,
    field_items: Option<&str>,
    super_stocks: Option<&str>,
) -> String {
    let field_section = field_items
        .map(|s| format!("\n## 韭研公社异动数据\n{s}\n"))
        .unwrap_or_default();

    let super_section = super_stocks
        .map(|s| format!("\n## 强势股数据（未涨停但强）\n{s}\n"))
        .unwrap_or_default();

    format!(
        r#"## 当前市场数据
- 日期：{date}
- 上涨家数：{rise} 家
- 下跌家数：{fall} 家
- 涨停家数：{limit_up} 只
- 跌停家数：{limit_down} 只
- 炸板率：{bomb_rate:.1}%
- 涨跌比：{ratio}
- 风险等级：{risk_level}
- 市场摘要：{summary}
{field_items}{super_stocks}
请在回答时结合以上实时市场数据，帮助用户理解当前市场状态。"#,
        date = overview.date,
        rise = overview.rise_count,
        fall = overview.fall_count,
        limit_up = overview.limit_up_count,
        limit_down = overview.limit_down_count,
        bomb_rate = overview.bomb_rate,
        ratio = format!(
            "{:.2}",
            overview.rise_count as f64 / overview.fall_count.max(1) as f64
        ),
        risk_level = overview.risk_level,
        summary = overview.decision_summary,
        field_items = field_section,
        super_stocks = super_section,
    )
}

async fn ai_chat(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ChatRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>>, StatusCode> {
    // Fetch market overview for system prompt
    let overview = fetch_overview_data(&state.api).await.map_err(|e| {
        eprintln!("ai_chat: overview error: {}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    // Fetch field items (韭研公社异动) — full JSON, skip on failure
    let field_items = fetch_field_json(&state.api).await;

    // Fetch super stocks (强势股) — full JSON, skip on failure
    let super_stocks = fetch_super_stock_json(&state.api).await;

    // Web 端暂使用内置 zarili 策略（二进制嵌入）
    let strategy_text = trending_agent_core::roles::get_builtin_strategy("zarili")
        .unwrap_or("你是一个A股短线交易AI助手。");
    let system_prompt = build_system_prompt(strategy_text);
    let user_message =
        build_user_message(&overview, field_items.as_deref(), super_stocks.as_deref());

    // Build messages array: system + user
    let prompt = serde_json::json!([
        {"role": "system", "content": system_prompt},
        {"role": "user", "content": format!("{}\n\n用户提问：{}", user_message, req.message)}
    ])
    .to_string();

    // Call LLM streaming
    let mut llm_stream = state
        .llm
        .chat_stream_with_messages(&prompt)
        .await
        .map_err(|e| {
            eprintln!("ai_chat: llm error: {}", e);
            StatusCode::SERVICE_UNAVAILABLE
        })?;

    // Convert to SSE stream
    use futures::StreamExt;

    let sse_stream = async_stream::stream! {
        while let Some(chunk) = llm_stream.next().await {
            match chunk {
                Ok(text) => {
                    if !text.is_empty() {
                        yield Ok(Event::default().data(text));
                    }
                }
                Err(e) => {
                    yield Ok(Event::default().data(format!("[ERROR] {}", e)));
                }
            }
        }
        yield Ok(Event::default().data("[DONE]"));
    };

    Ok(Sse::new(sse_stream))
}

async fn fetch_field_json(api: &ApiClient) -> Option<String> {
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    match api.jiuyan.fetch_field(&date, 1).await {
        Ok(plates) if !plates.is_empty() => serde_json::to_string(&plates).ok(),
        _ => None,
    }
}

async fn fetch_super_stock_json(api: &ApiClient) -> Option<String> {
    match api.xuangubao.fetch_super_stock().await {
        Ok(stocks) if !stocks.is_empty() => serde_json::to_string(&stocks).ok(),
        _ => None,
    }
}

async fn fetch_overview_data(api: &ApiClient) -> anyhow::Result<OverviewResponse> {
    let overview = api.xuangubao.fetch_market_overview().await?;

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

    Ok(OverviewResponse {
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
    })
}
async fn get_plates_by_any_stock(
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

/// Proxy install.sh from GitHub
async fn serve_install_sh() -> Result<impl IntoResponse, StatusCode> {
    let url = "https://raw.githubusercontent.com/oouxx/openatrading/main/install.sh";
    let client = reqwest::Client::new();
    let mut req = client.get(url);
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        req = req.header("Authorization", format!("Bearer {token}"));
    }
    let resp = req.send().await.map_err(|_| StatusCode::BAD_GATEWAY)?;
    let body = resp.text().await.map_err(|_| StatusCode::BAD_GATEWAY)?;
    Ok((
        StatusCode::OK,
        [("content-type", "text/plain; charset=utf-8")],
        body,
    ))
}

/// Proxy download from GitHub Releases
async fn serve_download(Path(file): Path<String>) -> Result<impl IntoResponse, StatusCode> {
    let url = format!("https://github.com/oouxx/openatrading/releases/latest/download/{file}");

    let client = reqwest::Client::new();
    let mut req = client
        .get(&url)
        .header("Accept", "application/octet-stream");
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        req = req.header("Authorization", format!("Bearer {token}"));
    }
    let resp = req.send().await.map_err(|_| StatusCode::BAD_GATEWAY)?;

    let status = resp.status();
    if !status.is_success() {
        return Err(StatusCode::NOT_FOUND);
    }

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    let body = resp.bytes().await.map_err(|_| StatusCode::BAD_GATEWAY)?;

    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, content_type)],
        body,
    ))
}

// ── Server ────────────────────────────────────────────

async fn start_web_server(port: u16) {
    let state = Arc::new(AppState {
        llm: LlmClient::from_env(),
        api: ApiClient::new(),
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/overview", get(get_overview))
        .route("/api/sentiment-history", get(get_sentiment_history))
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
        .route("/api/telegraph", get(get_telegraph_all))
        .route("/api/telegraph/:category", get(get_telegraph))
        .route("/api/dapan-abnormal", get(get_dapan_abnormal))
        .route("/api/market-trend/:symbol", get(get_market_trend))
        .route("/api/ai/chat", post(ai_chat))
        .route(
            "/api/plates-by-any-stock/:symbol",
            get(get_plates_by_any_stock),
        )
        // Proxy downloads from GitHub Releases
        .route("/install.sh", get(serve_install_sh))
        .route("/downloads/:file", get(serve_download))
        .with_state(state)
        .layer(cors);

    let addr = format!("0.0.0.0:{}", port);
    println!("🌐 Web API: http://0.0.0.0:{}", port);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    start_web_server(8080).await;
    Ok(())
}
