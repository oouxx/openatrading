use anyhow::{Context, Result};
use async_trait::async_trait;
use std::sync::Arc;

use crate::Channel;

const DISCORD_API_BASE: &str = "https://discord.com/api/v10";
const DISCORD_MAX_MESSAGE_LEN: usize = 2000;

pub struct DiscordChannel {
    bot_token: String,
    channel_id: String,
    client: Arc<reqwest::Client>,
}

impl DiscordChannel {
    pub fn new(bot_token: String, channel_id: String) -> Self {
        Self {
            bot_token,
            channel_id,
            client: Arc::new(reqwest::Client::new()),
        }
    }

    async fn send_chunk(&self, text: &str) -> Result<()> {
        let url = format!("{}/channels/{}/messages", DISCORD_API_BASE, self.channel_id);
        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bot {}", self.bot_token))
            .json(&serde_json::json!({ "content": text }))
            .send()
            .await
            .context("discord bot send message failed")?;

        let status = resp.status();
        if status.is_client_error() || status.is_server_error() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("discord bot returned {}: {}", status, body);
        }
        Ok(())
    }
}

#[async_trait]
impl Channel for DiscordChannel {
    async fn send_text(&self, text: &str) -> Result<()> {
        for chunk in split_chunks(text, DISCORD_MAX_MESSAGE_LEN) {
            self.send_chunk(chunk).await?;
        }
        Ok(())
    }
}

fn split_chunks(text: &str, max_len: usize) -> Vec<&str> {
    if text.len() <= max_len {
        return vec![text];
    }
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let end = (start + max_len).min(text.len());
        chunks.push(&text[start..end]);
        start = end;
    }
    chunks
}
