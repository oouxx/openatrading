use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde_json::json;
use tower_http::cors::{Any, CorsLayer};

use trending_agent_core::agents::sentiment::analyze_sentiment;
use trending_agent_core::api::xuangubao::{KlinePeriod, XuanGubaoExtra, XuanguBaoClient};
use trending_agent_core::api::Client as ApiClient;
use trending_agent_core::report::report::Report;

async fn build_report(field_date: Option<&str>) -> anyhow::Result<Report> {
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
        let field_date_str = field_date
            .unwrap_or(&chrono::Local::now().format("%Y-%m-%d").to_string())
            .to_string();
        let this_month = chrono::Local::now().format("%Y-%m").to_string();
        let (f, t) = tokio::join!(
            client.jiuyan.fetch_field(&field_date_str, 1),
            client.jiuyan.fetch_timeline(&this_month),
        );
        (f.unwrap_or_default(), t.unwrap_or_default())
    } else {
        (vec![], vec![])
    };

    // Sentiment: optional if Jiuyan auth is configured and community data is available
    let sentiment = if std::env::var("JIUYAN_PHONE").is_ok() {
        analyze_sentiment(&client).await.ok()
    } else {
        None
    };

    Ok(Report::new(
        overview,
        limit_up,
        plate_abnormal.clone(),
        field_items,
        timeline,
        vec![],
        sentiment,
        vec![],
        XuanGubaoExtra {
            limit_down,
            plate_abnormal,
        },
    ))
}

async fn start_web_server(port: u16) {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/report", get(get_report_json))
        .route("/api/report/live", get(get_report_live))
        .route("/api/kline/:symbol", get(get_kline))
        .route("/api/realtime/:symbol", get(get_realtime))
        .route("/api/yesterday-limit-up", get(get_yesterday_limit_up))
        .layer(cors);

    let addr = format!("0.0.0.0:{}", port);
    println!("🌐 Web API: http://localhost:{}", port);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(serde::Deserialize)]
struct ReportQuery {
    section: Option<String>,
}

async fn get_report_json(Query(q): Query<ReportQuery>) -> Result<impl IntoResponse, StatusCode> {
    let report = build_report(None).await.map_err(|e| {
        eprintln!("report error: {}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;
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

#[derive(serde::Deserialize)]
struct LiveQuery {
    date: Option<String>,
}

async fn get_report_live(Query(q): Query<LiveQuery>) -> Result<impl IntoResponse, StatusCode> {
    let report = build_report(q.date.as_deref()).await.map_err(|e| {
        eprintln!("report_live error: {}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    let json = serde_json::to_value(&report).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json))
}

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
    let json = serde_json::to_value(&data).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json))
}

async fn get_realtime(Path(symbol): Path<String>) -> Result<impl IntoResponse, StatusCode> {
    let client = XuanguBaoClient::new();
    let data = client.fetch_realtime(&symbol).await.map_err(|e| {
        eprintln!("realtime error: {}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    let json = serde_json::to_value(&data).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json))
}

async fn get_yesterday_limit_up() -> Result<impl IntoResponse, StatusCode> {
    let client = XuanguBaoClient::new();
    let data = client.fetch_yesterday_limit_up().await.map_err(|e| {
        eprintln!("yesterday_limit_up error: {:#}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    let json = serde_json::to_value(&data).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    start_web_server(8080).await;
    Ok(())
}
