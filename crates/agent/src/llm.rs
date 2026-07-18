use anyhow::Result;
use async_trait::async_trait;
use pi_coding_agent::core::agent_session::AgentSession;
use pi_coding_agent::core::sdk::{self, CreateAgentSessionOptions, NoToolsMode};
use pi_coding_agent::pi_agent_core::types::AgentEvent;
use pi_coding_agent::pi_ai_types::{get_env_api_key, AssistantMessageEvent, ContentBlock, Model};
// ponytail: get_model / register_built_in_api_providers aren't in pi_ai_types
// re-exports. Accessed through pi-coding-agent's pi_agent_core re-export chain.
use pi_coding_agent::pi_agent_core::pi_ai::models::get_model;
use pi_coding_agent::pi_agent_core::pi_ai::providers::register_builtins::register_built_in_api_providers;
pub use pi_coding_agent::pi_agent_core::types::{StreamFn, StreamFnOptions};

#[derive(Clone)]
pub struct LlmClient {
    pub model: Model,
    pub api_key: String,
}

impl LlmClient {
    pub fn to_stream_fn(&self) -> StreamFn {
        pi_coding_agent::core::sdk::create_default_stream_fn()
    }

    /// Create an in-memory agent session via the SDK.
    async fn create_session(&self, system_prompt: Option<&str>) -> Result<AgentSession> {
        let cwd = std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let (session, _result) = sdk::create_agent_session(CreateAgentSessionOptions {
            cwd,
            model: Some(self.model.clone()),
            no_tools: Some(NoToolsMode::All),
            custom_prompt: system_prompt.map(|s| s.to_string()),
            persist_session: false,
            session_file: None,
            extension_paths: Vec::new(),
            enable_extensions: true,
            agent_dir: None,
            thinking_level: None,
            scoped_models: None,
            tools: None,
            exclude_tools: None,
            append_system_prompt: None,
            session_name: None,
            stream_fn: None,
            convert_to_llm: None,
            custom_tools: None,
            extension_registry: None,
            cli_provider: None,
            cli_model: None,
            fork_from: None,
            session_dir: None,
        })
        .await
        .map_err(|e| anyhow::anyhow!("create_agent_session failed: {e}"))?;
        Ok(session)
    }

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

    pub fn with_config(provider: String, model_id: String) -> Self {
        Self::from_env(provider, model_id)
    }

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

    // ── Public chat API (unified via create_agent_session) ──────────────

    /// One-shot chat — wraps text as JSON and collects the response.
    pub async fn chat(&self, prompt: &str) -> Result<String> {
        let mut session = self.create_session(None).await?;
        session.add_user_text(prompt).await;
        let messages = session.get_messages().await;

        let text: String = messages
            .into_iter()
            .rev()
            .find_map(|msg| {
                if let pi_coding_agent::pi_agent_core::types::AgentMessage::Assistant {
                    content,
                    ..
                } = msg
                {
                    let t: String = content
                        .iter()
                        .filter_map(|b| {
                            if let ContentBlock::Text { text, .. } = b {
                                Some(text.as_str())
                            } else {
                                None
                            }
                        })
                        .collect();
                    if t.is_empty() {
                        None
                    } else {
                        Some(t)
                    }
                } else {
                    None
                }
            })
            .unwrap_or_default();

        if text.is_empty() {
            anyhow::bail!("LLM returned empty response");
        }
        Ok(text)
    }

    /// Stream chat — input is JSON messages array `[{"role":"user","content":"..."}]`.
    pub async fn chat_stream(&self, messages_json: &str) -> Result<StreamResult> {
        let system_prompt = extract_system_prompt(messages_json);
        let user_text = extract_user_text(messages_json);
        let session = self.create_session(system_prompt.as_deref()).await?;
        let stream = session_to_stream(session, user_text, false);
        Ok(Box::pin(stream))
    }

    /// Stream chat with `\x00TEXT\x00` / `\x00THINKING\x00` markers.
    pub async fn chat_stream_marked(&self, messages_json: &str) -> Result<StreamResult> {
        let system_prompt = extract_system_prompt(messages_json);
        let user_text = extract_user_text(messages_json);
        let session = self.create_session(system_prompt.as_deref()).await?;
        let stream = session_to_stream(session, user_text, true);
        Ok(Box::pin(stream))
    }

    /// Streaming chat with message history (backward compat alias).
    /// Delegates to [`chat_stream`].
    pub async fn chat_stream_with_messages(&self, messages_json: &str) -> Result<StreamResult> {
        self.chat_stream(messages_json).await
    }
}

// ── SDK session → Stream ─────────────────────────────────────────────────────

/// Spawn an AgentSession and forward text deltas through a channel.
/// When `marked` is true, chunks are prefixed with \x00TEXT\x00 / \x00THINKING\x00.
fn session_to_stream(
    mut session: AgentSession,
    text: String,
    marked: bool,
) -> impl futures::Stream<Item = Result<String>> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Result<String>>();
    let tx2 = tx.clone();

    tokio::spawn(async move {
        session
            .subscribe(std::sync::Arc::new(move |event, _signal| {
                let tx = tx2.clone();
                Box::pin(async move {
                    if let AgentEvent::MessageUpdate {
                        assistant_message_event,
                        ..
                    } = event
                    {
                        match assistant_message_event {
                            AssistantMessageEvent::TextDelta { delta, .. } if !delta.is_empty() => {
                                let chunk = if marked {
                                    format!("\x00TEXT\x00{}", delta.replace('\n', "\\n"))
                                } else {
                                    delta
                                };
                                let _ = tx.send(Ok(chunk));
                            },
                            AssistantMessageEvent::ThinkingDelta { delta, .. }
                                if !delta.is_empty() && marked =>
                            {
                                let _ = tx.send(Ok(format!(
                                    "\x00THINKING\x00{}",
                                    delta.replace('\n', "\\n")
                                )));
                            },
                            AssistantMessageEvent::Error { error, .. } => {
                                let msg = error.error_message.unwrap_or_else(|| "unknown".into());
                                let _ = tx.send(Err(anyhow::anyhow!("{msg}")));
                            },
                            _ => {},
                        }
                    }
                })
            }))
            .await;
        session.add_user_text(&text).await;
    });

    async_stream::stream! {
        while let Some(item) = rx.recv().await {
            yield item;
        }
    }
}

/// Extract the system prompt from a JSON messages array (first system message).
fn extract_system_prompt(messages_json: &str) -> Option<String> {
    let val: serde_json::Value = serde_json::from_str(messages_json).ok()?;
    let arr = val.as_array()?;
    for msg in arr {
        if msg.get("role").and_then(|r| r.as_str()) == Some("system") {
            return msg
                .get("content")
                .and_then(|c| c.as_str())
                .map(|s| s.to_string());
        }
    }
    None
}

/// Extract the last user text from a JSON messages array.
fn extract_user_text(messages_json: &str) -> String {
    let val: serde_json::Value = serde_json::from_str(messages_json).unwrap_or_default();
    let arr = val.as_array().map(|v| v.as_slice()).unwrap_or_default();
    for msg in arr.iter().rev() {
        if msg.get("role").and_then(|r| r.as_str()) == Some("user") {
            if let Some(text) = msg.get("content").and_then(|c| c.as_str()) {
                return text.to_string();
            }
        }
    }
    String::new()
}

// ── Trait (kept for TUI backward compat) ─────────────────────────────────────

#[async_trait]
pub trait ChatStreamProvider: Clone + Send + 'static {
    async fn chat_stream_with_messages(&self, messages_json: &str) -> Result<StreamResult>;
}

#[async_trait]
impl ChatStreamProvider for LlmClient {
    async fn chat_stream_with_messages(&self, messages_json: &str) -> Result<StreamResult> {
        self.chat_stream(messages_json).await
    }
}

// ── Event types & converters ─────────────────────────────────────────────────

/// Convert a single AssistantMessageEvent into StreamEvent(s), if relevant.
pub fn assistant_event_to_stream_events(event: &AssistantMessageEvent) -> Vec<StreamEvent> {
    let mut events = Vec::new();
    match event {
        AssistantMessageEvent::TextDelta { delta, .. } if !delta.is_empty() => {
            events.push(StreamEvent::Text(delta.clone()));
        },
        AssistantMessageEvent::ThinkingDelta { delta, .. } if !delta.is_empty() => {
            events.push(StreamEvent::ReasoningText(delta.clone()));
        },
        AssistantMessageEvent::ToolCallStart { .. } => {},
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
                use pi_coding_agent::pi_ai_types::ContentBlock;
                match block {
                    ContentBlock::Text { text, .. } => {
                        events.push(StreamEvent::Text(text.clone()));
                    },
                    ContentBlock::ToolCall {
                        id,
                        name,
                        arguments,
                        ..
                    } => {
                        events.push(StreamEvent::ToolCallStart(ToolCall {
                            id: id.clone(),
                            function_name: name.clone(),
                            arguments: arguments.to_string(),
                        }));
                    },
                    _ => {},
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
