use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde_json::json;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};

use trending_agent_core::api::xuangubao::XuanGubaoExtra;
use trending_agent_core::api::Client as ApiClient;
use trending_agent_core::report::report::Report;

#[derive(Clone)]
pub struct WebState {
    pub report: Arc<RwLock<Option<Report>>>,
}

impl Default for WebState {
    fn default() -> Self {
        Self::new()
    }
}

impl WebState {
    pub fn new() -> Self {
        Self {
            report: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn set_report(&self, report: Report) {
        *self.report.write().await = Some(report);
    }

    pub async fn get_report(&self) -> Option<Report> {
        self.report.read().await.clone()
    }
}

async fn refresh_report(state: &WebState) -> anyhow::Result<()> {
    let client = ApiClient::new();

    let (overview, limit_up, limit_down, plate_abnormal) = tokio::join!(
        client.xuangubao.fetch_market_overview(),
        client.xuangubao.fetch_limit_up(),
        client.xuangubao.fetch_limit_down(),
        client.xuangubao.fetch_plate_abnormal(30),
    );

    let overview = overview?;
    let limit_up = limit_up.unwrap_or_default();
    let limit_down = limit_down.unwrap_or_default();
    let plate_abnormal = plate_abnormal.unwrap_or_default();

    // Jiuyan: optional if JiuYan auth env is configured
    let (field_items, timeline) = if std::env::var("JIUYAN_PHONE").is_ok() {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let this_month = chrono::Local::now().format("%Y-%m").to_string();
        let (f, t) = tokio::join!(
            client.jiuyan.fetch_field(&today, 1),
            client.jiuyan.fetch_timeline(&this_month),
        );
        (f.unwrap_or_default(), t.unwrap_or_default())
    } else {
        (vec![], vec![])
    };

    let report = Report::new(
        overview,
        limit_up,
        plate_abnormal.clone(),
        field_items,
        timeline,
        vec![],
        None,
        vec![],
        XuanGubaoExtra {
            limit_down,
            plate_abnormal,
        },
    );
    state.set_report(report).await;
    Ok(())
}

async fn start_web_server(port: u16, state: WebState) {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/report", get(get_report_json))
        .route("/api/report/live", get(get_report_live))
        .layer(cors)
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    println!("🌐 Web API: http://localhost:{}", port);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(serde::Deserialize)]
struct ReportQuery {
    section: Option<String>,
}

async fn get_report_json(
    State(state): State<WebState>,
    Query(q): Query<ReportQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let report = state
        .get_report()
        .await
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let full = serde_json::to_value(&report).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let result = match q.section {
        Some(ref sections) if !sections.is_empty() => {
            let mut out = json!({});
            let obj = out.as_object_mut().unwrap();
            for key in sections.split(',').map(|s| s.trim()) {
                match key {
                    "summary" => {
                        obj.insert("risk_level".into(), full["risk_level"].clone());
                        obj.insert(
                            "opportunity_signals".into(),
                            full["opportunity_signals"].clone(),
                        );
                        obj.insert("decision_summary".into(), full["decision_summary"].clone());
                    }
                    _ => {
                        if let Some(val) = full.get(key) {
                            obj.insert(key.to_string(), val.clone());
                        }
                    }
                }
            }
            out
        }
        _ => full,
    };

    Ok(Json(result))
}

async fn get_report_live(State(state): State<WebState>) -> Result<impl IntoResponse, StatusCode> {
    refresh_report(&state).await.map_err(|e| {
        eprintln!("refresh error: {}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    let report = state
        .get_report()
        .await
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let json = serde_json::to_value(&report).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let state = WebState::new();

    if let Err(e) = refresh_report(&state).await {
        eprintln!(
            "⚠️  initial fetch failed: {} — API will retry on request",
            e
        );
    }

    start_web_server(8080, state).await;
    Ok(())
}
