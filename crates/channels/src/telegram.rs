use crate::adapter::MessageAdapter;
use crate::event::{AdapterError, MessageEvent, Platform};
use crate::NotificationSender;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::mpsc;

/// 单向通知发送（用于系统推送）。
pub struct TelegramChannel {
    bot_token: String,
    chat_id: String,
    client: Arc<reqwest::Client>,
}

impl TelegramChannel {
    #[must_use]
    pub fn new(bot_token: String, chat_id: String) -> Self {
        Self {
            bot_token,
            chat_id,
            client: Arc::new(reqwest::Client::new()),
        }
    }
}

#[async_trait]
impl NotificationSender for TelegramChannel {
    async fn send_text(&self, text: &str) -> Result<()> {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token);
        let stripped = text.replace(['*', '_'], "");
        let resp = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "chat_id": self.chat_id,
                "text": stripped,
            }))
            .send()
            .await
            .context("telegram sendMessage request failed")?;

        let status = resp.status();
        if status.is_client_error() || status.is_server_error() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("telegram sendMessage returned {status}: {body}");
        }
        Ok(())
    }
}

/// Telegram 适配器——通过 long polling 接收消息并转发为统一事件。
pub struct TelegramAdapter {
    bot_token: String,
    allowed_chat_id: String,
    client: reqwest::Client,
    stop_signal: Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Deserialize)]
struct GetUpdatesResponse {
    ok: bool,
    error_code: Option<i64>,
    description: Option<String>,
    result: Option<Vec<Update>>,
}

#[derive(Deserialize)]
struct Update {
    update_id: i64,
    message: Option<TgMessage>,
}

#[derive(Deserialize)]
struct TgMessage {
    message_id: i64,
    chat: TgChat,
    from: Option<TgUser>,
    text: Option<String>,
}

#[derive(Deserialize)]
struct TgChat {
    id: i64,
}

#[derive(Deserialize)]
struct TgUser {
    id: i64,
    first_name: Option<String>,
    username: Option<String>,
}

#[derive(Deserialize)]
struct SendResp {
    result: Option<SendResult>,
}
#[derive(Deserialize)]
struct SendResult {
    message_id: i64,
}

impl TelegramAdapter {
    #[must_use]
    pub fn new(bot_token: String, allowed_chat_id: String) -> Self {
        Self {
            bot_token,
            allowed_chat_id,
            client: reqwest::Client::new(),
            stop_signal: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// 发送文本到指定 `chat_id`。
    async fn send_text_to(&self, chat_id: &str, text: &str) -> Result<i64, AdapterError> {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token);
        let stripped = text.replace(['*', '_'], "");
        let resp = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "chat_id": chat_id,
                "text": stripped,
            }))
            .send()
            .await
            .map_err(|e| AdapterError::SendFailed(format!("telegram request: {e}")))?;

        let status = resp.status();
        if status.is_client_error() || status.is_server_error() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AdapterError::SendFailed(format!("{status}: {body}")));
        }

        let body: SendResp = resp
            .json()
            .await
            .map_err(|e| AdapterError::SendFailed(format!("parse response: {e}")))?;

        Ok(body.result.map_or(0, |r| r.message_id))
    }
}

#[async_trait]
impl MessageAdapter for TelegramAdapter {
    fn platform_name(&self) -> &'static str {
        "telegram"
    }

    async fn start(&self, sender: mpsc::Sender<MessageEvent>) -> Result<(), AdapterError> {
        let bot_token = self.bot_token.clone();
        let allowed_chat_id = self.allowed_chat_id.clone();
        let client = self.client.clone();
        let stop_signal = self.stop_signal.clone();

        tracing::info!("[telegram-adapter] polling started");

        // 注册命令列表
        let cmd_url = format!("https://api.telegram.org/bot{bot_token}/setMyCommands");
        let _ = self
            .client
            .post(&cmd_url)
            .json(&serde_json::json!({
                "commands": [
                    {"command": "ask", "description": "向角色提问 /ask role_name question"},
                    {"command": "new", "description": "开始新对话"},
                    {"command": "decision", "description": "触发每日决策"},
                ]
            }))
            .send()
            .await;

        let mut offset: i64 = 0;

        loop {
            // 检查是否应停止
            if stop_signal.load(std::sync::atomic::Ordering::Acquire) {
                tracing::info!("[telegram-adapter] polling stopped");
                break;
            }

            let url = format!(
                "https://api.telegram.org/bot{bot_token}/getUpdates?offset={offset}&timeout=30"
            );

            let resp = match client.get(&url).send().await {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("[telegram-adapter] getUpdates error: {e}");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
            };

            let text = match resp.text().await {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("[telegram-adapter] read body error: {e}");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
            };

            let body: GetUpdatesResponse = match serde_json::from_str(&text) {
                Ok(b) => b,
                Err(e) => {
                    tracing::error!(
                        "[telegram-adapter] parse error: {e}, body: {}",
                        &text[..text.len().min(500)]
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
            };

            if !body.ok {
                let desc = body.description.as_deref().unwrap_or("unknown");
                let code = body.error_code.map(|c| c.to_string()).unwrap_or_default();
                tracing::error!("[telegram-adapter] getUpdates error {code}: {desc}");
                let delay = if body.error_code == Some(409) { 30 } else { 5 };
                tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                continue;
            }

            if let Some(updates) = body.result {
                for update in updates {
                    offset = update.update_id + 1;

                    if let Some(msg) = update.message {
                        if let Some(text) = msg.text {
                            let chat_id = msg.chat.id.to_string();
                            if chat_id != allowed_chat_id {
                                continue;
                            }

                            let user_name = msg
                                .from
                                .as_ref()
                                .and_then(|u| u.username.clone().or_else(|| u.first_name.clone()));

                            let event = MessageEvent {
                                platform: Platform::Telegram,
                                channel_id: chat_id,
                                user_id: msg
                                    .from
                                    .as_ref()
                                    .map(|u| u.id.to_string())
                                    .unwrap_or_default(),
                                user_name,
                                text,
                                message_id: msg.message_id.to_string(),
                                raw: serde_json::Value::Null,
                            };

                            if sender.send(event).await.is_err() {
                                tracing::error!("[telegram-adapter] receiver dropped, stopping");
                                break;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn stop(&self) -> Result<(), AdapterError> {
        self.stop_signal
            .store(true, std::sync::atomic::Ordering::Release);
        tracing::info!("[telegram-adapter] stop signal sent");
        Ok(())
    }

    async fn reply(&self, event: &MessageEvent, text: &str) -> Result<(), AdapterError> {
        self.send_text_to(&event.channel_id, text).await?;
        Ok(())
    }
}
