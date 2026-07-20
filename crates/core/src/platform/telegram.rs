// platform/telegram.rs
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{Duration, Instant};

use crate::llm::runtime::EventEmitter;
use crate::llm::types::{DesktopState, FrontendEvent};

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
        // 忽略 "message is not modified" 之类的错误响应，这是正常情况
        // （文本没变时 Telegram 会报错，属于预期行为不用处理）
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

pub struct TelegramEmitter {
    client: TelegramClient,
    chat_id: i64,
    /// key: session_id，因为同一个 chat 理论上一次只应该有一个 in-flight
    /// 对话，但保留 map 结构方便未来支持同一 chat 并发多个 session。
    buffers: Arc<Mutex<HashMap<String, StreamBuffer>>>,
    throttle: Duration,
}

impl TelegramEmitter {
    pub fn new(client: TelegramClient, chat_id: i64) -> Self {
        Self {
            client,
            chat_id,
            buffers: Arc::new(Mutex::new(HashMap::new())),
            throttle: Duration::from_millis(1200), // 留点余量，别贴着限速线走
        }
    }
}

#[async_trait]
impl EventEmitter for TelegramEmitter {
    async fn emit_agent_event(&self, event: FrontendEvent) {
        match event.event_type.as_str() {
            "message_start" => {
                let mut buffers = self.buffers.lock().await;
                buffers.insert(
                    event.session_id.clone(),
                    StreamBuffer {
                        chat_id: self.chat_id,
                        message_id: None,
                        accumulated_text: String::new(),
                        last_edit_at: Instant::now() - self.throttle, // 允许立刻发第一条
                    },
                );
            },
            "message_update" => {
                let Some(delta) = event.data.get("delta").and_then(|d| d.as_str()) else {
                    return;
                };
                let mut buffers = self.buffers.lock().await;
                let Some(buf) = buffers.get_mut(&event.session_id) else {
                    return;
                };
                buf.accumulated_text.push_str(delta);

                // 节流：距上次编辑不足 throttle 时间就跳过，等下一次 delta
                // 或者 message_end 时再统一发送最新内容。
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
                if let Some(buf) = buffers.remove(&event.session_id) {
                    // 最终内容强制发一次，不受节流限制，保证最后结果是完整的。
                    let final_text = buf.accumulated_text;
                    if final_text.chars().count() > 4096 {
                        // 超长分段发送，前面已经发出的那条消息里的内容作废，
                        // 直接按 4096 字符切块顺序发多条。
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
            "tool_execution_start" => {
                // Telegram 上一般不逐条展示工具调用细节（太吵），
                // 可选：发一个轻量的 "🔧 正在查询..." 提示，或者干脆忽略。
            },
            _ => {},
        }
    }

    async fn emit_state_changed(&self, _state: &DesktopState) {
        // Telegram 没有侧边栏 UI，忽略。
    }

    async fn emit_transcript_changed(
        &self,
        _session_id: &str,
        _transcript: Vec<serde_json::Value>,
    ) {
        // 同上，忽略。
    }
}

fn truncate_for_telegram(text: &str) -> String {
    // 流式过程中的中间状态允许截断显示"...”，最终结果在 message_end 里
    // 会重新处理，不用担心中间截断丢内容。
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
