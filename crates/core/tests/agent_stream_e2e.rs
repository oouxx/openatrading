//! Simplified e2e test: direct Agent::process() with get_limit_up tool.
//!
//! Uses the same pattern as tool_call_demo — no event streaming layer,
//! just Agent::process() directly.
//!
//! ```sh
//! export DEEPSEEK_API_KEY=sk-...   # or LLM_API_KEY / OPENROUTER_API_KEY
//! cargo test -p openatrading-core --test agent_stream_e2e -- --nocapture
//! ```
//!
//! What the test does:
//! 1. Builds an Agent with get_limit_up tool.
//! 2. Sends a user message asking the model to call get_limit_up.
//! 3. Calls agent.process() and inspects the returned messages.
//! 4. Asserts a ToolResult for get_limit_up with non-empty data.

use std::sync::Arc;

use openatrading_agent::agent::{Agent, AgentOptions};
use openatrading_agent::pi_ai_types::{
    ContentBlock, Model, ModelCost, StopReason,
};
use openatrading_agent::types::{
    AgentMessage, AgentState, ConvertToLlmFn, StreamFn,
};
use openatrading_provider::providers::register_builtins::register_built_in_api_providers;
use openatrading_provider::stream as pi_stream;
use openatrading_provider::pi_types;

use openatrading_core::llm::tools::get_limit_up;

// ── helpers (same pattern as tool_call_demo) ──

fn make_stream_fn(api_key: String) -> StreamFn {
    Arc::new(move |model, context, _thinking, _opts| {
        let key = api_key.clone();
        Box::pin(async move {
            let options = pi_types::StreamOptions {
                api_key: Some(key),
                max_tokens: Some(4096),
                temperature: Some(0.7),
                ..Default::default()
            };
            let stream = pi_stream::stream(&model, &context, Some(options));
            Ok(Box::new(stream)
                as openatrading_agent::pi_ai_types::StreamResponse)
        })
    })
}

fn make_convert_to_llm() -> ConvertToLlmFn {
    Arc::new(|messages: &[AgentMessage]| {
        messages
            .iter()
            .filter_map(|m| match m {
                AgentMessage::User { content, timestamp } => {
                    Some(openatrading_agent::pi_ai_types::Message::User {
                        content: content.clone(),
                        timestamp: *timestamp,
                    })
                }
                AgentMessage::Assistant {
                    content,
                    api,
                    provider,
                    model,
                    usage,
                    stop_reason,
                    error_message,
                    timestamp,
                } => {
                    Some(openatrading_agent::pi_ai_types::Message::Assistant {
                        content: content.clone(),
                        api: api.clone(),
                        provider: provider.clone(),
                        model: model.clone(),
                        response_model: None,
                        response_id: None,
                        diagnostics: None,
                        usage: usage.clone(),
                        stop_reason: stop_reason.clone().unwrap_or(StopReason::Stop),
                        error_message: error_message.clone(),
                        timestamp: *timestamp,
                    })
                }
                AgentMessage::ToolResult {
                    tool_call_id,
                    tool_name,
                    content,
                    details,
                    is_error,
                    timestamp,
                } => {
                    Some(openatrading_agent::pi_ai_types::Message::ToolResult {
                        tool_call_id: tool_call_id.clone(),
                        tool_name: tool_name.clone(),
                        content: content.clone(),
                        details: Some(details.clone()),
                        is_error: *is_error,
                        timestamp: *timestamp,
                    })
                }
                _ => None,
            })
            .collect()
    })
}

// ── test ──

#[tokio::test]
async fn stream_chat_invokes_get_limit_up() {
    let api_key = std::env::var("DEEPSEEK_API_KEY")
        .or_else(|_| std::env::var("LLM_API_KEY"))
        .or_else(|_| std::env::var("OPENROUTER_API_KEY"))
        .expect("set DEEPSEEK_API_KEY, LLM_API_KEY, or OPENROUTER_API_KEY");

    register_built_in_api_providers();

    let model = Model {
        id: "deepseek-chat".into(),
        name: "DeepSeek Chat".into(),
        api: "openai-completions".into(),
        provider: "deepseek".into(),
        base_url: "https://api.deepseek.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec!["text".into()],
        cost: ModelCost::default(),
        context_window: 128000,
        max_tokens: 4096,
        headers: None,
        compat: None,
    };

    let system_prompt = "你是一个A股短线交易AI助手，擅长分析涨停板行情。\
                        当用户问涨停板时，调用 get_limit_up 工具获取数据并基于结果回答。\
                        不要编造数据。";

    let agent = Agent::new(AgentOptions {
        initial_state: Some(AgentState {
            system_prompt: system_prompt.into(),
            model,
            thinking_level: "off".into(),
            tools: vec![get_limit_up()],
            messages: vec![],
            is_streaming: false,
            streaming_message: None,
            pending_tool_calls: Default::default(),
            error_message: None,
        }),
        convert_to_llm: Some(make_convert_to_llm()),
        stream_fn: Some(make_stream_fn(api_key)),
        ..Default::default()
    });

    eprintln!("[test] calling agent.process()...");

    let messages = agent
        .process(vec![AgentMessage::User {
            content: vec![ContentBlock::Text {
                text: "请用 get_limit_up 工具查询今天的涨停股票，并简要总结前 5 只。".into(),
                text_signature: None,
            }],
            timestamp: chrono::Utc::now().timestamp_millis(),
        }])
        .await
        .expect("agent.process() failed");

    eprintln!("[test] got {} messages", messages.len());

    let mut saw_tool_result = false;
    for msg in &messages {
        match msg {
            AgentMessage::Assistant {
                content,
                stop_reason,
                usage,
                ..
            } => {
                let text: String = content
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text, .. } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect();
                eprintln!(
                    "[test] Assistant (stop={:?}, tokens={}): {}",
                    stop_reason,
                    usage.total_tokens,
                    &text.chars().take(200).collect::<String>()
                );
            }
            AgentMessage::ToolResult {
                tool_name,
                content,
                is_error,
                ..
            } => {
                let text: String = content
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text, .. } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect();
                eprintln!(
                    "[test] ToolResult [{}] {} ({} chars)",
                    if *is_error { "ERROR" } else { "OK" },
                    tool_name,
                    text.len()
                );
                if tool_name == "get_limit_up" {
                    assert!(!text.is_empty(), "get_limit_up returned empty result");
                    assert!(
                        text.len() > 20,
                        "get_limit_up result too short: {}",
                        &text.chars().take(100).collect::<String>()
                    );
                    saw_tool_result = true;
                }
            }
            _ => {}
        }
    }

    assert!(
        saw_tool_result,
        "model never called get_limit_up (messages: {})",
        messages.len()
    );
}
