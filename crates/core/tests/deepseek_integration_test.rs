//! DeepSeek v4 Flash integration test — exercises the full agent pipeline:
//! LLM streaming, tool calling (get_market_overview / get_limit_up),
//! and multi-round agent processing.
//!
//! Follows the same pattern as agent_stream_e2e.rs.
//!
//! ```sh
//! cargo test -p openatrading-core --test deepseek_integration_test -- --nocapture
//! ```

use std::sync::Arc;

use openatrading_core::llm::pi_ai_types::{AssistantMessageEvent, ContentBlock};
use openatrading_core::llm::{tools, AgentEvent, AgentMessage, LlmClient};
#[ignore = "integration test"]
#[tokio::test]
async fn test_deepseek_v4_flash_agent_process() {
    load_env();

    let llm = LlmClient::with_config("deepseek".to_string(), "deepseek-v4-flash".to_string());

    let tools: Vec<Arc<openatrading_core::llm::DynTool>> =
        vec![tools::get_market_overview(), tools::get_limit_up()];

    let agent = llm.create_agent(
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

#[tokio::test]
#[ignore = "integration test"]
async fn test_deepseek_is_streaming() {
    load_env();

    let llm = LlmClient::with_config("deepseek".to_string(), "deepseek-v4-flash".to_string());

    let tools: Vec<Arc<openatrading_core::llm::DynTool>> =
        vec![tools::get_market_overview(), tools::get_limit_up()];

    let agent = Arc::new(llm.create_agent(
        tools,
        "你是一个A股短线交易AI助手。可以调用工具获取实时数据，基于数据给出分析。",
    ));

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<AgentEvent>();

    let listener = Arc::new(
        move |event: AgentEvent, _signal: Option<tokio::sync::watch::Receiver<bool>>| {
            let tx = tx.clone();
            Box::pin(async move {
                let _ = tx.send(event);
            }) as std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
        },
    );

    let handle = agent.subscribe(listener).await;

    let agent_clone = Arc::clone(&agent);
    tokio::spawn(async move {
        let _ = agent_clone
            .process(vec![AgentMessage::User {
                content: vec![ContentBlock::Text {
                    text: "请先用 get_market_overview 查询市场概况，然后用 get_limit_up 查询今日涨停板。".to_string(),
                    text_signature: None,
                }],
                timestamp: 0,
            }])
            .await;
        drop(handle);
    });

    let mut text_delta_count = 0u32;
    let mut total_text = String::new();

    while let Some(event) = rx.recv().await {
        match event {
            AgentEvent::MessageUpdate {
                assistant_message_event: AssistantMessageEvent::TextDelta { ref delta, .. },
                ..
            } if !delta.is_empty() => {
                text_delta_count += 1;
                total_text.push_str(delta);
                eprint!("{delta}");
            },
            AgentEvent::AgentEnd { .. } => break,
            _ => {},
        }
    }

    eprintln!();
    eprintln!(
        "[stream stats] deltas={text_delta_count}, total_chars={}",
        total_text.len()
    );

    assert!(
        text_delta_count >= 1,
        "expected at least one TextDelta event (got {text_delta_count}) — response is not streaming"
    );
    assert!(
        !total_text.trim().is_empty(),
        "accumulated text should not be empty"
    );
}
