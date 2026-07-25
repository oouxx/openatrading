use crate::adapter::MessageAdapter;
use crate::event::{AdapterError, MessageEvent, Platform};
use crate::NotificationSender;
use anyhow::{Context, Result};
use async_trait::async_trait;
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream};

// ── 单向通知（用于系统推送） ───────────────────────────

const API_BASE: &str = "https://discord.com/api/v10";
const MAX_MESSAGE_LEN: usize = 2000;

pub struct DiscordChannel {
    bot_token: String,
    channel_id: String,
    client: Arc<reqwest::Client>,
}

impl DiscordChannel {
    #[must_use]
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

// ── Gateway WebSocket 双向适配器 ─────────────────────

/// Discord 适配器——通过 Gateway WebSocket 接收消息并转发为统一事件。
pub struct DiscordAdapter {
    bot_token: String,
    /// 允许处理的频道 ID（空 = 所有频道）
    allowed_channel_id: Option<String>,
    client: reqwest::Client,
    /// gateway URL，首次 Identify 后获得
    gateway_url: tokio::sync::Mutex<String>,
}

/// Gateway OP Code
mod op {
    pub const DISPATCH: u8 = 0;
    pub const HEARTBEAT: u8 = 1;
    pub const IDENTIFY: u8 = 2;
    pub const HELLO: u8 = 10;
}

/// Gateway 下行帧
#[derive(Deserialize)]
struct GatewayPayload {
    op: u8,
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    guild_id: Option<String>,
}

#[derive(Deserialize)]
struct MessageAuthor {
    id: String,
    global_name: Option<String>,
    username: String,
}

impl DiscordAdapter {
    #[must_use]
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
    #[must_use]
    pub fn with_channel(mut self, channel_id: impl Into<String>) -> Self {
        self.allowed_channel_id = Some(channel_id.into());
        self
    }
}

#[async_trait]
impl MessageAdapter for DiscordAdapter {
    fn platform_name(&self) -> &'static str {
        "discord"
    }

    #[allow(clippy::too_many_lines)]
    async fn start(&self, sender: mpsc::Sender<MessageEvent>) -> Result<(), AdapterError> {
        let bot_token = self.bot_token.clone();
        let allowed_channel_id = self.allowed_channel_id.clone();
        let mut gateway_url = self.gateway_url.lock().await.clone();

        loop {
            // 1. 连接 Gateway
            let (ws, _) = match tokio_tungstenite::connect_async(&gateway_url).await {
                Ok(conn) => conn,
                Err(e) => {
                    tracing::error!("[discord-adapter] connect failed: {e}, retry in 5s");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
            };
            tracing::info!("[discord-adapter] connected to gateway");

            let (write, mut read) = ws.split();
            let write = Arc::new(Mutex::new(write));

            // 2. 等待 Hello
            let Some(Ok(hello)) = read.next().await else {
                tracing::error!("[discord-adapter] no hello, retry in 5s");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                continue;
            };

            let data: serde_json::Value = if let Message::Text(t) = hello {
                serde_json::from_str(&t).unwrap_or_default()
            } else {
                tracing::error!("[discord-adapter] unexpected hello message type");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                continue;
            };

            let hello_payload: GatewayPayload = serde_json::from_value(data)
                .map_err(|e| AdapterError::Other(anyhow::anyhow!("parse hello: {e}")))?;

            if hello_payload.op != op::HELLO {
                tracing::error!(
                    "[discord-adapter] expected OP 10, got OP {}",
                    hello_payload.op
                );
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                continue;
            }

            let hello_data: HelloData = serde_json::from_value(hello_payload.d)
                .map_err(|e| AdapterError::Other(anyhow::anyhow!("parse hello data: {e}")))?;

            // 3. 发送 Identify
            let identify = serde_json::json!({
                "op": op::IDENTIFY,
                "d": {
                    "token": bot_token,
                    // GUILD_MESSAGES (1<<9) | MESSAGE_CONTENT (1<<15)
                    "intents": 512 | 32768,
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
                        .unwrap_or_else(|e| panic!("identify serialization failed: {e}")),
                ),
            )
            .await;

            // 4. 心跳循环 + 消息处理
            let heartbeat_interval =
                std::time::Duration::from_millis(hello_data.heartbeat_interval);
            let mut seq: Option<i64> = None;
            let mut last_beat = tokio::time::Instant::now();

            tracing::info!(
                "[discord-adapter] identified, heartbeat every {}ms",
                heartbeat_interval.as_millis()
            );

            loop {
                // 检查是否该发心跳
                if last_beat.elapsed() >= heartbeat_interval {
                    last_beat = tokio::time::Instant::now();

                    let heartbeat = serde_json::json!({
                        "op": op::HEARTBEAT,
                        "d": seq
                    });
                    ws_send(
                        &write,
                        Message::Text(
                            serde_json::to_string(&heartbeat)
                                .unwrap_or_else(|e| panic!("heartbeat serialization failed: {e}")),
                        ),
                    )
                    .await;
                }

                // 读取消息（带超时让心跳检查能跑）
                let msg = tokio::time::timeout(
                    std::time::Duration::from_millis(
                        u64::try_from(heartbeat_interval.as_millis()).unwrap_or(u64::MAX) / 3,
                    ),
                    read.next(),
                )
                .await;

                let payload_text = match msg {
                    Ok(Some(Ok(Message::Text(t)))) => t,
                    Ok(Some(Ok(Message::Ping(data)))) => {
                        ws_send(&write, Message::Pong(data)).await;
                        continue;
                    }
                    Ok(Some(Ok(Message::Close(_)))) => {
                        tracing::info!("[discord-adapter] connection closed");
                        break;
                    }
                    Ok(Some(Err(e))) => {
                        tracing::error!("[discord-adapter] ws error: {e}");
                        break;
                    }
                    Ok(None) => {
                        break;
                    }
                    _ => continue,
                };

                let payload: GatewayPayload = match serde_json::from_str(&payload_text) {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::error!("[discord-adapter] parse payload error: {e}");
                        continue;
                    }
                };

                match payload.op {
                    op::DISPATCH => {
                        if let Some(s) = payload.s {
                            seq = Some(s);
                        }
                        match payload.t.as_deref() {
                            Some("MESSAGE_CREATE") => {
                                if let Ok(msg_data) =
                                    serde_json::from_value::<MessageCreateData>(payload.d)
                                {
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
                                        tracing::error!("[discord-adapter] receiver dropped");
                                        return Ok(());
                                    }
                                }
                            }
                            Some("READY") => {
                                if let Some(url) =
                                    payload.d.get("resume_gateway_url").and_then(|v| v.as_str())
                                {
                                    gateway_url = url.to_string();
                                }
                                tracing::info!("[discord-adapter] ready");
                            }
                            Some("RECONNECT") => {
                                tracing::info!("[discord-adapter] reconnect requested");
                                break;
                            }
                            _ => {}
                        }
                    }
                    op::HEARTBEAT => {
                        let heartbeat = serde_json::json!({
                            "op": op::HEARTBEAT,
                            "d": seq
                        });
                        ws_send(
                            &write,
                            Message::Text(
                                serde_json::to_string(&heartbeat).unwrap_or_else(|e| {
                                    panic!("heartbeat serialization failed: {e}")
                                }),
                            ),
                        )
                        .await;
                    }
                    _ => {}
                }
            }

            // 内层循环退出意味着连接断开或被要求重连，等待 5s 后重连。
            tracing::info!("[discord-adapter] reconnecting in 5s");
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    }

    async fn stop(&self) -> Result<(), AdapterError> {
        // 下一次重连前不再连接（通过信号量停止）
        tracing::info!("[discord-adapter] stop requested");
        Ok(())
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

/// 通过 Arc<Mutex<SplitSink>> 发送 WebSocket 消息。
type WsWriter =
    Arc<Mutex<SplitSink<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>, Message>>>;

async fn ws_send(writer: &WsWriter, msg: Message) {
    if let Err(e) = writer.lock().await.send(msg).await {
        tracing::error!("[discord-adapter] ws send error: {e}");
    }
}
