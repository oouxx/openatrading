# Gateway-Web 统一管理 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 web 服务器集成到 core crate，由 Gateway builder 模式统一管理生命周期，合并 docker-compose 为单文件。

**Architecture:** `crates/web/` move to `crates/core/src/web/`，Gateway struct 变为 builder 模式（`GatewayConfig` 按需组合 cron + web），`openatrading gateway run` 一个进程同时跑 cron scheduler + axum web server。

**Tech Stack:** axum 0.7, tower-http 0.5, async-stream 0.3, tokio (multi-thread + net)

---

### Task 1: 移动 web 代码到 core crate

**Files:**
- Create: `crates/core/src/web/mod.rs`
- Create: `crates/core/src/web/handlers.rs`
- Create: `crates/core/src/web/chat.rs`
- Create: `crates/core/src/web/download.rs`
- Modify: `crates/core/src/lib.rs`

（代码内容见下方各 Step）

- [ ] **Step 1: 创建 `crates/core/src/web/download.rs`**

从 `crates/web/src/main.rs` 提取 `serve_install_sh`、`serve_download`、`fetch_assets` handler 函数，改为接受 `Gateway` 参数或独立函数，不依赖 AppState。

```rust
use axum::{
    extract::Path,
    http::StatusCode,
    response::IntoResponse,
};

/// Proxy install.sh from GitHub
pub async fn serve_install_sh() -> Result<impl IntoResponse, StatusCode> {
    let url = "https://raw.githubusercontent.com/oouxx/openatrading/main/install.sh";
    let client = reqwest::Client::new();
    let mut req = client.get(url);
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        req = req.header("Authorization", format!("Bearer {token}"));
    }
    let resp = req.send().await.map_err(|_| StatusCode::BAD_GATEWAY)?;
    let status = resp.status();
    let body = resp.text().await.map_err(|_| StatusCode::BAD_GATEWAY)?;
    Ok((
        status,
        [("content-type", "text/plain; charset=utf-8")],
        body,
    ))
}

/// Fetch latest release asset list, cached for 5 min
async fn fetch_assets() -> Result<Vec<(i64, String)>, ()> {
    static CACHE: std::sync::OnceLock<(std::time::Instant, Vec<(i64, String)>)> =
        std::sync::OnceLock::new();

    let cache = CACHE.get();
    if let Some((t, assets)) = cache {
        if t.elapsed() < std::time::Duration::from_secs(300) {
            return Ok(assets.clone());
        }
    }

    let url = "https://api.github.com/repos/oouxx/openatrading/releases/latest";
    let client = reqwest::Client::new();
    let mut req = client.get(url).header("User-Agent", "openatrading-web");
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        req = req.header("Authorization", format!("Bearer {token}"));
    }
    let resp = req.send().await.map_err(|_| ())?;
    let json: serde_json::Value = resp.json().await.map_err(|_| ())?;

    let mut assets = Vec::new();
    if let Some(arr) = json["assets"].as_array() {
        for a in arr {
            if let (Some(id), Some(name)) = (a["id"].as_i64(), a["name"].as_str()) {
                assets.push((id, name.to_string()));
            }
        }
    }

    let _ = CACHE.set((std::time::Instant::now(), assets.clone()));
    Ok(assets)
}

/// Proxy download from GitHub Releases via asset API, fallback to local
pub async fn serve_download(Path(file): Path<String>) -> Result<impl IntoResponse, StatusCode> {
    // Try GitHub asset API
    if let Ok(assets) = fetch_assets().await {
        if let Some((asset_id, _)) = assets.iter().find(|(_, name)| name == &file) {
            let url = format!(
                "https://api.github.com/repos/oouxx/openatrading/releases/assets/{asset_id}"
            );
            let client = reqwest::Client::new();
            let mut req = client
                .get(&url)
                .header("Accept", "application/octet-stream")
                .header("User-Agent", "openatrading-web");
            if let Ok(token) = std::env::var("GITHUB_TOKEN") {
                req = req.header("Authorization", format!("Bearer {token}"));
            }
            if let Ok(resp) = req.send().await {
                if resp.status().is_success() {
                    let content_type = resp
                        .headers()
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("application/octet-stream")
                        .to_string();
                    let body = resp.bytes().await.map_err(|_| StatusCode::BAD_GATEWAY)?;
                    let response = axum::response::Response::builder()
                        .status(StatusCode::OK)
                        .header(axum::http::header::CONTENT_TYPE, content_type)
                        .body(axum::body::Body::from(body))
                        .unwrap();
                    return Ok(response);
                }
            }
        }
    }

    // Fallback to local downloads/
    let local_path = std::path::Path::new("downloads").join(&file);
    if local_path.exists() {
        let content_type = match local_path.extension().and_then(|e| e.to_str()) {
            Some("dmg") => "application/x-apple-diskimage",
            Some("deb") => "application/vnd.debian.binary-package",
            Some("AppImage") => "application/octet-stream",
            _ => "application/octet-stream",
        };
        let data = std::fs::read(&local_path).map_err(|_| StatusCode::NOT_FOUND)?;
        let response = axum::response::Response::builder()
            .status(StatusCode::OK)
            .header(axum::http::header::CONTENT_TYPE, content_type)
            .body(axum::body::Body::from(data))
            .unwrap();
        return Ok(response);
    }

    Err(StatusCode::NOT_FOUND)
}
```

- [ ] **Step 2: 创建 `crates/core/src/web/handlers.rs`**

从 `crates/web/src/main.rs` 提取所有 API handler（除 AI chat 和 download 外）。handler 增加可选的 `State<Arc<AppState>>` 参数来访问 memory/trade。

```rust
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

use super::AppState;

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

/// GET /api/overview
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
/// If AppState is available, reads from memory store instead of re-fetching
pub async fn get_sentiment_history(
    state: Option<State<Arc<AppState>>>,
) -> Result<impl IntoResponse, StatusCode> {
    let mut days: Vec<SentimentHistoryDay> = Vec::new();

    // Try memory store first
    if let Some(state) = &state {
        if let Ok(Some(history)) = state.memory.get_historical_context(10) {
            // Return cached snapshots in simplified format
            // (memory store provides raw JSON strings; parse into SentimentHistoryDay)
            for line in history.lines().take(10) {
                if let Ok(day) = serde_json::from_str::<SentimentHistoryDay>(line) {
                    days.push(day);
                }
            }
            if !days.is_empty() {
                return Ok(Json(days));
            }
        }
    }

    // Fallback to API
    let client = XuanguBaoClient::new();
    let trading_days = XuanguBaoClient::last_n_trading_days(10);

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
    pub count: i32,
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
    pub date: Option<String>,
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
    pub period: String,
    #[serde(default = "default_kline_count")]
    pub count: u32,
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

/// GET /api/telegraph (all categories)
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
pub async fn get_market_trend(
    Path(symbol): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
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

/// GET /api/plates-by-any-stock/:symbol
pub async fn get_plates_by_any_stock(
    Path(symbol): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let client = ApiClient::new();
    let plates = client
        .xuangubao
        .plates_by_any_stock(&symbol)
        .await
        .map_err(|e| {
            eprintln!("get_plates_by_any_stock: error: {}", e);
            StatusCode::SERVICE_UNAVAILABLE
        })?;
    Ok(Json(plates))
}
```

- [ ] **Step 3: 创建 `crates/core/src/web/chat.rs`**

从 `crates/web/src/main.rs` 提取 AI chat SSE handler。

```rust
use axum::{
    extract::State,
    http::StatusCode,
    response::{sse::Event, Sse},
    Json,
};
use futures::stream::Stream;
use serde::Deserialize;
use std::sync::Arc;

use crate::api::Client as ApiClient;
use crate::llm::openrouter::LlmClient;
use crate::roles;

use super::handlers::OverviewResponse;
use super::AppState;

#[derive(Deserialize)]
pub struct ChatRequest {
    pub message: String,
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

pub async fn ai_chat(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ChatRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>>, StatusCode> {
    let overview = fetch_overview_data(&state.api).await.map_err(|e| {
        eprintln!("ai_chat: overview error: {}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    let field_items = fetch_field_json(&state.api).await;
    let super_stocks = fetch_super_stock_json(&state.api).await;

    let strategy_text = roles::get_builtin_strategy("zarili")
        .unwrap_or("你是一个A股短线交易AI助手。");
    let system_prompt = build_system_prompt(strategy_text);
    let user_message =
        build_user_message(&overview, field_items.as_deref(), super_stocks.as_deref());

    let prompt = serde_json::json!([
        {"role": "system", "content": system_prompt},
        {"role": "user", "content": format!("{}\n\n用户提问：{}", user_message, req.message)}
    ])
    .to_string();

    let mut llm_stream = state
        .llm
        .chat_stream_with_messages(&prompt)
        .await
        .map_err(|e| {
            eprintln!("ai_chat: llm error: {}", e);
            StatusCode::SERVICE_UNAVAILABLE
        })?;

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
```

- [ ] **Step 4: 创建 `crates/core/src/web/mod.rs`**

Web 模块注册路由，`AppState` 持有 shared state。

```rust
pub mod chat;
pub mod download;
pub mod handlers;

use axum::{routing::get, Router};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use crate::api::Client as ApiClient;
use crate::llm::openrouter::LlmClient;
use crate::memory::sqlite::SqliteMemoryStore;
use crate::trade::sqlite::SqliteTradeManager;

pub struct AppState {
    pub llm: LlmClient,
    pub api: ApiClient,
    pub memory: Arc<SqliteMemoryStore>,
    pub trade: Arc<SqliteTradeManager>,
}

/// Build the axum Router for the web API
pub fn build_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/overview", get(handlers::get_overview))
        .route("/api/sentiment-history", get(handlers::get_sentiment_history))
        .route("/api/limit-up", get(handlers::get_limit_up))
        .route("/api/limit-down", get(handlers::get_limit_down))
        .route("/api/plate-abnormal", get(handlers::get_plate_abnormal))
        .route("/api/field-items", get(handlers::get_field_items))
        .route("/api/timeline", get(handlers::get_timeline))
        .route("/api/sentiment", get(handlers::get_sentiment))
        .route("/api/kline/:symbol", get(handlers::get_kline))
        .route("/api/realtime/:symbol", get(handlers::get_realtime))
        .route("/api/yesterday-limit-up", get(handlers::get_yesterday_limit_up))
        .route("/api/super-stock", get(handlers::get_super_stock))
        .route("/api/telegraph", get(handlers::get_telegraph_all))
        .route("/api/telegraph/:category", get(handlers::get_telegraph))
        .route("/api/dapan-abnormal", get(handlers::get_dapan_abnormal))
        .route("/api/market-trend/:symbol", get(handlers::get_market_trend))
        .route("/api/ai/chat", get(chat::ai_chat))
        .route(
            "/api/plates-by-any-stock/:symbol",
            get(handlers::get_plates_by_any_stock),
        )
        .route("/install.sh", get(download::serve_install_sh))
        .route("/downloads/:file", get(download::serve_download))
        .with_state(state)
        .layer(cors)
}
```

- [ ] **Step 5: 修改 `crates/core/src/lib.rs`，增加 `pub mod web;`**

```rust
pub mod agents;
pub mod api;
pub mod cron;
pub mod gateway;
pub mod llm;
pub mod memory;
pub mod report;
pub mod roles;
pub mod social;
pub mod templates;
pub mod trade;
pub mod utils;
pub mod web;
```

- [ ] **Step 6: 修改 `crates/core/Cargo.toml`，增加 web 依赖**

将 axum、tower-http、async-stream 从 `crates/web/Cargo.toml` 移入 core。

```toml
[dependencies]
# ... keep existing ...
axum = "0.7"
tower-http = { version = "0.5", features = ["cors"] }
async-stream = "0.3"
```

并在 workspace root `Cargo.toml` 的 `[workspace.dependencies]` 中无需变更（axum/tower-http/async-stream 不是 workspace deps，直接写在 core 即可）。

- [ ] **Step 7: 编译验证**

```bash
cargo build -p openatrading-core
```

预期：编译通过。

---

### Task 2: 重构 Gateway 为 Builder 模式

**Files:**
- Modify: `crates/core/src/gateway/mod.rs`

- [ ] **Step 1: 重写 `crates/core/src/gateway/mod.rs`**

```rust
use anyhow::Result;
use std::path::Path;
use std::sync::Arc;

use crate::cron::{self, CronConfig};
use crate::memory::sqlite::SqliteMemoryStore;
use crate::social::NotificationSender;
use crate::trade::sqlite::SqliteTradeManager;
use crate::web::{self, AppState};
use crate::api::Client as ApiClient;
use crate::llm::openrouter::LlmClient;

pub struct Gateway {
    pub memory: Arc<SqliteMemoryStore>,
    pub trade: Arc<SqliteTradeManager>,
}

pub struct GatewayConfig {
    pub cron: Option<(CronConfig, Arc<dyn NotificationSender>)>,
    pub web_port: Option<u16>,
}

impl Gateway {
    pub fn init(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let memory = Arc::new(SqliteMemoryStore::open(db_path)?);
        let trade = Arc::new(SqliteTradeManager::open(db_path)?);
        Ok(Gateway { memory, trade })
    }

    /// Start configured services, block until Ctrl+C
    pub async fn start(self, config: GatewayConfig) -> Result<()> {
        let mut handles = Vec::new();

        // ── Cron ──────────────────────────────
        if let Some((cron_config, social)) = config.cron {
            let scheduler = cron::start_scheduler(
                &cron_config,
                self.memory.clone(),
                self.trade.clone(),
                social,
            )
            .await?;
            handles.push(tokio::spawn(async move {
                scheduler.start().await.unwrap();
            }));
            eprintln!("[gateway] cron scheduler started");
        }

        // ── Web ───────────────────────────────
        if let Some(port) = config.web_port {
            let state = Arc::new(AppState {
                llm: LlmClient::from_env(),
                api: ApiClient::new(),
                memory: self.memory.clone(),
                trade: self.trade.clone(),
            });

            let router = web::build_router(state);
            let addr = format!("0.0.0.0:{}", port);
            let listener = tokio::net::TcpListener::bind(&addr).await?;
            println!("🌐 Web API: http://0.0.0.0:{}", port);

            handles.push(tokio::spawn(async move {
                axum::serve(listener, router).await.unwrap();
            }));
        }

        if handles.is_empty() {
            eprintln!("[gateway] nothing to run (no cron, no web)");
            return Ok(());
        }

        eprintln!("[gateway] running (Ctrl+C to stop)");

        // Wait for Ctrl+C
        tokio::signal::ctrl_c().await?;
        eprintln!("\n[gateway] shutting down...");

        for h in handles {
            h.abort();
        }

        Ok(())
    }
}
```

- [ ] **Step 2: 编译验证**

```bash
cargo build -p openatrading-core
```

预期：编译通过。

---

### Task 3: 更新 CLI 支持 gateway run 新参数

**Files:**
- Modify: `crates/cli/src/cli/app.rs`
- Modify: `crates/cli/src/cli/gateway_cmd.rs`
- Modify: `crates/cli/src/cli/mod.rs`

- [ ] **Step 1: 修改 `app.rs`，给 `GatewayAction::Run` 增加 flags**

```rust
#[derive(Subcommand)]
pub enum GatewayAction {
    /// 安装 systemd 服务
    Install,
    /// 卸载 systemd 服务
    Uninstall,
    /// 启动服务
    Start,
    /// 停止服务
    Stop,
    /// 查看服务状态
    Status,
    /// 前台运行（按需组合 cron + web）
    Run {
        /// 仅运行 cron scheduler
        #[arg(long)]
        cron_only: bool,
        /// 仅运行 web server
        #[arg(long)]
        web_only: bool,
    },
}
```

- [ ] **Step 2: 修改 `gateway_cmd.rs` 中 `cmd_gateway_run` 签名**

```rust
use crate::cli::config::AppConfig;
use openatrading_core::cron::CronConfig;
use openatrading_core::gateway::{Gateway, GatewayConfig};
use openatrading_core::social::{NoopSender, NotificationSender};
use openatrading_core::social::telegram::TelegramSender;

pub async fn cmd_gateway_run(
    project_root: &PathBuf,
    cron_only: bool,
    web_only: bool,
) -> Result<()> {
    let app_config = AppConfig::load(project_root)?;

    let db_path = project_root
        .join(".atrading")
        .join(&app_config.service.db_path);

    let gateway = Gateway::init(&db_path)?;

    let cron_config = if cron_only || !web_only {
        Some(match &app_config.cron {
            Some(c) => CronConfig {
                enabled: c.enabled,
                collection_schedule: c.collection_schedule.clone(),
                decision_schedule: c.decision_schedule.clone(),
            },
            None => CronConfig {
                enabled: true,
                collection_schedule: "0 */30 9-15 * * 1-5 *".to_string(),
                decision_schedule: "0 0 0 * * 1-5 *".to_string(),
            },
        })
    } else {
        None
    };

    let social: Arc<dyn NotificationSender> = if cron_config.is_some() {
        match &app_config.social {
            Some(sc) if sc.telegram.as_ref().map(|t| t.enabled).unwrap_or(false) => {
                let tg = sc.telegram.as_ref().unwrap();
                if tg.bot_token.as_ref().map(|t| t.is_empty()).unwrap_or(true)
                    || tg.chat_id.as_ref().map(|c| c.is_empty()).unwrap_or(true)
                {
                    Arc::new(NoopSender)
                } else {
                    Arc::new(TelegramSender {
                        bot_token: tg.bot_token.clone().unwrap_or_default(),
                        chat_id: tg.chat_id.clone().unwrap_or_default(),
                        notify_on_collect: tg.notify_on_collect,
                        notify_on_decision: tg.notify_on_decision,
                        notify_on_trade: tg.notify_on_trade,
                    })
                }
            }
            _ => Arc::new(NoopSender),
        }
    } else {
        Arc::new(NoopSender)
    };

    let cron_entry = cron_config.map(|c| (c, social));

    let web_port = if web_only || !cron_only {
        Some(8080u16)
    } else {
        None
    };

    let config = GatewayConfig {
        cron: cron_entry,
        web_port,
    };

    gateway.start(config).await
}
```

同时删除不再需要的原有 import（移除 `use openatrading_core::gateway::Gateway;` 的旧用法，清理 `use openatrading_core::social::telegram::TelegramSender;` 等无用导入的路径调整）。

- [ ] **Step 3: 修改 `mod.rs` 中 gateway run 的路由**

```rust
GatewayAction::Run { cron_only, web_only } => {
    cmd_gateway_run(&project_root, cron_only, web_only).await
}
```

- [ ] **Step 4: 编译验证**

```bash
cargo build -p openatrading-cli
```

预期：编译通过。

---

### Task 4: 删除 web crate 和清理 workspace

**Files:**
- Delete: `crates/web/` (整个目录)
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: 从 workspace members 移除 web crate**

```toml
[workspace]
resolver = "2"
members = [
    "crates/core",
    "crates/cli",
    "src-tauri",
]
```

- [ ] **Step 2: 删除 web crate**

```bash
rm -rf crates/web
```

- [ ] **Step 3: 编译验证全 workspace**

```bash
cargo build
```

预期：编译通过，`openatrading-web` 二进制不再存在。

---

### Task 5: 部署文件合并

**Files:**
- Modify: `docker-compose.yaml`
- Delete: `docker-compose-web.yaml`
- Delete: `Dockerfile.backend`
- Modify: `nginx.conf`
- Modify: `Dockerfile.frontend`
- Modify: `deploy.sh`
- Modify: `Makefile`

- [ ] **Step 1: 重写 `docker-compose.yaml`**

```yaml
services:
  gateway:
    image: ghcr.io/oouxx/openatrading:latest
    build:
      context: .
      dockerfile: Dockerfile
    ports:
      - "8080:8080"
    env_file:
      - .env
    environment:
      - RUST_BACKTRACE=1
      - TZ=Asia/Shanghai
    volumes:
      - ~/.atrading:/root/.atrading
      - ./downloads:/app/downloads
    restart: unless-stopped

  frontend:
    image: ghcr.io/oouxx/openatrading-frontend:latest
    build:
      context: .
      dockerfile: Dockerfile.frontend
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - /root/certs:/etc/ssl/cloudflare:ro
    depends_on:
      - gateway
    restart: unless-stopped
```

- [ ] **Step 2: 修改 `nginx.conf`，proxy_pass 用域名替代 env var**

```nginx
# HTTP -> HTTPS redirect
server {
    listen 80;
    server_name trending.96112901.xyz;
    return 301 https://$server_name$request_uri;
}

# HTTPS
server {
    listen 443 ssl;
    server_name trending.96112901.xyz;

    ssl_certificate /etc/ssl/cloudflare/cert.pem;
    ssl_certificate_key /etc/ssl/cloudflare/key.pem;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;

    gzip on;
    gzip_types text/css application/javascript application/json image/svg+xml text/plain;
    gzip_min_length 256;

    root /app/dist;
    index index.html;

    # API proxy -> gateway (always on trending.96112901.xyz:8080)
    location /api/ {
        proxy_pass http://trending.96112901.xyz:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_read_timeout 60s;
    }

    location /downloads/ {
        proxy_pass http://trending.96112901.xyz:8080;
        proxy_set_header Host $host;
    }
    location /install.sh {
        proxy_pass http://trending.96112901.xyz:8080;
        proxy_set_header Host $host;
    }

    location /assets/ {
        expires 1y;
        add_header Cache-Control "public, immutable";
    }

    location / {
        try_files $uri $uri/ /index.html;
    }
}
```

- [ ] **Step 3: 修改 `Dockerfile.frontend`，删除 `ENV BACKEND_HOST`**

将末尾：
```dockerfile
COPY nginx.conf /etc/nginx/templates/default.conf.template
EXPOSE 80
ENV BACKEND_HOST=backend
```

改为：
```dockerfile
COPY nginx.conf /etc/nginx/nginx.conf
EXPOSE 80
```

因为不再需要 env var 替换，直接 COPY 到 `/etc/nginx/nginx.conf`（标准路径）。

- [ ] **Step 4: 删除文件**

```bash
rm docker-compose-web.yaml
rm Dockerfile.backend
```

- [ ] **Step 5: 修改 `deploy.sh`，简化部署**

`deploy_web()` 改为只构建前端：
```bash
deploy_web() {
  echo ""
  echo "=== [web] Deploying frontend (mode: $MODE) ==="
  ssh $SSH_OPTS "$SERVER" "cd $REMOTE_DIR && docker compose up -d --build frontend"
}
```

`deploy_gateway()` 改为：
```bash
deploy_gateway() {
  echo ""
  echo "=== [gateway] Deploying gateway (mode: $MODE) ==="
  ssh $SSH_OPTS "$SERVER" "cd $REMOTE_DIR && docker compose up -d --build gateway"
}
```

`deploy` all 改为：
```bash
all)
  echo ""
  echo "=== [all] Deploying all services ==="
  ssh $SSH_OPTS "$SERVER" "cd $REMOTE_DIR && docker compose up -d --build"
  ;;
```

删除 `deploy` target 中对 `clone_or_pull`、`ghcr_login`、`build_downloads`、`update_system_cli` 等不再需要的调用的多余逻辑。保留 `clone_or_pull`、`build_downloads`、`update_system_cli` 函数但调整执行顺序为仅 gateway/all 时执行。

- [ ] **Step 6: 修改 `Makefile`**

`run-web` target：
```makefile
run-web:
	@lsof -ti:8080 | xargs kill -9 2>/dev/null || true
	cargo run -p openatrading-cli -- gateway run --web-only
```

`deploy-web` help text：
```makefile
	@echo "  deploy-web     deploy frontend only"
```

---

### Task 6: 全量编译 + 测试

- [ ] **Step 1: 全量编译**

```bash
cargo build
```

预期：全 workspace 编译通过，无 warning。

- [ ] **Step 2: 运行测试**

```bash
cargo test
```

预期：全部测试通过。

- [ ] **Step 3: Lint 检查**

```bash
cargo clippy --all-targets -- -D warnings
```

预期：无 clippy 警告。

- [ ] **Step 4: 验证 CLI help**

```bash
cargo run -p openatrading-cli -- gateway run --help
```

预期：显示 `--cron-only` 和 `--web-only` flags。

---

### Task 7: 清理和提交

- [ ] **Step 1: Commit**

```bash
git add -A
git commit -m "refactor: unify web into core, builder-pattern gateway, single docker-compose"
```
