use anyhow::Result;
use async_trait::async_trait;

use super::NotificationSender;

pub struct TelegramSender {
    pub bot_token: String,
    pub chat_id: String,
}

impl TelegramSender {
    pub async fn send_message(&self, text: &str) -> Result<()> {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token);
        let client = reqwest::Client::new();
        client
            .post(&url)
            .json(&serde_json::json!({
                "chat_id": self.chat_id,
                "text": text,
                "parse_mode": "Markdown",
            }))
            .send()
            .await?;
        Ok(())
    }
}

#[async_trait]
impl NotificationSender for TelegramSender {
    async fn send_text(&self, text: &str) -> Result<()> {
        self.send_message(text).await
    }
}