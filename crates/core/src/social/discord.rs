use anyhow::Result;
use async_trait::async_trait;

use super::NotificationSender;

pub struct DiscordSender {
    pub webhook_url: String,
}

impl DiscordSender {
    pub async fn send_message(&self, text: &str) -> Result<()> {
        let client = reqwest::Client::new();
        client
            .post(&self.webhook_url)
            .json(&serde_json::json!({ "content": text }))
            .send()
            .await?;
        Ok(())
    }
}

#[async_trait]
impl NotificationSender for DiscordSender {
    async fn send_text(&self, text: &str) -> Result<()> {
        self.send_message(text).await
    }
}
