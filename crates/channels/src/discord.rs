//! Discord 通道实现。
//!
//! 包含：
//! - [`DiscordChannel`]：单向通知发送（`NotificationSender`）
//! - [`DiscordAdapter`]：双向通道（`Channel`），通过 Gateway WebSocket 接收消息

use crate::event::{AdapterError, MessageEvent, Platform};
use crate::{Channel, NotificationSender};
use anyhow::{Context, Result};
use async_trait::async_trait;
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream};
use tokio_util::sync::CancellationToken;

// ── 单向通知发送 ───────────────────────────────────────────────

const API_BASE: &str = "https://discord.com/api/v10";
const MAX_MESSAGE_LEN: usize = 2000;

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
}

#[async_trait]
impl NotificationSender for DiscordChannel {
    async fn send_text(&self, text: &str) -> Result<()> {
        for chunk in split_chunks(text, MAX_MESSAGE_LEN) {
            self.send_chunk(chunk).await?;
        }
        Ok(())
    }
}

impl DiscordChannel {
    async fn send_chunk(&self, text: &str) -> Result<()> {
        let url = format!("{}/channels/{}/messages", API_BASE, self.channel_id);
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
            anyhow::bail!("discord bot returned {status}: {body}");
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

// ── Gateway WebSocket 双向通道 ────────────────────────────────

/// Discord 适配器——通过 Gateway WebSocket 接收消息并转发为统一事件。
pub struct DiscordAdapter {
    bot_token: String,
    allowed_channel_id: Option<String>,
    client: reqwest::Client,
    gateway_url: tokio::sync::Mutex<String>,
}

/// Gateway OP Code
mod op {
    pub const DISPATCH: u8 = 0;
    pub const HEARTBEAT: u8 = 1;
    pub const IDENTIFY: u8 = 2;
}

/// Gateway 下行帧
#[derive(Deserialize)]
struct GatewayPayload {
    op: u8,
    s: Option<i64>,
    t: Option<String>,
    d: serde_json::Value,
}

/// OP 10 Hello 的数据
#[derive(Deserialize)]
struct HelloData {
    heartbeat_interval: u64,
}

/// `MESSAGE_CREATE` 事件数据
#[derive(Deserialize)]
struct MessageCreateData {
    id: String,
    channel_id: String,
    author: MessageAuthor,
    content: String,
}

#[derive(Deserialize)]
struct MessageAuthor {
    id: String,
    global_name: Option<String>,
    username: String,
}

impl DiscordAdapter {
    pub fn new(bot_token: String) -> Self {
        Self {
            bot_token,
            allowed_channel_id: None,
            client: reqwest::Client::new(),
            gateway_url: tokio::sync::Mutex::new(
                "wss://gateway.discord.gg/?v=10&encoding=json".into(),
            ),
        }
    }

    /// 限制只处理指定频道。
    pub fn with_channel(mut self, channel_id: impl Into<String>) -> Self {
        self.allowed_channel_id = Some(channel_id.into());
        self
    }
}

#[async_trait]
impl Channel for DiscordAdapter {
    fn name(&self) -> &'static str {
        "discord"
    }

    async fn listen(
        &self,
        sender: mpsc::Sender<MessageEvent>,
        cancel: CancellationToken,
    ) -> Result<(), AdapterError> {
        let bot_token = self.bot_token.clone();
        let allowed_channel_id = self.allowed_channel_id.clone();
        let mut gateway_url = self.gateway_url.lock().await.clone();

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("[discord] listen cancelled");
                    return Ok(());
                }
                result = self.run_gateway_loop(&bot_token, &allowed_channel_id, &mut gateway_url, &sender, &cancel) => {
                    if let Err(e) = result {
                        tracing::error!("[discord] gateway loop error: {e}, reconnecting in 5s");
                    } else {
                        tracing::info!("[discord] gateway loop exited, reconnecting in 5s");
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
            }
        }
    }

    /// 通过 REST API 回复消息。
    async fn reply(&self, event: &MessageEvent, text: &str) -> Result<(), AdapterError> {
        let url = format!("{}/channels/{}/messages", API_BASE, event.channel_id);
        for chunk in split_chunks(text, MAX_MESSAGE_LEN) {
            let resp = self
                .client
                .post(&url)
                .header("Authorization", format!("Bot {}", self.bot_token))
                .json(&serde_json::json!({ "content": chunk }))
                .send()
                .await
                .map_err(|e| AdapterError::SendFailed(format!("discord send: {e}")))?;

            let status = resp.status();
            if status.is_client_error() || status.is_server_error() {
                let body = resp.text().await.unwrap_or_default();
                return Err(AdapterError::SendFailed(format!("{status}: {body}")));
            }
        }
        Ok(())
    }
}

impl DiscordAdapter {
    async fn run_gateway_loop(
        &self,
        bot_token: &str,
        allowed_channel_id: &Option<String>,
        gateway_url: &mut String,
        sender: &mpsc::Sender<MessageEvent>,
        cancel: &CancellationToken,
    ) -> Result<(), AdapterError> {
        // 1. 连接 Gateway
        let (ws, _) = tokio_tungstenite::connect_async(&*gateway_url)
            .await
            .map_err(|e| AdapterError::SendFailed(format!("discord ws connect: {e}")))?;
        tracing::info!("[discord] connected to gateway");

        let (write, mut read) = ws.split();
        let write = Arc::new(Mutex::new(write));

        // 2. 等待 Hello
        let hello = read
            .next()
            .await
            .ok_or_else(|| AdapterError::SendFailed("discord ws closed before hello".into()))?
            .map_err(|e| AdapterError::SendFailed(format!("discord ws error: {e}")))?;

        let hello_payload: GatewayPayload = serde_json::from_str(&hello.to_string())
            .map_err(|e| AdapterError::SendFailed(format!("parse hello: {e}")))?;

        let hello_data: HelloData = serde_json::from_value(hello_payload.d)
            .map_err(|e| AdapterError::SendFailed(format!("parse hello data: {e}")))?;

        let heartbeat_interval = hello_data.heartbeat_interval;
        let heartbeat_duration = std::time::Duration::from_millis(heartbeat_interval);

        // 3. Identify
        let identify = serde_json::json!({
            "op": op::IDENTIFY,
            "d": {
                "token": bot_token,
                "intents": 1 << 15, // MESSAGE_CONTENT
                "properties": {
                    "os": "linux",
                    "browser": "openatrading",
                    "device": "openatrading"
                }
            }
        });
        ws_send(
            &write,
            Message::Text(
                serde_json::to_string(&identify)
                    .map_err(|e| AdapterError::SendFailed(format!("serialize identify: {e}")))?,
            ),
        )
        .await;

        // 4. 心跳 + 消息处理循环
        let mut seq: Option<i64> = None;
        let mut last_heartbeat = tokio::time::Instant::now();

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("[discord] gateway loop cancelled");
                    return Ok(());
                }
                msg = read.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            let payload: GatewayPayload = match serde_json::from_str(&text) {
                                Ok(p) => p,
                                Err(e) => {
                                    tracing::warn!("[discord] parse payload error: {e}");
                                    continue;
                                }
                            };

                            if let Some(s) = payload.s {
                                seq = Some(s);
                            }

                            match payload.op {
                                op::DISPATCH => {
                                    match payload.t.as_deref() {
                                        Some("MESSAGE_CREATE") => {
                                            let msg_data: MessageCreateData = match serde_json::from_value(payload.d) {
                                                Ok(m) => m,
                                                Err(e) => {
                                                    tracing::warn!("[discord] parse MESSAGE_CREATE error: {e}");
                                                    continue;
                                                }
                                            };

                                            if let Some(ref allowed) = allowed_channel_id {
                                                if &msg_data.channel_id != allowed {
                                                    continue;
                                                }
                                            }

                                            if msg_data.content.is_empty() {
                                                continue;
                                            }

                                            let user_name = msg_data
                                                .author
                                                .global_name
                                                .or(Some(msg_data.author.username));

                                            let event = MessageEvent {
                                                platform: Platform::Discord,
                                                channel_id: msg_data.channel_id,
                                                user_id: msg_data.author.id,
                                                user_name,
                                                text: msg_data.content,
                                                message_id: msg_data.id,
                                                raw: serde_json::Value::Null,
                                            };

                                            if sender.send(event).await.is_err() {
                                                tracing::error!("[discord] receiver dropped");
                                                return Ok(());
                                            }
                                        }
                                        Some("READY") => {
                                            if let Some(url) =
                                                payload.d.get("resume_gateway_url").and_then(|v| v.as_str())
                                            {
                                                *gateway_url = url.to_string();
                                            }
                                            tracing::info!("[discord] ready");
                                        }
                                        Some("RECONNECT") => {
                                            tracing::info!("[discord] reconnect requested");
                                            return Ok(());
                                        }
                                        _ => {}
                                    }
                                }
                                op::HEARTBEAT => {
                                    let heartbeat = serde_json::json!({
                                        "op": op::HEARTBEAT,
                                        "d": seq
                                    });
                                    let text = serde_json::to_string(&heartbeat)
                                        .map_err(|e| AdapterError::SendFailed(format!("serialize heartbeat: {e}")))?;
                                    ws_send(&write, Message::Text(text)).await;
                                }
                                _ => {}
                            }
                        }
                        Some(Ok(_)) => {}
                        Some(Err(e)) => {
                            tracing::error!("[discord] ws error: {e}");
                            return Err(AdapterError::SendFailed(format!("ws error: {e}")));
                        }
                        None => {
                            tracing::info!("[discord] ws closed");
                            return Ok(());
                        }
                    }
                }
                _ = tokio::time::sleep_until(last_heartbeat + heartbeat_duration) => {
                    // 定期心跳
                    let heartbeat = serde_json::json!({
                        "op": op::HEARTBEAT,
                        "d": seq
                    });
                    let text = serde_json::to_string(&heartbeat)
                        .map_err(|e| AdapterError::SendFailed(format!("serialize heartbeat: {e}")))?;
                    ws_send(&write, Message::Text(text)).await;
                    last_heartbeat = tokio::time::Instant::now();
                }
            }
        }
    }
}

type WsWriter =
    Arc<Mutex<SplitSink<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>, Message>>>;

async fn ws_send(writer: &WsWriter, msg: Message) {
    if let Err(e) = writer.lock().await.send(msg).await {
        tracing::error!("[discord] ws send error: {e}");
    }
}
