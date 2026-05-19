use anyhow::Result;
use async_trait::async_trait;

use super::NotificationSender;

pub struct LarkSender {
    pub webhook_url: String,
}

impl LarkSender {
    pub async fn send_message(&self, text: &str) -> Result<()> {
        let client = reqwest::Client::new();
        client
            .post(&self.webhook_url)
            .json(&serde_json::json!({
                "msg_type": "interactive",
                "card": {
                    "elements": [{ "tag": "markdown", "content": text }]
                }
            }))
            .send()
            .await?;
        Ok(())
    }
}

#[async_trait]
impl NotificationSender for LarkSender {
    async fn send_text(&self, text: &str) -> Result<()> {
        self.send_message(text).await
    }
}