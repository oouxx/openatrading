pub mod agent_ws;
pub mod cache;
pub mod chat;
pub mod chat_session;
pub mod gateway;
pub mod web;
pub mod mcp;
pub mod handlers;

use cache::AppCache;
use std::sync::Arc;
use tokio::sync::RwLock;

use axum::{
    routing::{delete, get, patch, post, put},
    Router,
};
use openatrading_core::log::Level;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing_subscriber::fmt::time::LocalTime;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use openatrading_core::context::AppContext;
use openatrading_core::llm::McpServerConfig;
pub use openatrading_core::RoleEntry;

pub struct AppState {
    pub ctx: AppContext,
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
        .route("/api/overview", get(web::get_overview))
        .route("/api/search-stocks", get(web::search_stocks))
        .route(
            "/api/sentiment-history",
            get(web::get_sentiment_history),
        )
        .route("/api/limit-up", get(web::get_limit_up))
        .route("/api/limit-down", get(web::get_limit_down))
        .route("/api/plate-abnormal", get(web::get_plate_abnormal))
        .route("/api/field-items", get(web::get_field_items))
        .route("/api/timeline", get(web::get_timeline))
        .route("/api/kline/stream", get(web::get_kline_stream))
        .route("/api/kline/:symbol", get(web::get_kline))
        .route("/api/realtime/stream", get(web::get_realtime_stream))
        .route("/api/realtime/:symbol", get(web::get_realtime))
        .route(
            "/api/yesterday-limit-up",
            get(web::get_yesterday_limit_up),
        )
        .route("/api/super-stock", get(web::get_super_stock))
        .route("/api/index-quotes", get(web::get_index_quotes))
        .route("/api/telegraph", get(web::get_telegraph_all))
        .route("/api/telegraph/:category", get(web::get_telegraph))
        .route("/api/dapan-abnormal", get(web::get_dapan_abnormal))
        .route("/api/market-trend/:symbol", get(web::get_market_trend))
        .route("/api/ai/sessions/ws", get(agent_ws::ws_handler))
        .route("/api/ai/roles", get(chat::get_ai_roles))
        .route("/api/ai/commands", get(chat::get_ai_commands))
        .route("/api/skills", get(chat::get_skills))
        .route("/api/skills", post(chat::save_skill))
        .route("/api/skills/:name", delete(chat::delete_skill))
        .route("/api/tools", get(chat::get_tools))
        .route(
            "/api/plates-by-any-stock/:symbol",
            get(web::get_plates_by_any_stock),
        )
        .route("/api/roles", get(web::get_roles))
        .route(
            "/api/roles/:name/positions",
            get(web::get_role_positions),
        )
        .route("/api/roles/:name/logs", get(web::get_role_logs))
        .route("/api/roles", post(web::create_role))
        .route(
            "/api/roles/:name/strategy",
            put(web::update_role_strategy),
        )
        .route("/api/roles/:name", delete(web::delete_role))
        .route("/api/roles/:name/reset", post(web::reset_role))
        .route(
            "/api/roles/:name/pnl-history",
            get(web::get_role_pnl_history),
        )
        .route("/api/roles/:name/history", get(web::get_role_history))
        .route("/api/replay/events", get(web::get_replay_events))
        .route("/api/replay/index", get(web::get_replay_index))
        .route("/api/replay/news", get(web::get_replay_news))
        .route("/api/replay/snapshots", get(web::get_replay_snapshots))
        .route(
            "/api/dragon-tiger/detail",
            get(web::get_dragon_tiger_detail),
        )
        .route(
            "/api/dragon-tiger/stock-seat",
            get(web::get_dragon_tiger_stock_seat),
        )
        .route("/api/ai/sessions", get(chat::list_sessions))
        .route(
            "/api/ai/sessions/:id/messages",
            get(chat::get_session_messages),
        )
        .route("/api/ai/sessions/:id", delete(chat::delete_session))
        .route("/api/ai/sessions/:id", patch(chat::rename_session))
        .route(
            "/api/ai/sessions/:id/tool-results/:tool_call_id",
            get(chat::get_tool_result),
        )
        .route("/api/mcp/servers", get(mcp::list_handler))
        .route("/api/mcp/servers", post(mcp::add_handler))
        .route("/api/mcp/servers/:name", delete(mcp::delete_handler))
        .route("/api/mcp/servers/:name/toggle", get(mcp::toggle_handler))
        .route("/api/watchlist", get(web::get_watchlist))
        .route("/api/watchlist", post(web::add_watchlist))
        .route("/api/watchlist/:code", delete(web::delete_watchlist))
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
