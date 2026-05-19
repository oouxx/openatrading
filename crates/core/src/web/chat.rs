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

use super::handlers::OverviewResponse;
use super::AppState;

// ── AI Chat (SSE) ─────────────────────────────────────

#[derive(Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Deserialize)]
pub struct ChatRequest {
    message: String,
    #[serde(default)]
    history: Vec<ChatMessage>,
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
    let strategy_text =
        crate::roles::get_builtin_strategy("zarili").unwrap_or("你是一个A股短线交易AI助手。");
    let system_prompt = build_system_prompt(strategy_text);
    let user_message =
        build_user_message(&overview, field_items.as_deref(), super_stocks.as_deref());

    let mut messages_vec: Vec<serde_json::Value> = vec![
        serde_json::json!({"role": "system", "content": system_prompt}),
        serde_json::json!({"role": "user", "content": user_message}),
        serde_json::json!({"role": "assistant", "content": "已收到市场数据，请提问。"}),
    ];

    for msg in &req.history {
        messages_vec.push(serde_json::json!({
            "role": msg.role,
            "content": msg.content,
        }));
    }

    messages_vec.push(serde_json::json!({
        "role": "user",
        "content": req.message,
    }));

    let prompt = serde_json::json!(messages_vec).to_string();

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