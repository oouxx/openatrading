pub mod cache;
pub mod chat;
pub mod gateway;
pub mod handlers;
pub mod mcp;

use cache::AppCache;
use std::sync::Arc;
use tokio::sync::RwLock;

use axum::{
    routing::{delete, get, post, put},
    Router,
};
use openatrading_core::log::Level;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing_subscriber::fmt::time::LocalTime;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use openatrading_core::chat::ChatThreadStore;
use openatrading_core::context::AppContext;
use openatrading_core::llm::McpServerConfig;
pub use openatrading_core::RoleEntry;

pub struct AppState {
    pub ctx: AppContext,
    pub chat_threads: Arc<ChatThreadStore>,
    pub roles: Arc<RwLock<Vec<RoleEntry>>>,
    pub mcp_servers: std::sync::Mutex<Vec<McpServerConfig>>,
    pub cache: Arc<AppCache>,
}

pub fn build_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);
    let timer = LocalTime::rfc_3339();
    let _ = tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_timer(timer))
        .try_init();

    Router::new()
        .route("/api/overview", get(handlers::get_overview))
        .route("/api/search-stocks", get(handlers::search_stocks))
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
            "/api/realtime/:symbol/stream",
            get(handlers::get_realtime_stream),
        )
        .route(
            "/api/yesterday-limit-up",
            get(handlers::get_yesterday_limit_up),
        )
        .route("/api/super-stock", get(handlers::get_super_stock))
        .route("/api/index-quotes", get(handlers::get_index_quotes))
        .route("/api/telegraph", get(handlers::get_telegraph_all))
        .route("/api/telegraph/:category", get(handlers::get_telegraph))
        .route("/api/dapan-abnormal", get(handlers::get_dapan_abnormal))
        .route("/api/market-trend/:symbol", get(handlers::get_market_trend))
        .route("/api/ai/chat", post(chat::ai_chat))
        .route("/api/ai/roles", get(chat::get_ai_roles))
        .route("/api/ai/commands", get(chat::get_ai_commands))
        .route("/api/skills", get(chat::get_skills))
        .route("/api/skills", post(chat::save_skill))
        .route("/api/skills/:name", delete(chat::delete_skill))
        .route("/api/tools", get(chat::get_tools))
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
        .route("/api/roles", post(handlers::create_role))
        .route(
            "/api/roles/:name/strategy",
            put(handlers::update_role_strategy),
        )
        .route("/api/roles/:name", delete(handlers::delete_role))
        .route("/api/roles/:name/reset", post(handlers::reset_role))
        .route(
            "/api/roles/:name/pnl-history",
            get(handlers::get_role_pnl_history),
        )
        .route("/api/roles/:name/history", get(handlers::get_role_history))
        .route("/api/replay/events", get(handlers::get_replay_events))
        .route("/api/replay/index", get(handlers::get_replay_index))
        .route("/api/replay/news", get(handlers::get_replay_news))
        .route("/api/replay/snapshots", get(handlers::get_replay_snapshots))
        // .route("/api/replay/analyze", post(handlers::ai_replay_analyze))
        .route(
            "/api/dragon-tiger/detail",
            get(handlers::get_dragon_tiger_detail),
        )
        .route(
            "/api/dragon-tiger/stock-seat",
            get(handlers::get_dragon_tiger_stock_seat),
        )
        .route("/api/ai/threads", get(chat::list_threads))
        .route("/api/ai/threads", post(chat::create_thread))
        .route("/api/ai/threads/:id", put(chat::rename_thread))
        .route("/api/ai/threads/:id", delete(chat::delete_thread))
        .route(
            "/api/ai/threads/:id/messages",
            get(chat::get_thread_messages),
        )
        .route(
            "/api/ai/threads/:id/messages",
            post(chat::save_thread_message),
        )
        .route("/api/mcp/servers", get(mcp::list_handler))
        .route("/api/mcp/servers", post(mcp::add_handler))
        .route("/api/mcp/servers/:name", delete(mcp::delete_handler))
        .route("/api/mcp/servers/:name/toggle", get(mcp::toggle_handler))
        .route("/api/watchlist", get(handlers::get_watchlist))
        .route("/api/watchlist", post(handlers::add_watchlist))
        .route("/api/watchlist/:code", delete(handlers::delete_watchlist))
        .layer(cors)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(
                    DefaultMakeSpan::new()
                        .level(Level::INFO)
                        .include_headers(false),
                )
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
        .with_state(state)
}
