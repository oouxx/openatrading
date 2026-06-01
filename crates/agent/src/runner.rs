use anyhow::Result;
use std::pin::Pin;
use std::sync::Arc;

use openatrading_provider::llm::LlmClient;
use crate::tool::ToolSet;

use pi_agent_core::agent::AgentOptions;
use pi_agent_core::pi_ai_types::{ContentBlock, StopReason, StreamOptions, StreamResponse, Transport};
use pi_agent_core::types::{AgentState, GetApiKeyFn, StreamFn};

use crate::types::{AgentEvent, AgentMessage};

#[derive(Debug)]
pub struct AgentResult {
    pub final_text: String,
    pub rounds: usize,
}

#[cfg(not(target_arch = "wasm32"))]
pub type AgentStream = Pin<Box<dyn futures::Stream<Item = Result<String>> + Send>>;

pub async fn agent_loop(
    llm: &LlmClient,
    tools: &Arc<ToolSet>,
    system_prompt: &str,
    user_message: &str,
) -> Result<AgentResult> {
    let agent = create_agent(llm, tools, system_prompt)?;

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
                if text.is_empty() { None } else { Some(text) }
            }
            _ => None,
        })
        .unwrap_or_default();

    Ok(AgentResult {
        final_text,
        rounds: 1,
    })
}

pub async fn agent_loop_stream(
    llm: LlmClient,
    tools: Arc<ToolSet>,
    system_prompt: String,
    history: Vec<serde_json::Value>,
    user_message: String,
) -> Result<AgentStream> {
    let agent = Arc::new(create_agent(&llm, &tools, &system_prompt)?);

    let mut pi_messages: Vec<AgentMessage> = Vec::new();
    for h in &history {
        let role = h.get("role").and_then(|r| r.as_str()).unwrap_or("");
        let content = h.get("content").and_then(|c| c.as_str()).unwrap_or("");
        match role {
            "user" => {
                pi_messages.push(AgentMessage::User {
                    content: vec![ContentBlock::text(content)],
                    timestamp: 0,
                });
            }
            "assistant" => {
                pi_messages.push(AgentMessage::Assistant {
                    content: vec![ContentBlock::text(content)],
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
        content: vec![ContentBlock::text(&user_message)],
        timestamp: 0,
    });

    let (tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<AgentEvent>();

    let listener = Arc::new(move |event: AgentEvent, _signal: Option<tokio::sync::watch::Receiver<bool>>| {
        let tx = tx.clone();
        Box::pin(async move {
            let _ = tx.send(event);
        }) as std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
    });

    agent.subscribe(listener).await;

    let agent_clone = agent.clone();
    let (done_tx, mut done_rx) = tokio::sync::mpsc::unbounded_channel::<Result<()>>();
    tokio::spawn(async move {
        let result = agent_clone.process(pi_messages).await;
        match result {
            Ok(_) => {
                let _ = done_tx.send(Ok(()));
            }
            Err(e) => {
                let _ = done_tx.send(Err(anyhow::anyhow!("{e}")));
            }
        }
    });

    let stream = async_stream::stream! {
        loop {
            tokio::select! {
                Some(event) = event_rx.recv() => {
                    match event {
                        AgentEvent::MessageStart { .. } => {}
                        AgentEvent::MessageEnd { message } => {
                            match &message {
                                AgentMessage::Assistant { content, .. } => {
                                    for block in content {
                                        match block {
                                            ContentBlock::Text { text, .. } => {
                                                if !text.is_empty() {
                                                    yield Ok(serde_json::json!({
                                                        "type": "text",
                                                        "content": text,
                                                    }).to_string());
                                                }
                                            }
                                            ContentBlock::ToolCall { id, name, arguments, .. } => {
                                                yield Ok(serde_json::json!({
                                                    "type": "tool_call",
                                                    "name": name,
                                                    "args": arguments.to_string(),
                                                }).to_string());
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                AgentMessage::ToolResult { content, tool_name, .. } => {
                                    let result_text: String = content.iter()
                                        .filter_map(|b| match b {
                                            ContentBlock::Text { text, .. } => Some(text.as_str()),
                                            _ => None,
                                        })
                                        .collect();
                                    yield Ok(serde_json::json!({
                                        "type": "tool_result",
                                        "name": tool_name,
                                        "result": result_text,
                                    }).to_string());
                                }
                                _ => {}
                            }
                        }
                        AgentEvent::ToolExecutionStart { tool_name, args, .. } => {
                            yield Ok(serde_json::json!({
                                "type": "tool_call",
                                "name": tool_name,
                                "args": args.to_string(),
                            }).to_string());
                        }
                        AgentEvent::ToolExecutionEnd { tool_name, result, .. } => {
                            let result_text = result.to_string();
                            yield Ok(serde_json::json!({
                                "type": "tool_result",
                                "name": tool_name,
                                "result": result_text,
                            }).to_string());
                        }
                        AgentEvent::AgentEnd { .. } => {
                            yield Ok(serde_json::json!({"type": "done"}).to_string());
                            break;
                        }
                        _ => {}
                    }
                }
                Some(res) = done_rx.recv() => {
                    if let Err(e) = res {
                        yield Ok(serde_json::json!({
                            "type": "error",
                            "message": e.to_string(),
                        }).to_string());
                    }
                    yield Ok(serde_json::json!({"type": "done"}).to_string());
                    break;
                }
                else => break,
            }
        }
    };

    Ok(Box::pin(stream))
}

fn create_agent(llm: &LlmClient, tools: &Arc<ToolSet>, system_prompt: &str) -> Result<crate::agent::Agent> {
    let model = llm.model.clone();
    let api_key = llm.api_key.clone();
    let stream_llm = llm.clone();

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

    let agent_tools = tools.to_agent_tools();
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
        tools: agent_tools,
        messages: Vec::new(),
        is_streaming: false,
        streaming_message: None,
        pending_tool_calls: std::collections::HashSet::new(),
        error_message: None,
    };

    let agent = crate::agent::Agent::new(AgentOptions {
        initial_state: Some(state),
        stream_fn: Some(stream_fn),
        get_api_key: Some(get_api_key),
        ..Default::default()
    });

    Ok(agent)
}