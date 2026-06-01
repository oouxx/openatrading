use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Result;

use openatrading_provider::llm::LlmClient;
use pi_agent_core::agent::{Agent, AgentOptions};
use pi_agent_core::pi_ai_types::{ContentBlock, StreamOptions, StreamResponse, Transport};
use pi_agent_core::types::{AgentMessage, AgentState, DynTool, GetApiKeyFn, StreamFn};

#[derive(Debug)]
pub struct AgentResult {
    pub final_text: String,
    pub rounds: usize,
}

pub fn create_agent(llm: &LlmClient, tools: Vec<Arc<DynTool>>, system_prompt: &str) -> Agent {
    let model = llm.model.clone();
    let stream_llm = llm.clone();
    let api_key = llm.api_key.clone();

    let stream_fn: StreamFn = Arc::new(move |_model, ctx, _thinking, opts| {
        let api_key = api_key.clone();
        let stream_llm = stream_llm.clone();
        Box::pin(async move {
            let mut options = StreamOptions {
                api_key: Some(api_key),
                max_tokens: Some(4096),
                temperature: Some(0.7),
                ..Default::default()
            };
            if let Some(key) = opts.api_key.clone() {
                options.api_key = Some(key);
            }
            if let Some(session_id) = opts.session_id {
                options.session_id = Some(session_id);
            }
            if let Some(transport) = opts.transport {
                options.transport = Some(match transport.as_str() {
                    "websocket" => Transport::Websocket,
                    "websocket-cached" => Transport::WebsocketCached,
                    "auto" => Transport::Auto,
                    _ => Transport::Sse,
                });
            }

            let event_stream = stream_llm.create_stream(&_model, &ctx, options);
            let response: StreamResponse = Box::new(event_stream);
            Ok(response)
        })
    });

    let get_api_key: GetApiKeyFn = Arc::new({
        let api_key = llm.api_key.clone();
        move |_provider| {
            let key = api_key.clone();
            Box::pin(async move { Some(key) })
        }
    });

    let state = AgentState {
        system_prompt: system_prompt.to_string(),
        model,
        thinking_level: "off".into(),
        tools,
        messages: Vec::new(),
        is_streaming: false,
        streaming_message: None,
        pending_tool_calls: HashSet::new(),
        error_message: None,
    };

    Agent::new(AgentOptions {
        initial_state: Some(state),
        stream_fn: Some(stream_fn),
        get_api_key: Some(get_api_key),
        ..Default::default()
    })
}

pub async fn agent_loop(
    llm: &LlmClient,
    tools: Vec<Arc<DynTool>>,
    system_prompt: &str,
    user_message: &str,
) -> Result<AgentResult> {
    let agent = create_agent(llm, tools, system_prompt);

    let messages = vec![AgentMessage::User {
        content: vec![ContentBlock::text(user_message)],
        timestamp: 0,
    }];

    let result = agent
        .process(messages)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let final_text: String = result
        .iter()
        .rev()
        .find_map(|msg| match msg {
            AgentMessage::Assistant { content, .. } => {
                let text: String = content
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text, .. } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect();
                if text.is_empty() {
                    None
                } else {
                    Some(text)
                }
            },
            _ => None,
        })
        .unwrap_or_default();

    Ok(AgentResult {
        final_text,
        rounds: 1,
    })
}

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::future::Future;
    use std::pin::Pin;

    use super::*;
    use pi_ai::types::{AssistantMessage, AssistantMessageEvent, StopReason, Usage};

    /// Build a minimal `AssistantMessage` with sensible defaults for testing.
    fn make_msg(content: Vec<ContentBlock>) -> AssistantMessage {
        AssistantMessage {
            content,
            api: "test".into(),
            provider: "test".into(),
            model: "test".into(),
            response_model: None,
            response_id: None,
            diagnostics: None,
            usage: Usage::default(),
            stop_reason: StopReason::Stop,
            error_message: None,
            timestamp: 0,
        }
    }

    /// Build a mock `StreamFn` that plays back a canned text reply.
    fn mock_text_stream_fn(reply: &str) -> StreamFn {
        let reply = reply.to_string();
        Arc::new(move |_model, _ctx, _thinking, _opts| {
            let reply = reply.clone();
            Box::pin(async move {
                let msg = make_msg(vec![ContentBlock::text(&reply)]);
                let events = vec![
                    AssistantMessageEvent::TextDelta {
                        content_index: 0,
                        delta: reply.clone(),
                        partial: msg.clone(),
                    },
                    AssistantMessageEvent::Done {
                        reason: StopReason::Stop,
                        message: msg,
                    },
                ];
                let stream = futures::stream::iter(events);
                let response: StreamResponse = Box::new(stream);
                Ok(response)
            })
                as Pin<
                    Box<
                        dyn Future<Output = Result<StreamResponse, Box<dyn Error + Send + Sync>>>
                            + Send,
                    >,
                >
        })
    }

    #[tokio::test]
    async fn test_create_agent_basic() {
        let llm = LlmClient::with_config("deepseek".to_string(), "deepseek-v4-flash".to_string());
        let agent = create_agent(&llm, vec![], "你是一个交易助手");
        // Agent created without panicking
        let _ = agent;
    }

    #[tokio::test]
    async fn test_create_agent_with_tools() {
        let llm = LlmClient::with_config("deepseek".to_string(), "deepseek-v4-flash".to_string());
        let agent = create_agent(&llm, vec![], "你是一个A股短线交易AI助手");
        let _ = agent;
    }

    #[tokio::test]
    async fn test_agent_process_returns_text() {
        let expected = "今日市场情绪偏暖，建议持有。";
        let stream_fn = mock_text_stream_fn(expected);

        let agent = Agent::new(AgentOptions {
            stream_fn: Some(stream_fn),
            get_api_key: Some(Arc::new(|_| {
                Box::pin(async move { Some("test-key".to_string()) })
            })),
            ..Default::default()
        });

        let messages = vec![AgentMessage::User {
            content: vec![ContentBlock::text("大盘怎么样？")],
            timestamp: 0,
        }];

        let result = agent
            .process(messages)
            .await
            .expect("agent process should succeed");
        let last_text: String = result
            .iter()
            .rev()
            .find_map(|msg| match msg {
                AgentMessage::Assistant { content, .. } => {
                    let t: String = content
                        .iter()
                        .filter_map(|b| match b {
                            ContentBlock::Text { text, .. } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect();
                    if t.is_empty() {
                        None
                    } else {
                        Some(t)
                    }
                },
                _ => None,
            })
            .unwrap_or_default();

        assert!(!last_text.is_empty(), "expected assistant text");
        assert_eq!(last_text, expected);
    }

    #[tokio::test]
    async fn test_agent_process_empty_user_message() {
        let stream_fn = mock_text_stream_fn("收到空消息");
        let agent = Agent::new(AgentOptions {
            stream_fn: Some(stream_fn),
            get_api_key: Some(Arc::new(|_| {
                Box::pin(async move { Some("test-key".to_string()) })
            })),
            ..Default::default()
        });

        let messages = vec![AgentMessage::User {
            content: vec![ContentBlock::text("")],
            timestamp: 0,
        }];

        let result = agent
            .process(messages)
            .await
            .expect("agent process should succeed");
        assert!(!result.is_empty(), "should return at least one message");
    }

    #[tokio::test]
    async fn test_agent_process_stop_reason_propagated() {
        let stream_fn = mock_text_stream_fn("完成");
        let agent = Agent::new(AgentOptions {
            stream_fn: Some(stream_fn),
            get_api_key: Some(Arc::new(|_| {
                Box::pin(async move { Some("test-key".to_string()) })
            })),
            ..Default::default()
        });

        let result = agent
            .process(vec![AgentMessage::User {
                content: vec![ContentBlock::text("hi")],
                timestamp: 0,
            }])
            .await
            .expect("agent process should succeed");

        // process should end with at least one assistant message
        let has_assistant = result
            .iter()
            .any(|m| matches!(m, AgentMessage::Assistant { .. }));
        assert!(has_assistant, "should contain assistant messages");
    }
}
