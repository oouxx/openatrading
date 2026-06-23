use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use futures::StreamExt;
use pi_ai::env_api_keys::get_env_api_key;
use pi_ai::models::get_model;
use pi_ai::providers::register_builtins::register_built_in_api_providers;
use pi_ai::stream;
use pi_ai::types::{
    AssistantMessageEvent, ContentBlock, Context, Message, Model, SimpleStreamOptions,
    StreamOptions, Tool as PiTool,
};

pub use pi_agent_core::types::{StreamFn, StreamFnOptions};

#[derive(Clone)]
pub struct LlmClient {
    pub model: Model,
    pub api_key: String,
}

impl LlmClient {
    /// Build a pi-agent-core-compatible `StreamFn` from this client.
    /// This bridges `LlmClient::create_stream` into the generic `StreamFn`
    /// interface that `pi_agent_core::agent::Agent` and `run_agent_loop` expect.
    pub fn to_stream_fn(&self) -> StreamFn {
        let api_key = self.api_key.clone();
        let stream_llm = self.clone();
        Arc::new(move |_model, ctx, _thinking, opts| {
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
                        "websocket" => pi_ai::types::Transport::Websocket,
                        "websocket-cached" => pi_ai::types::Transport::WebsocketCached,
                        "auto" => pi_ai::types::Transport::Auto,
                        _ => pi_ai::types::Transport::Sse,
                    });
                }
                let event_stream = stream_llm.create_stream(&_model, &ctx, options);
                let response: pi_agent_core::pi_ai_types::StreamResponse = Box::new(event_stream);
                Ok(response)
            })
        })
    }

    /// Create a pi-agent-core `Agent` pre-configured with this client's LLM.
    pub fn create_agent(
        &self,
        tools: Vec<std::sync::Arc<pi_agent_core::types::DynTool>>,
        system_prompt: &str,
    ) -> pi_agent_core::agent::Agent {
        use pi_agent_core::agent::{Agent, AgentOptions};
        use pi_agent_core::types::{AgentState, GetApiKeyFn};

        let stream_fn = self.to_stream_fn();
        let get_api_key: GetApiKeyFn = std::sync::Arc::new({
            let api_key = self.api_key.clone();
            move |_provider| {
                let key = api_key.clone();
                Box::pin(async move { Some(key) })
            }
        });

        Agent::new(AgentOptions {
            initial_state: Some(AgentState {
                system_prompt: system_prompt.to_string(),
                model: self.model.clone(),
                thinking_level: "off".into(),
                tools,
                messages: Vec::new(),
                is_streaming: false,
                streaming_message: None,
                pending_tool_calls: HashSet::new(),
                error_message: None,
            }),
            stream_fn: Some(stream_fn),
            get_api_key: Some(get_api_key),
            ..Default::default()
        })
    }

    /// Return a sensible base_url for a known provider when the model is
    /// not in the built-in registry.  Falls back to empty (which pi-ai's
    /// HTTP transport treats as "use the registered API endpoint").
    fn default_base_url(provider: &str) -> String {
        match provider {
            "deepseek" => "https://api.deepseek.com",
            "openrouter" => "https://openrouter.ai/api/v1",
            "siliconflow" => "https://api.siliconflow.cn/v1",
            "github" => "https://models.inference.ai.azure.com",
            _ => "",
        }
        .to_string()
    }

    pub fn from_env(provider: String, model_id: String) -> Self {
        register_built_in_api_providers();
        let api_key = std::env::var("LLM_API_KEY")
            .ok()
            .or_else(|| get_env_api_key(&provider))
            .unwrap_or_default();
        let model = get_model(&provider, &model_id).unwrap_or_else(|| Model {
            id: model_id.clone(),
            name: model_id.clone(),
            api: "openai-completions".to_string(),
            provider: provider.clone(),
            base_url: Self::default_base_url(&provider),
            reasoning: false,
            thinking_level_map: None,
            input: vec!["text".to_string()],
            cost: Default::default(),
            context_window: 128000,
            max_tokens: 4096,
            headers: None,
            compat: None,
        });

        tracing::info!(
            "[llm] from_env: provider={}, model={}, api_key={}..{}",
            model.provider,
            model.id,
            if api_key.is_empty() { "EMPTY" } else { "sk-" },
            if api_key.len() > 6 {
                &api_key[api_key.len() - 4..]
            } else {
                ""
            }
        );

        Self { model, api_key }
    }

    /// Create LlmClient with explicit provider + model (override env/defaults).
    /// Call from_env() first, then override provider and model.
    pub fn with_config(provider: String, model_id: String) -> Self {
        Self::from_env(provider, model_id)
    }

    /// Create LlmClient from config-provided provider/model, with env var fallback.
    /// Priority: config field > env var > built-in default.
    pub fn from_config(config_provider: Option<&str>, config_model: Option<&str>) -> Self {
        let provider = config_provider
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .or_else(|| std::env::var("LLM_PROVIDER").ok())
            .unwrap_or_else(|| "openrouter".to_string());
        let model_id = config_model
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .or_else(|| std::env::var("LLM_MODEL").ok())
            .unwrap_or_else(|| "openrouter/free".to_string());
        Self::with_config(provider, model_id)
    }

    pub async fn chat(&self, prompt: &str) -> Result<String> {
        let context = Context {
            system_prompt: None,
            messages: vec![Message::User {
                content: vec![ContentBlock::text(prompt)],
                timestamp: 0,
            }],
            tools: None,
        };

        let options = StreamOptions {
            api_key: Some(self.api_key.clone()),
            max_tokens: Some(4096),
            temperature: Some(0.7),
            ..Default::default()
        };

        let msg = stream::complete(&self.model, &context, Some(options))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let text: String = msg
            .content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();

        if text.is_empty() {
            anyhow::bail!("LLM returned empty response");
        }
        Ok(text)
    }

    pub async fn chat_stream(&self, prompt: &str) -> Result<StreamResult> {
        let context = Context {
            system_prompt: None,
            messages: vec![Message::User {
                content: vec![ContentBlock::text(prompt)],
                timestamp: 0,
            }],
            tools: None,
        };

        let options = StreamOptions {
            api_key: Some(self.api_key.clone()),
            max_tokens: Some(4096),
            temperature: Some(0.7),
            ..Default::default()
        };

        let event_stream = stream::stream(&self.model, &context, Some(options));
        let result_stream = event_stream_to_text_stream(event_stream);
        Ok(Box::pin(result_stream))
    }

    pub async fn chat_stream_with_messages(&self, messages_json: &str) -> Result<StreamResult> {
        let messages_val: serde_json::Value =
            serde_json::from_str(messages_json).unwrap_or_default();

        let context = json_to_context(&messages_val, None);

        let options = StreamOptions {
            api_key: Some(self.api_key.clone()),
            max_tokens: Some(4096),
            temperature: Some(0.7),
            ..Default::default()
        };

        let event_stream = stream::stream(&self.model, &context, Some(options));
        let result_stream = event_stream_to_text_stream(event_stream);
        Ok(Box::pin(result_stream))
    }

    /// Stream text from the model with per-chunk markers that distinguish
    /// normal text from thinking/reasoning content.
    ///
    /// See [`event_stream_to_marked_stream`] for the marker format.
    pub async fn chat_stream_with_messages_marked(
        &self,
        messages_json: &str,
    ) -> Result<StreamResult> {
        let messages_val: serde_json::Value =
            serde_json::from_str(messages_json).unwrap_or_default();

        let context = json_to_context(&messages_val, None);

        let options = StreamOptions {
            api_key: Some(self.api_key.clone()),
            max_tokens: Some(4096),
            temperature: Some(0.7),
            ..Default::default()
        };

        let event_stream = stream::stream(&self.model, &context, Some(options));
        let result_stream = event_stream_to_marked_stream(event_stream);
        Ok(Box::pin(result_stream))
    }

    pub fn stream_simple(
        &self,
        context: &Context,
        reasoning: Option<&str>,
    ) -> pi_ai::utils::event_stream::AssistantMessageEventStream {
        let options = SimpleStreamOptions {
            base: StreamOptions {
                api_key: Some(self.api_key.clone()),
                max_tokens: Some(4096),
                temperature: Some(0.7),
                ..Default::default()
            },
            reasoning: reasoning.map(|s| s.to_string()),
            thinking_budgets: None,
        };

        stream::stream_simple(&self.model, context, Some(options))
    }

    /// Create a stream for the given model and context. Used by the agent's
    /// `StreamFn` so the agent crate does not have to depend on `pi-ai`.
    pub fn create_stream(
        &self,
        model: &Model,
        context: &Context,
        options: StreamOptions,
    ) -> pi_ai::utils::event_stream::AssistantMessageEventStream {
        let mut options = options;
        if options.api_key.as_ref().is_none_or(|k| k.is_empty()) {
            options.api_key = Some(self.api_key.clone());
        }
        if options.max_tokens.is_none() {
            options.max_tokens = Some(4096);
        }
        if options.temperature.is_none() {
            options.temperature = Some(0.7);
        }
        stream::stream(model, context, Some(options))
    }
}

/// Trait for abstracting the LLM chat stream provider.
/// Enables the TUI to accept either a real [`LlmClient`] or a mock in tests.
#[async_trait]
pub trait ChatStreamProvider: Clone + Send + 'static {
    async fn chat_stream_with_messages(&self, messages_json: &str) -> Result<StreamResult>;
}

#[async_trait]
impl ChatStreamProvider for LlmClient {
    async fn chat_stream_with_messages(&self, messages_json: &str) -> Result<StreamResult> {
        self.chat_stream_with_messages(messages_json).await
    }
}

fn json_to_context(
    messages_val: &serde_json::Value,
    tools_val: Option<&serde_json::Value>,
) -> Context {
    let mut system_prompt = None;
    let mut messages = Vec::new();

    if let Some(arr) = messages_val.as_array() {
        for msg in arr {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
            let content = msg.get("content").and_then(|c| c.as_str()).unwrap_or("");

            match role {
                "system" => {
                    system_prompt = Some(content.to_string());
                },
                "user" => {
                    messages.push(Message::User {
                        content: vec![ContentBlock::text(content)],
                        timestamp: 0,
                    });
                },
                "assistant" => {
                    let mut blocks = Vec::new();
                    if !content.is_empty() {
                        blocks.push(ContentBlock::text(content));
                    }
                    if let Some(tool_calls) = msg.get("tool_calls").and_then(|tc| tc.as_array()) {
                        for tc in tool_calls {
                            let id = tc
                                .get("id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let name = tc
                                .get("function")
                                .and_then(|f| f.get("name"))
                                .and_then(|n| n.as_str())
                                .unwrap_or("")
                                .to_string();
                            let arguments = tc
                                .get("function")
                                .and_then(|f| f.get("arguments"))
                                .and_then(|a| a.as_str())
                                .unwrap_or("{}");
                            let args_val: serde_json::Value =
                                serde_json::from_str(arguments).unwrap_or_default();
                            blocks.push(ContentBlock::ToolCall {
                                id,
                                name,
                                arguments: args_val,
                                thought_signature: None,
                            });
                        }
                    }
                    if !blocks.is_empty() {
                        messages.push(Message::Assistant {
                            content: blocks,
                            api: String::new(),
                            provider: String::new(),
                            model: String::new(),
                            response_model: None,
                            response_id: None,
                            diagnostics: None,
                            usage: Default::default(),
                            stop_reason: pi_ai::types::StopReason::Stop,
                            error_message: None,
                            timestamp: 0,
                        });
                    }
                },
                "tool" => {
                    let tool_call_id = msg
                        .get("tool_call_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let tool_name = msg
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    messages.push(Message::ToolResult {
                        tool_call_id,
                        tool_name,
                        content: vec![ContentBlock::text(content)],
                        details: None,
                        is_error: false,
                        timestamp: 0,
                    });
                },
                _ => {},
            }
        }
    }

    let tools = tools_val.and_then(|v| v.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|t| {
                let func = t.get("function")?;
                Some(PiTool {
                    name: func.get("name")?.as_str()?.to_string(),
                    description: func
                        .get("description")
                        .and_then(|d| d.as_str())
                        .unwrap_or("")
                        .to_string(),
                    parameters: func.get("parameters").cloned().unwrap_or_default(),
                })
            })
            .collect::<Vec<_>>()
    });

    Context {
        system_prompt,
        messages,
        tools,
    }
}

fn event_stream_to_text_stream(
    event_stream: pi_ai::utils::event_stream::AssistantMessageEventStream,
) -> impl futures::Stream<Item = Result<String>> {
    async_stream::stream! {
        let mut stream = Box::pin(event_stream);
        while let Some(event) = stream.next().await {
            match event {
                AssistantMessageEvent::TextDelta { delta, .. }
                    if !delta.is_empty() => {
                        yield Ok(delta);
                    }
                AssistantMessageEvent::Error { error, .. } => {
                    let msg = error.error_message.as_deref().unwrap_or("unknown error");
                    yield Err(anyhow::anyhow!("LLM stream error: {msg}"));
                    break;
                }
                _ => {}
            }
        }
    }
}

/// Like [`event_stream_to_text_stream`] but prefixes each chunk so the
/// consumer can distinguish text from reasoning/thinking content.
///
/// - Text chunks are prefixed with `\x00TEXT\x00`
/// - Thinking chunks are prefixed with `\x00THINKING\x00`
///
/// The null-byte prefix is chosen because it cannot appear in LLM output,
/// making parsing unambiguous.
fn event_stream_to_marked_stream(
    event_stream: pi_ai::utils::event_stream::AssistantMessageEventStream,
) -> impl futures::Stream<Item = Result<String>> {
    async_stream::stream! {
        let mut stream = Box::pin(event_stream);
        while let Some(event) = stream.next().await {
            match event {
                AssistantMessageEvent::TextDelta { delta, .. }
                    if !delta.is_empty() => {
                        yield Ok(format!("\x00TEXT\x00{}", delta.replace('\n', "\\n")));
                    }
                AssistantMessageEvent::ThinkingDelta { delta, .. }
                    if !delta.is_empty() => {
                        yield Ok(format!("\x00THINKING\x00{}", delta.replace('\n', "\\n")));
                    }
                AssistantMessageEvent::Error { error, .. } => {
                    let msg = error.error_message.as_deref().unwrap_or("unknown error");
                    yield Err(anyhow::anyhow!("LLM stream error: {msg}"));
                    break;
                }
                _ => {}
            }
        }
    }
}

/// Convert a single AssistantMessageEvent into StreamEvent(s), if relevant.
/// Shared between stream-based (event_stream_to_tool_stream) and
/// channel-based (SSE handler) consumers.
pub fn assistant_event_to_stream_events(event: &AssistantMessageEvent) -> Vec<StreamEvent> {
    let mut events = Vec::new();
    match event {
        AssistantMessageEvent::TextDelta { delta, .. } if !delta.is_empty() => {
            events.push(StreamEvent::Text(delta.clone()));
        },
        AssistantMessageEvent::ThinkingDelta { delta, .. } if !delta.is_empty() => {
            events.push(StreamEvent::ReasoningText(delta.clone()));
        },
        AssistantMessageEvent::ToolCallStart { .. } => {
            // Name not available until ToolCallEnd — skip empty start
        },
        AssistantMessageEvent::ToolCallDelta { delta, .. } if !delta.is_empty() => {
            events.push(StreamEvent::ToolCallDelta(ToolCall {
                id: String::new(),
                function_name: String::new(),
                arguments: delta.clone(),
            }));
        },
        AssistantMessageEvent::ToolCallEnd { tool_call, .. } if !tool_call.name.is_empty() => {
            events.push(StreamEvent::ToolCallStart(ToolCall {
                id: tool_call.id.clone(),
                function_name: tool_call.name.clone(),
                arguments: tool_call.arguments.to_string(),
            }));
        },
        AssistantMessageEvent::Done { message, .. } => {
            for block in &message.content {
                use pi_ai::types::ContentBlock;
                if let ContentBlock::ToolCall {
                    id,
                    name,
                    arguments,
                    ..
                } = block
                {
                    events.push(StreamEvent::ToolCallStart(ToolCall {
                        id: id.clone(),
                        function_name: name.clone(),
                        arguments: arguments.to_string(),
                    }));
                }
            }
        },
        AssistantMessageEvent::Error { error, .. } => {
            let msg = error.error_message.as_deref().unwrap_or("unknown error");
            events.push(StreamEvent::Error(msg.to_string()));
        },
        _ => {},
    }
    events
}

fn default_tool_call() -> ToolCall {
    ToolCall {
        id: String::new(),
        function_name: String::new(),
        arguments: String::new(),
    }
}

fn event_stream_to_tool_stream(
    event_stream: pi_ai::utils::event_stream::AssistantMessageEventStream,
) -> impl futures::Stream<Item = Result<Vec<StreamEvent>>> {
    async_stream::stream! {
        let mut stream = Box::pin(event_stream);
        while let Some(event) = stream.next().await {
            let events = assistant_event_to_stream_events(&event);
            if !events.is_empty() {
                yield Ok(events);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub function_name: String,
    pub arguments: String,
}

pub enum StreamEvent {
    Text(String),
    ReasoningText(String),
    ToolCallStart(ToolCall),
    ToolCallDelta(ToolCall),
    Error(String),
}

#[cfg(not(target_arch = "wasm32"))]
pub type StreamResult = std::pin::Pin<Box<dyn futures::Stream<Item = Result<String>> + Send>>;
#[cfg(target_arch = "wasm32")]
pub type StreamResult = std::pin::Pin<Box<dyn futures::Stream<Item = Result<String>>>>;

#[cfg(not(target_arch = "wasm32"))]
pub type ToolStreamResult =
    std::pin::Pin<Box<dyn futures::Stream<Item = Result<Vec<StreamEvent>>> + Send>>;
#[cfg(target_arch = "wasm32")]
pub type ToolStreamResult =
    std::pin::Pin<Box<dyn futures::Stream<Item = Result<Vec<StreamEvent>>>>>;

#[derive(Debug, serde::Deserialize)]
pub struct ToolCallMessage {
    pub role: Option<String>,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCallItem>>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ToolCallItem {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: Option<String>,
    pub function: ToolCallFunction,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct ToolChatChoice {
    pub message: ToolCallMessage,
}

#[derive(Debug, serde::Deserialize)]
pub struct ToolChatResponse {
    pub choices: Vec<ToolChatChoice>,
}
