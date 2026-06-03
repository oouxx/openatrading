use std::sync::Arc;

use openatrading_agent::pi_ai_types::ContentBlock;
use openatrading_agent::runner::create_agent;
use openatrading_agent::types::AgentMessage;
use openatrading_core::llm::tools;
use openatrading_core::llm::LlmClient;

#[tokio::test]
async fn test_openrouter_agent_process() {
    load_env();

    let llm = LlmClient::with_config(
        "openrouter".to_string(),
        "nvidia/nemotron-3-super-120b-a12b:free".to_string(),
    );

    let tools: Vec<Arc<openatrading_core::llm::DynTool>> =
        vec![tools::get_market_overview(), tools::get_limit_up()];

    let agent = create_agent(
        &llm,
        tools,
        "你是一个A股短线交易AI助手。可以调用工具获取实时数据，基于数据给出分析。",
    );

    let messages = agent
        .process(vec![AgentMessage::User {
            content: vec![ContentBlock::text(
                "请先用 get_market_overview 查询市场概况，然后用 get_limit_up 查询今日涨停板。",
            )],
            timestamp: 0,
        }])
        .await
        .expect("agent.process() failed");

    // Verify: agent called tools and got results
    let has_tool_call = messages.iter().any(|m| {
        matches!(m, AgentMessage::Assistant { content, .. } if content.iter().any(|b| matches!(b, ContentBlock::ToolCall { .. })))
    });
    let has_tool_result = messages
        .iter()
        .any(|m| matches!(m, AgentMessage::ToolResult { .. }));
    let has_final_text = messages.iter().any(|m| {
        matches!(m, AgentMessage::Assistant { content, .. } if {
            let t: String = content.iter().filter_map(|b| match b {
                ContentBlock::Text { text, .. } => Some(text.as_str()),
                _ => None,
            }).collect();
            t.len() > 50
        })
    });

    assert!(has_tool_call, "agent should have called tools");
    assert!(has_tool_result, "agent should have received tool results");
    assert!(has_final_text, "agent should have produced final analysis");
}

/// Load env from project root .env
fn load_env() {
    let content = std::fs::read_to_string("../../.env").unwrap_or_default();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, val)) = line.split_once('=') {
            std::env::set_var(key.trim(), val.trim());
        }
    }
}
