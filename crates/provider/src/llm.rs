use anyhow::Result;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: u32,
    temperature: f32,
    stream: bool,
}

#[derive(Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Debug, Deserialize)]
struct StreamChunk {
    choices: Option<Vec<StreamChoice>>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: Option<Delta>,
}

#[derive(Debug, Deserialize)]
struct Delta {
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
    tool_calls: Option<Vec<ToolCallDelta>>,
}

#[derive(Debug, Deserialize)]
struct ToolCallDelta {
    id: Option<String>,
    function: Option<FunctionDelta>,
}

#[derive(Debug, Deserialize)]
struct FunctionDelta {
    name: Option<String>,
    arguments: Option<String>,
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
}

#[cfg(not(target_arch = "wasm32"))]
pub type StreamResult = Pin<Box<dyn futures::Stream<Item = Result<String>> + Send>>;
#[cfg(target_arch = "wasm32")]
pub type StreamResult = Pin<Box<dyn futures::Stream<Item = Result<String>>>>;

#[cfg(not(target_arch = "wasm32"))]
pub type ToolStreamResult = Pin<Box<dyn futures::Stream<Item = Result<Vec<StreamEvent>>> + Send>>;
#[cfg(target_arch = "wasm32")]
pub type ToolStreamResult = Pin<Box<dyn futures::Stream<Item = Result<Vec<StreamEvent>>>>>;

#[derive(Clone)]
pub struct LlmClient {
    pub client: Client,
    pub api_key: String,
    pub api_endpoint: String,
    pub model: String,
}

impl LlmClient {
    pub fn from_env() -> Self {
        let api_key = std::env::var("LLM_API_KEY")
            .or_else(|_| std::env::var("DEEPSEEK_API_KEY"))
            .or_else(|_| std::env::var("OPENROUTER_API_KEY"))
            .unwrap_or_default();

        let default_endpoint = if std::env::var("OPENROUTER_API_KEY").is_ok()
            && std::env::var("DEEPSEEK_API_KEY").is_err()
            && std::env::var("LLM_API_KEY").is_err()
        {
            "https://openrouter.ai/api/v1/chat/completions"
        } else {
            "https://api.deepseek.com/v1/chat/completions"
        };

        let api_endpoint =
            std::env::var("LLM_API_ENDPOINT").unwrap_or_else(|_| default_endpoint.to_string());

        let model = std::env::var("LLM_MODEL").unwrap_or_else(|_| "deepseek-chat".to_string());

        eprintln!(
            "[llm] from_env: endpoint={}, model={}, api_key={}..{}",
            api_endpoint,
            model,
            if api_key.is_empty() { "EMPTY" } else { "sk-" },
            if api_key.len() > 6 {
                &api_key[api_key.len() - 4..]
            } else {
                ""
            }
        );

        Self {
            client: Client::new(),
            api_key,
            api_endpoint,
            model,
        }
    }

    pub fn new(api_key: String) -> Self {
        let mut c = Self::from_env();
        if !api_key.is_empty() {
            c.api_key = api_key;
        }
        c
    }

    pub async fn chat(&self, prompt: &str) -> Result<String> {
        let req = ChatRequest {
            model: self.model.clone(),
            messages: vec![Message {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            max_tokens: 4096,
            temperature: 0.7,
            stream: false,
        };

        let resp: ChatResponse = self
            .client
            .post(&self.api_endpoint)
            .bearer_auth(&self.api_key)
            .json(&req)
            .send()
            .await?
            .json()
            .await?;

        let content = resp
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .unwrap_or_default();
        if content.is_empty() {
            anyhow::bail!("LLM returned empty response");
        }
        Ok(content)
    }

    pub async fn chat_with_messages(&self, messages_json: &str) -> Result<String> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": serde_json::from_str::<serde_json::Value>(messages_json).unwrap_or_default(),
            "max_tokens": 4096,
            "temperature": 0.7,
            "stream": false,
        })
        .to_string();

        let resp: ChatResponse = self
            .client
            .post(&self.api_endpoint)
            .bearer_auth(&self.api_key)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await?
            .json()
            .await?;

        let content = resp
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .unwrap_or_default();
        if content.is_empty() {
            anyhow::bail!("LLM returned empty response");
        }
        Ok(content)
    }

    pub async fn chat_stream_with_messages(&self, messages_json: &str) -> Result<StreamResult> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": serde_json::from_str::<serde_json::Value>(messages_json).unwrap_or_default(),
            "max_tokens": 4096,
            "temperature": 0.7,
            "stream": true,
        })
        .to_string();

        let resp = self
            .client
            .post(&self.api_endpoint)
            .bearer_auth(&self.api_key)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await?;

        let status = resp.status();
        eprintln!("[llm] chat_stream_with_messages response status: {status}");
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            eprintln!("[llm] error response body: {body}");
            anyhow::bail!("LLM API error {status}: {body}");
        }

        use std::cell::RefCell;

        let buf = RefCell::new(String::new());

        let stream = resp.bytes_stream().map(move |chunk| match chunk {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes).to_string();
                let mut content = String::new();

                let mut prev = buf.borrow_mut();
                let merged = format!("{}{}", *prev, text);

                let mut lines: Vec<&str> = merged.lines().collect();

                if merged.ends_with('\n') {
                    prev.clear();
                } else {
                    *prev = lines.pop().unwrap_or("").to_string();
                }

                for line in lines {
                    if line.starts_with("data: ") {
                        let data = line.trim_start_matches("data: ");
                        if data == "[DONE]" {
                            if !prev.is_empty() {
                                let remaining = prev.clone();
                                prev.clear();
                                if remaining.starts_with("data: ") {
                                    let remaining_data = remaining.trim_start_matches("data: ");
                                    if remaining_data != "[DONE]" {
                                        if let Ok(chunk) =
                                            serde_json::from_str::<StreamChunk>(remaining_data)
                                        {
                                            if let Some(choices) = chunk.choices {
                                                for choice in choices {
                                                    if let Some(delta) = choice.delta {
                                                        if let Some(c) = delta.content {
                                                            content.push_str(&c);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            continue;
                        }
                        if let Ok(chunk) = serde_json::from_str::<StreamChunk>(data) {
                            if let Some(choices) = chunk.choices {
                                for choice in choices {
                                    if let Some(delta) = choice.delta {
                                        if let Some(c) = delta.content {
                                            content.push_str(&c);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if !content.is_empty() {
                    Ok(content)
                } else {
                    Ok(String::new())
                }
            }
            Err(e) => Err(anyhow::anyhow!("Stream error: {}", e)),
        });

        Ok(Box::pin(stream))
    }

    pub async fn chat_stream(&self, prompt: &str) -> Result<StreamResult> {
        let req = ChatRequest {
            model: self.model.clone(),
            messages: vec![Message {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            max_tokens: 4096,
            temperature: 0.7,
            stream: true,
        };

        let resp = self
            .client
            .post(&self.api_endpoint)
            .bearer_auth(&self.api_key)
            .json(&req)
            .send()
            .await?;

        use std::cell::RefCell;
        let buf = RefCell::new(String::new());

        let stream = resp.bytes_stream().map(move |chunk| match chunk {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes).to_string();
                let mut content = String::new();

                let mut prev = buf.borrow_mut();
                let merged = format!("{}{}", *prev, text);

                let mut lines: Vec<&str> = merged.lines().collect();
                if merged.ends_with('\n') {
                    prev.clear();
                } else {
                    *prev = lines.pop().unwrap_or("").to_string();
                }

                for line in lines {
                    if line.starts_with("data: ") {
                        let data = line.trim_start_matches("data: ");
                        if data == "[DONE]" {
                            continue;
                        }
                        if let Ok(chunk) = serde_json::from_str::<StreamChunk>(data) {
                            if let Some(choices) = chunk.choices {
                                for choice in choices {
                                    if let Some(delta) = choice.delta {
                                        if let Some(c) = delta.content {
                                            content.push_str(&c);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if !content.is_empty() {
                    Ok(content)
                } else {
                    Ok(String::new())
                }
            }
            Err(e) => Err(anyhow::anyhow!("Stream error: {}", e)),
        });

        Ok(Box::pin(stream))
    }

    pub async fn chat_stream_with_tools(
        &self,
        messages: &serde_json::Value,
        tools: &serde_json::Value,
    ) -> Result<ToolStreamResult> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "max_tokens": 4096,
            "temperature": 0.7,
            "stream": true,
            "tools": tools,
        });

        let resp = self
            .client
            .post(&self.api_endpoint)
            .bearer_auth(&self.api_key)
            .header("Content-Type", "application/json")
            .body(body.to_string())
            .send()
            .await?;

        let status = resp.status();
        eprintln!("[llm] chat_stream_with_tools response status: {status}");
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            eprintln!("[llm] error response body: {body}");
            anyhow::bail!("LLM API error {status}: {body}");
        }

        use std::cell::RefCell;
        let buf = RefCell::new(String::new());

        let stream = resp.bytes_stream().map(move |chunk| {
            let text = String::from_utf8_lossy(&chunk?).to_string();
            let mut events: Vec<StreamEvent> = Vec::new();

            let mut prev = buf.borrow_mut();
            let merged = format!("{}{}", *prev, text);

            let mut lines: Vec<&str> = merged.lines().collect();
            if merged.ends_with('\n') {
                prev.clear();
            } else {
                *prev = lines.pop().unwrap_or("").to_string();
            }

            for line in lines {
                if !line.starts_with("data: ") {
                    continue;
                }
                let data = line.trim_start_matches("data: ");
                if data == "[DONE]" {
                    continue;
                }
                let chunk: StreamChunk = match serde_json::from_str(data) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let Some(choices) = chunk.choices else {
                    continue;
                };
                for choice in choices {
                    let Some(delta) = choice.delta else { continue };

                    if let Some(content) = delta.content {
                        if !content.is_empty() {
                            events.push(StreamEvent::Text(content));
                        }
                    }

                    if let Some(rc) = delta.reasoning_content {
                        if !rc.is_empty() {
                            events.push(StreamEvent::ReasoningText(rc));
                        }
                    }

                    if let Some(tc_deltas) = delta.tool_calls {
                        for tc in tc_deltas {
                            let id = tc.id.unwrap_or_default();
                            let func = tc.function.unwrap_or(FunctionDelta {
                                name: None,
                                arguments: None,
                            });
                            let name = func.name.unwrap_or_default();
                            let args = func.arguments.unwrap_or_default();

                            if !id.is_empty() && !name.is_empty() {
                                events.push(StreamEvent::ToolCallStart(ToolCall {
                                    id,
                                    function_name: name,
                                    arguments: args,
                                }));
                            } else if !args.is_empty() {
                                events.push(StreamEvent::ToolCallDelta(ToolCall {
                                    id: String::new(),
                                    function_name: String::new(),
                                    arguments: args,
                                }));
                            }
                        }
                    }
                }
            }

            Ok(events)
        });

        Ok(Box::pin(stream))
    }

    pub async fn chat_with_tools(&self, body_json: &str) -> Result<ToolChatResponse> {
        let resp = self
            .client
            .post(&self.api_endpoint)
            .bearer_auth(&self.api_key)
            .header("Content-Type", "application/json")
            .body(body_json.to_string())
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("LLM API error {status}: {body}");
        }
        let response: ToolChatResponse = resp.json().await?;
        Ok(response)
    }
}

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