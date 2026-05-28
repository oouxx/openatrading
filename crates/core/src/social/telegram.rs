use anyhow::{Context, Result};
use async_trait::async_trait;
use std::sync::Arc;

use super::NotificationSender;

pub struct TelegramSender {
    pub bot_token: String,
    pub chat_id: String,
    client: Arc<reqwest::Client>,
}

impl TelegramSender {
    pub fn new(bot_token: String, chat_id: String) -> Self {
        Self {
            bot_token,
            chat_id,
            client: Arc::new(reqwest::Client::new()),
        }
    }

    pub async fn send_message(&self, text: &str) -> Result<()> {
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
            anyhow::bail!("telegram sendMessage returned {}: {}", status, body);
        }
        Ok(())
    }
}

#[async_trait]
impl NotificationSender for TelegramSender {
    async fn send_text(&self, text: &str) -> Result<()> {
        self.send_message(text).await
    }
}