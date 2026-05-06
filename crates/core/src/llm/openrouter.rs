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
}

#[cfg(not(target_arch = "wasm32"))]
pub type StreamResult = Pin<Box<dyn futures::Stream<Item = Result<String>> + Send>>;
#[cfg(target_arch = "wasm32")]
pub type StreamResult = Pin<Box<dyn futures::Stream<Item = Result<String>>>>;

pub struct LlmClient {
    client: Client,
    api_key: String,
    model: String,
}

impl LlmClient {
    pub fn new(api_key: String) -> Self {
        let model = std::env::var("OPENROUTER_MODEL")
            .unwrap_or_else(|_| "deepseek/deepseek-chat".to_string());
        Self {
            client: Client::new(),
            api_key,
            model,
        }
    }

    pub async fn chat(&self, prompt: &str) -> Result<String> {
        let req = ChatRequest {
            model: self.model.clone(),
            messages: vec![Message {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            max_tokens: 400,
            temperature: 0.7,
            stream: false,
        };

        let resp: ChatResponse = self
            .client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .bearer_auth(&self.api_key)
            .json(&req)
            .send()
            .await?
            .json()
            .await?;

        Ok(resp.choices[0].message.content.clone())
    }

    pub async fn chat_stream(&self, prompt: &str) -> Result<StreamResult> {
        let req = ChatRequest {
            model: self.model.clone(),
            messages: vec![Message {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            max_tokens: 400,
            temperature: 0.7,
            stream: true,
        };

        let resp = self
            .client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .bearer_auth(&self.api_key)
            .json(&req)
            .send()
            .await?;

        let stream = resp.bytes_stream().map(|chunk| match chunk {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes).to_string();
                let mut content = String::new();

                for line in text.lines() {
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
}
