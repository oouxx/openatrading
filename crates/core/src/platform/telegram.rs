// platform/telegram.rs
// Telegram platform — no emitter needed.
// Agent events are streamed directly via Telegram Bot API in the handler.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{Duration, Instant};

/// 极简 Telegram Bot API 客户端封装，具体实现按你用的库替换
/// (teloxide / frankenstein / 直接 reqwest 都行)。
#[derive(Clone)]
pub struct TelegramClient {
    bot_token: String,
    http: reqwest::Client,
}

impl TelegramClient {
    pub fn new(bot_token: String) -> Self {
        Self {
            bot_token,
            http: reqwest::Client::new(),
        }
    }

    async fn send_message(&self, chat_id: i64, text: &str) -> anyhow::Result<i64> {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token);
        let resp: serde_json::Value = self
            .http
            .post(&url)
            .json(&serde_json::json!({"chat_id": chat_id, "text": text}))
            .send()
            .await?
            .json()
            .await?;
        Ok(resp["result"]["message_id"].as_i64().unwrap_or_default())
    }

    async fn edit_message(&self, chat_id: i64, message_id: i64, text: &str) -> anyhow::Result<()> {
        let url = format!(
            "https://api.telegram.org/bot{}/editMessageText",
            self.bot_token
        );
        let _ = self
            .http
            .post(&url)
            .json(&serde_json::json!({"chat_id": chat_id, "message_id": message_id, "text": text}))
            .send()
            .await?;
        Ok(())
    }
}

/// 每个 session 对应一次 Telegram 对话轮次的累积状态：
/// 累积的文本、当前在编辑哪条消息、上次编辑时间（用于节流）。
struct StreamBuffer {
    chat_id: i64,
    message_id: Option<i64>,
    accumulated_text: String,
    last_edit_at: Instant,
}

pub struct TelegramStreamer {
    client: TelegramClient,
    chat_id: i64,
    buffers: Arc<Mutex<HashMap<String, StreamBuffer>>>,
    throttle: Duration,
}

impl TelegramStreamer {
    pub fn new(client: TelegramClient, chat_id: i64) -> Self {
        Self {
            client,
            chat_id,
            buffers: Arc::new(Mutex::new(HashMap::new())),
            throttle: Duration::from_millis(1200),
        }
    }

    pub async fn handle_event(&self, session_id: &str, event_type: &str, data: &serde_json::Value) {
        match event_type {
            "message_start" => {
                let mut buffers = self.buffers.lock().await;
                buffers.insert(
                    session_id.to_string(),
                    StreamBuffer {
                        chat_id: self.chat_id,
                        message_id: None,
                        accumulated_text: String::new(),
                        last_edit_at: Instant::now() - self.throttle,
                    },
                );
            },
            "message_update" => {
                let Some(delta) = data.get("delta").and_then(|d| d.as_str()) else {
                    return;
                };
                let mut buffers = self.buffers.lock().await;
                let Some(buf) = buffers.get_mut(session_id) else {
                    return;
                };
                buf.accumulated_text.push_str(delta);

                if buf.last_edit_at.elapsed() < self.throttle {
                    return;
                }
                buf.last_edit_at = Instant::now();

                let text = truncate_for_telegram(&buf.accumulated_text);
                match buf.message_id {
                    None => {
                        if let Ok(mid) = self.client.send_message(buf.chat_id, &text).await {
                            buf.message_id = Some(mid);
                        }
                    },
                    Some(mid) => {
                        let _ = self.client.edit_message(buf.chat_id, mid, &text).await;
                    },
                }
            },
            "message_end" => {
                let mut buffers = self.buffers.lock().await;
                if let Some(buf) = buffers.remove(session_id) {
                    let final_text = buf.accumulated_text;
                    if final_text.chars().count() > 4096 {
                        for chunk in chunk_for_telegram(&final_text) {
                            let _ = self.client.send_message(buf.chat_id, &chunk).await;
                        }
                    } else {
                        match buf.message_id {
                            Some(mid) => {
                                let _ = self
                                    .client
                                    .edit_message(buf.chat_id, mid, &final_text)
                                    .await;
                            },
                            None => {
                                let _ = self.client.send_message(buf.chat_id, &final_text).await;
                            },
                        }
                    }
                }
            },
            _ => {},
        }
    }
}

fn truncate_for_telegram(text: &str) -> String {
    if text.chars().count() > 4096 {
        text.chars().take(4090).collect::<String>() + "..."
    } else {
        text.to_string()
    }
}

fn chunk_for_telegram(text: &str) -> Vec<String> {
    text.chars()
        .collect::<Vec<_>>()
        .chunks(4096)
        .map(|c| c.iter().collect())
        .collect()
}
