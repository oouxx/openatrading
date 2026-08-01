//! Slack 通道实现。
//!
//! 包含：
//! - [`SlackChannel`]：单向通知发送（`NotificationSender`）
//! - [`SlackAdapter`]：双向通道（`Channel`）

use crate::event::{AdapterError, MessageEvent};
use crate::{Channel, NotificationSender};
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

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
        let url = format!("{SLACK_API_BASE}/chat.postMessage");
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
            anyhow::bail!("slack bot returned {status}: {body}");
        }

        let body: serde_json::Value = resp.json().await.unwrap_or_default();
        if body["ok"].as_bool() != Some(true) {
            let error = body["error"].as_str().unwrap_or("unknown");
            anyhow::bail!("slack bot api error: {error}");
        }
        Ok(())
    }
}

/// Slack 双向通道（当前仅支持推送，监听待实现）。
pub struct SlackAdapter {
    inner: SlackChannel,
}

impl SlackAdapter {
    pub fn new(bot_token: String, channel: String) -> Self {
        Self {
            inner: SlackChannel::new(bot_token, channel),
        }
    }
}

#[async_trait]
impl Channel for SlackAdapter {
    fn name(&self) -> &'static str {
        "slack"
    }

    async fn listen(
        &self,
        _sender: mpsc::Sender<MessageEvent>,
        _cancel: CancellationToken,
    ) -> Result<(), AdapterError> {
        // Slack Events API 需要公网 HTTP endpoint，当前未实现。
        // 可通过 SlackAdapter 子类或 webhook 通道扩展。
        tracing::warn!("[slack] listen not implemented (requires Events API endpoint)");
        futures_util::future::pending::<()>().await;
        Ok(())
    }

    async fn reply(&self, _event: &MessageEvent, text: &str) -> Result<(), AdapterError> {
        self.inner
            .send_text(text)
            .await
            .map_err(|e| AdapterError::SendFailed(e.to_string()))
    }
}
