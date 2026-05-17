pub mod chat;
pub mod download;
pub mod handlers;

use serde::Serialize;
use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};
use tower_http::cors::{Any, CorsLayer};

use crate::api::Client as ApiClient;
use crate::llm::openrouter::LlmClient;
use crate::memory::sqlite::SqliteMemoryStore;
use crate::trade::sqlite::SqliteTradeManager;

#[derive(Debug, Clone, Serialize)]
pub struct RoleEntry {
    pub name: String,
    pub display_name: String,
    pub strategy: String,
}

pub struct AppState {
    pub llm: LlmClient,
    pub api: ApiClient,
    pub memory: Arc<SqliteMemoryStore>,
    pub trade: Arc<SqliteTradeManager>,
    pub roles: Vec<RoleEntry>,
}

pub fn build_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/overview", get(handlers::get_overview))
        .route(
            "/api/sentiment-history",
            get(handlers::get_sentiment_history),
        )
        .route("/api/limit-up", get(handlers::get_limit_up))
        .route("/api/limit-down", get(handlers::get_limit_down))
        .route("/api/plate-abnormal", get(handlers::get_plate_abnormal))
        .route("/api/field-items", get(handlers::get_field_items))
        .route("/api/timeline", get(handlers::get_timeline))
        .route("/api/sentiment", get(handlers::get_sentiment))
        .route("/api/kline/:symbol", get(handlers::get_kline))
        .route("/api/realtime/:symbol", get(handlers::get_realtime))
        .route(
            "/api/yesterday-limit-up",
            get(handlers::get_yesterday_limit_up),
        )
        .route("/api/super-stock", get(handlers::get_super_stock))
        .route("/api/telegraph", get(handlers::get_telegraph_all))
        .route("/api/telegraph/:category", get(handlers::get_telegraph))
        .route("/api/dapan-abnormal", get(handlers::get_dapan_abnormal))
        .route("/api/market-trend/:symbol", get(handlers::get_market_trend))
        .route("/api/ai/chat", post(chat::ai_chat))
        .route(
            "/api/plates-by-any-stock/:symbol",
            get(handlers::get_plates_by_any_stock),
        )
        .route("/api/roles", get(handlers::get_roles))
        .route(
            "/api/roles/:name/positions",
            get(handlers::get_role_positions),
        )
        .route("/api/roles/:name/logs", get(handlers::get_role_logs))
        // Proxy downloads from GitHub Releases
        .route("/install.sh", get(download::serve_install_sh))
        .route("/downloads/:file", get(download::serve_download))
        .with_state(state)
        .layer(cors)
}
