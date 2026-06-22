use anyhow::{Context, Result};
use async_trait::async_trait;
use std::sync::Arc;

use crate::NotificationSender;

const SLACK_API_BASE: &str = "https://slack.com/api";

pub struct SlackChannel {
    bot_token: String,
    channel: String,
    client: Arc<reqwest::Client>,
}

impl SlackChannel {
    pub fn new(bot_token: String, channel: String) -> Self {
        Self {
            bot_token,
            channel,
            client: Arc::new(reqwest::Client::new()),
        }
    }
}

#[async_trait]
impl NotificationSender for SlackChannel {
    async fn send_text(&self, text: &str) -> Result<()> {
        let url = format!("{}/chat.postMessage", SLACK_API_BASE);
        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.bot_token))
            .json(&serde_json::json!({
                "channel": self.channel,
                "text": text,
            }))
            .send()
            .await
            .context("slack bot send message failed")?;

        let status = resp.status();
        if status.is_client_error() || status.is_server_error() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("slack bot returned {}: {}", status, body);
        }

        let body: serde_json::Value = resp.json().await.unwrap_or_default();
        if body["ok"].as_bool() != Some(true) {
            let error = body["error"].as_str().unwrap_or("unknown");
            anyhow::bail!("slack bot api error: {}", error);
        }
        Ok(())
    }
}
