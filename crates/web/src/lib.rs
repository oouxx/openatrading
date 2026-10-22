pub mod agent_ws;
pub mod automations;
pub mod cache;
pub mod chat;
pub mod chat_session;
pub mod gateway;
pub mod handlers;
pub mod market_stream;
pub mod mcp;
pub mod web;

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
use openatrading_core::llm::McpConfigStore;
pub use openatrading_core::RoleEntry;

pub struct AppState {
    pub ctx: AppContext,
    pub roles: Arc<RwLock<Vec<RoleEntry>>>,
    /// 外部 MCP server 的配置读写入口（文件为唯一事实源）。
    pub mcp: McpConfigStore,
    pub cache: Arc<AppCache>,
    /// Shared multiplexer for market SSE streams (kline/realtime). One upstream
    /// poll per symbol set, fanned out to every SSE connection.
    pub market_hub: Arc<market_stream::MarketStreamHub>,
    /// 自动化调度中枢(定时任务 CRUD/触发/收件箱)。
    pub automation: Arc<openatrading_core::automation::AutomationHub>,
    /// Tauri-only: sessions that already have a background event-forward task.
    /// Guards against creating a new subscription + forward task on every
    /// `send_prompt`, which would deliver every event N times (WebSocket path
    /// relies on the actor's fan-out dropping dead subscriber channels).
    pub forward_tasks: std::sync::Mutex<std::collections::HashMap<String, ()>>,
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
        .route("/api/sentiment-history", get(web::get_sentiment_history))
        .route("/api/limit-up", get(web::get_limit_up))
        .route("/api/limit-down", get(web::get_limit_down))
        .route("/api/plate-abnormal", get(web::get_plate_abnormal))
        .route("/api/field-items", get(web::get_field_items))
        .route("/api/timeline", get(web::get_timeline))
        .route("/api/kline/stream", get(web::get_kline_stream))
        .route("/api/kline/:symbol", get(web::get_kline))
        .route("/api/realtime/stream", get(web::get_realtime_stream))
        .route("/api/realtime/:symbol", get(web::get_realtime))
        .route("/api/yesterday-limit-up", get(web::get_yesterday_limit_up))
        .route("/api/super-stock", get(web::get_super_stock))
        .route("/api/index-quotes", get(web::get_index_quotes))
        .route("/api/telegraph", get(web::get_telegraph_all))
        .route("/api/telegraph/:category", get(web::get_telegraph))
        .route("/api/dapan-abnormal", get(web::get_dapan_abnormal))
        .route("/api/market-trend/:symbol", get(web::get_market_trend))
        .route("/api/ai/sessions/ws", get(agent_ws::ws_handler))
        .route("/api/ai/roles", get(chat::get_ai_roles))
        .route("/api/ai/commands", get(chat::get_ai_commands))
        .route("/api/agent/settings", get(chat::get_agent_settings))
        .route("/api/agent/settings", post(chat::update_agent_settings))
        .route("/api/agent/models", get(chat::get_agent_models))
        .route("/api/agent/auth", get(chat::get_agent_auth))
        .route("/api/agent/auth", post(chat::set_agent_api_key))
        .route(
            "/api/agent/auth/:provider",
            delete(chat::delete_agent_api_key),
        )
        .route("/api/skills", get(chat::get_skills))
        .route("/api/skills", post(chat::save_skill))
        .route("/api/skills/:name", delete(chat::delete_skill))
        .route("/api/tools", get(chat::get_tools))
        .route(
            "/api/plates-by-any-stock/:symbol",
            get(web::get_plates_by_any_stock),
        )
        .route("/api/roles", get(web::get_roles))
        .route("/api/roles/:name/positions", get(web::get_role_positions))
        .route("/api/roles/:name/account", get(web::get_role_account))
        .route("/api/roles/:name/trades", get(web::get_role_trades))
        .route("/api/roles", post(web::create_role))
        .route("/api/roles/:name", delete(web::delete_role))
        .route("/api/roles/:name/reset", post(web::reset_role))
        .route("/api/roles/:name/orders", post(web::place_order))
        .route(
            "/api/roles/:name/pnl-history",
            get(web::get_role_pnl_history),
        )
        .route(
            "/api/roles/:name/pnl-calendar",
            get(web::get_role_pnl_calendar),
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
        .route(
            "/api/automations",
            get(automations::list_automations).post(automations::create_automation),
        )
        .route(
            "/api/automations/:id",
            put(automations::update_automation).delete(automations::delete_automation),
        )
        .route(
            "/api/automations/:id/trigger",
            post(automations::trigger_automation),
        )
        .route("/api/automations/runs", get(automations::list_runs))
        .route(
            "/api/automations/runs/:id/read",
            post(automations::mark_run_read),
        )
        .route(
            "/api/automations/runs/:id/archive",
            post(automations::archive_run),
        )
        .route("/api/mcp/servers", get(mcp::list_handler))
        .route("/api/mcp/servers", post(mcp::add_handler))
        .route(
            "/api/mcp/servers/:name",
            delete(mcp::delete_handler).put(mcp::update_handler),
        )
        .route("/api/mcp/servers/:name/toggle", get(mcp::toggle_handler))
        .route("/api/watchlist", get(web::get_watchlist))
        .route("/api/watchlist", post(web::add_watchlist))
        .route("/api/watchlist/:code", delete(web::delete_watchlist))
        .route("/api/about", get(web::get_about))
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
