use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Result;

use openatrading_provider::llm::LlmClient;
use pi_agent_core::agent::{Agent, AgentOptions};
use pi_agent_core::pi_ai_types::{ContentBlock, StreamOptions, StreamResponse, Transport};
use pi_agent_core::types::{
    AgentMessage, AgentState, DynTool, GetApiKeyFn, StreamFn,
};

#[derive(Debug)]
pub struct AgentResult {
    pub final_text: String,
    pub rounds: usize,
}



pub fn create_agent(
    llm: &LlmClient,
    tools: Vec<Arc<DynTool>>,
    system_prompt: &str,
) -> Agent {
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

