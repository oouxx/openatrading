use serde::Serialize;
use std::sync::OnceLock;
use wasm_bindgen::prelude::*;

static XGB_BASE_URL: OnceLock<String> = OnceLock::new();

use trending_agent_core::api::xuangubao::XuanguBaoClient;

#[derive(Serialize)]
struct WasmReport {
    date: String,
    overview: trending_agent_core::api::xuangubao::MarketOverview,
    limit_up: Vec<trending_agent_core::api::xuangubao::Stock>,
    plate_abnormal: Vec<trending_agent_core::api::xuangubao::PlateAbnormalEvent>,
    risk_level: String,
    opportunity_signals: Vec<String>,
    decision_summary: String,
    field_items: Vec<serde_json::Value>,
    timeline: Vec<serde_json::Value>,
    analyses: Vec<serde_json::Value>,
    sentiment: Option<serde_json::Value>,
    taoguba_users: Vec<serde_json::Value>,
}

/// 设置选股宝 API 代理地址。开发时传入 "/api/xgb" 走 Vite proxy，生产环境调用真实 URL。
#[wasm_bindgen]
pub fn set_xgb_base_url(url: &str) {
    let _ = XGB_BASE_URL.set(url.to_string());
}

#[wasm_bindgen]
pub async fn run_report() -> Result<JsValue, JsValue> {
    let base_url = XGB_BASE_URL
        .get()
        .cloned()
        .unwrap_or_else(|| "https://flash-api.xuangubao.cn/api".to_string());

    let client = XuanguBaoClient::with_base_url(base_url);

    // WASM 中 futures::join! 不兼容 wasm-bindgen-futures，使用顺序 await
    let overview = client
        .fetch_market_overview()
        .await
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let limit_up = client
        .fetch_limit_up()
        .await
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let plates = client
        .fetch_plate_abnormal(50)
        .await
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    // 风险等级计算
    let ratio = overview.rise_count as f64 / overview.fall_count.max(1) as f64;
    let risk_level = if ratio < 0.8 && overview.bomb_rate > 30.0 {
        "高"
    } else if ratio < 1.2 || overview.bomb_rate > 20.0 {
        "中"
    } else {
        "低"
    };

    let mut opportunity_signals = Vec::new();
    if let Some(head) = limit_up.iter().max_by_key(|s| s.limit_up_days) {
        opportunity_signals.push(format!(
            "龙头：{}（{}板）",
            head.stock_chi_name, head.limit_up_days
        ));
    }

    let decision_summary = format!(
        "风险等级：{}；上涨{}家/下跌{}家",
        risk_level, overview.rise_count, overview.fall_count
    );

    let now = js_sys::Date::new_0();
    let date_str = format!(
        "{}-{:02}-{:02} {:02}:{:02}",
        now.get_full_year(),
        now.get_month() + 1,
        now.get_date(),
        now.get_hours(),
        now.get_minutes()
    );

    let report = WasmReport {
        date: date_str,
        overview,
        limit_up,
        plate_abnormal: plates,
        risk_level: risk_level.to_string(),
        opportunity_signals,
        decision_summary,
        field_items: vec![],
        timeline: vec![],
        analyses: vec![],
        sentiment: None,
        taoguba_users: vec![],
    };

    let json = serde_json::to_string(&report).map_err(|e| JsValue::from_str(&e.to_string()))?;

    Ok(JsValue::from_str(&json))
}
