use axum::{
    extract::State,
    http::StatusCode,
    response::{sse::Event, IntoResponse, Sse},
    Json,
};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use openatrading_core::api::Client as ApiClient;

use crate::handlers::OverviewResponse;
use crate::AppState;

#[derive(Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Deserialize)]
pub struct ChatRequest {
    message: String,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    history: Vec<ChatMessage>,
}

#[derive(Serialize)]
pub struct AiRoleItem {
    pub name: String,
    pub display_name: String,
}

pub async fn get_ai_roles(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    let roles_guard = state.roles.read().await;
    let roles: Vec<AiRoleItem> = roles_guard
        .iter()
        .map(|r| AiRoleItem {
            name: r.name.clone(),
            display_name: r.display_name.clone(),
        })
        .collect();
    Ok(Json(roles))
}

pub fn build_system_prompt(strategy_text: &str) -> String {
    strategy_text.to_string()
}

pub fn build_user_message(
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
        bomb_rate = overview.bomb_rate * 100.0,
        ratio = format_args!(
            "{:.2}",
            overview.rise_count as f64 / overview.fall_count.max(1) as f64
        ),
        risk_level = overview.risk_level,
        summary = overview.decision_summary,
        field_items = field_section,
        super_stocks = super_section,
    )
}

pub async fn fetch_field_json(api: &ApiClient) -> Option<String> {
    let date = openatrading_core::utils::time::beijing_now()
        .format("%Y-%m-%d")
        .to_string();
    match api.jiuyan.fetch_field(&date, 1).await {
        Ok(plates) if !plates.is_empty() => serde_json::to_string(&plates).ok(),
        _ => None,
    }
}

pub async fn fetch_super_stock_json(api: &ApiClient) -> Option<String> {
    match api.xuangubao.fetch_super_stock().await {
        Ok(stocks) if !stocks.is_empty() => serde_json::to_string(&stocks).ok(),
        _ => None,
    }
}

pub async fn fetch_overview_data(api: &ApiClient) -> anyhow::Result<OverviewResponse> {
    let overview = api.xuangubao.fetch_market_overview().await?;

    let ratio = overview.rise_count as f64 / overview.fall_count.max(1) as f64;
    let risk_level = if ratio < 0.8 && overview.bomb_rate > 0.3 {
        "高"
    } else if ratio < 1.2 || overview.bomb_rate > 0.2 {
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
        date: openatrading_core::utils::time::beijing_now()
            .format("%Y-%m-%d %H:%M")
            .to_string(),
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
    use openatrading_core::llm::{create_agent, AgentMessage};
    use openatrading_core::llm::pi_ai_types::{ContentBlock, StopReason};

    let tools: Vec<Arc<openatrading_core::llm::DynTool>> = vec![
        openatrading_core::llm::tools::get_market_overview(),
        openatrading_core::llm::tools::get_limit_up(),
        openatrading_core::llm::tools::get_limit_down(),
        openatrading_core::llm::tools::get_super_stock(),
        openatrading_core::llm::tools::get_yesterday_limit_up(),
        openatrading_core::llm::tools::get_dapan_abnormal(),
        openatrading_core::llm::tools::get_realtime_quote(),
        openatrading_core::llm::tools::get_telegraph(),
    ];

    let role_name = req.role.as_deref().unwrap_or("zarili");
    let roles_guard = state.roles.read().await;
    let strategy_text = roles_guard
        .iter()
        .find(|r| r.name == role_name)
        .map(|r| r.strategy.as_str())
        .unwrap_or("你是一个A股短线交易AI助手。可以调用工具获取实时市场数据来回答用户问题。");

    let system_prompt = build_system_prompt(strategy_text);

    let agent = create_agent(&state.llm, tools, &system_prompt);

    let mut pi_messages: Vec<AgentMessage> = Vec::new();
    for h in req.history {
        match h.role.as_str() {
            "user" => {
                pi_messages.push(AgentMessage::User {
                    content: vec![ContentBlock::Text {
                        text: h.content,
                        text_signature: None,
                    }],
                    timestamp: 0,
                });
            }
            "assistant" => {
                pi_messages.push(AgentMessage::Assistant {
                    content: vec![ContentBlock::Text {
                        text: h.content,
                        text_signature: None,
                    }],
                    api: String::new(),
                    provider: String::new(),
                    model: String::new(),
                    usage: Default::default(),
                    stop_reason: Some(StopReason::Stop),
                    error_message: None,
                    timestamp: 0,
                });
            }
            _ => {}
        }
    }
    pi_messages.push(AgentMessage::User {
        content: vec![ContentBlock::Text {
            text: req.message,
            text_signature: None,
        }],
        timestamp: 0,
    });

    let sse_stream = async_stream::stream! {
        match agent.process(pi_messages).await {
            Ok(results) => {
                for msg in &results {
                    match msg {
                        AgentMessage::Assistant { content, error_message, .. } => {
                            if let Some(err) = error_message {
                                if !err.is_empty() {
                                    yield Ok(Event::default().data(format!("[ERROR] {err}")));
                                }
                            }
                            for block in content {
                                match block {
                                    ContentBlock::Text { text, .. } => {
                                        if !text.is_empty() {
                                            yield Ok(Event::default().data(text.replace('\n', "\\n")));
                                        }
                                    }
                                    ContentBlock::ToolCall { name, .. } => {
                                        yield Ok(Event::default().event("tool").data(name.as_str()));
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            Err(e) => {
                yield Ok(Event::default().data(format!("[ERROR] {e}")));
            }
        }
        yield Ok(Event::default().data("[DONE]"));
    };

    Ok(Sse::new(sse_stream))
}