use anyhow::{Context, Result};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::NotificationSender;

const LARK_API_BASE: &str = "https://open.feishu.cn/open-apis";
const LARK_MAX_CONTENT_LEN: usize = 28000;

struct CachedToken {
    token: String,
    expires_at: tokio::time::Instant,
}

pub struct LarkChannel {
    app_id: String,
    app_secret: String,
    receive_id: String,
    client: Arc<reqwest::Client>,
    cached_token: Arc<RwLock<Option<CachedToken>>>,
}

impl LarkChannel {
    pub fn new(app_id: String, app_secret: String, receive_id: String) -> Self {
        Self {
            app_id,
            app_secret,
            receive_id,
            client: Arc::new(reqwest::Client::new()),
            cached_token: Arc::new(RwLock::new(None)),
        }
    }

    async fn get_tenant_access_token(&self) -> Result<String> {
        {
            let cache = self.cached_token.read().await;
            if let Some(cached) = cache.as_ref() {
                if cached.expires_at > tokio::time::Instant::now() {
                    return Ok(cached.token.clone());
                }
            }
        }

        let url = format!("{}/auth/v3/tenant_access_token/internal", LARK_API_BASE);
        let resp = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "app_id": self.app_id,
                "app_secret": self.app_secret,
            }))
            .send()
            .await
            .context("lark get tenant_access_token failed")?;

        let status = resp.status();
        if status.is_client_error() || status.is_server_error() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("lark auth returned {}: {}", status, body);
        }

        let body: serde_json::Value = resp.json().await.unwrap_or_default();
        let code = body["code"].as_i64().unwrap_or(-1);
        if code != 0 {
            let msg = body["msg"].as_str().unwrap_or("unknown");
            anyhow::bail!("lark auth error code {}: {}", code, msg);
        }

        let token = body["tenant_access_token"]
            .as_str()
            .context("lark auth missing tenant_access_token")?
            .to_string();
        let expire = body["expire"].as_i64().unwrap_or(7200);
        let ttl = expire.max(60) as u64 - 120;

        {
            let mut cache = self.cached_token.write().await;
            *cache = Some(CachedToken {
                token: token.clone(),
                expires_at: tokio::time::Instant::now() + tokio::time::Duration::from_secs(ttl),
            });
        }

        Ok(token)
    }

    async fn invalidate_token(&self) {
        let mut cache = self.cached_token.write().await;
        *cache = None;
    }

    async fn send_chunk(&self, text: &str) -> Result<()> {
        let token = self.get_tenant_access_token().await?;

        let url = format!("{}/im/v1/messages?receive_id_type=chat_id", LARK_API_BASE);
        let content = serde_json::json!({ "text": text }).to_string();
        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .json(&serde_json::json!({
                "receive_id": self.receive_id,
                "msg_type": "text",
                "content": content,
            }))
            .send()
            .await
            .context("lark bot send message failed")?;

        let status = resp.status();
        if status.is_client_error() || status.is_server_error() {
            let body = resp.text().await.unwrap_or_default();
            if status.as_u16() == 401 {
                self.invalidate_token().await;
            }
            anyhow::bail!("lark bot returned {}: {}", status, body);
        }

        let body: serde_json::Value = resp.json().await.unwrap_or_default();
        let code = body["code"].as_i64().unwrap_or(-1);
        if code != 0 {
            let msg = body["msg"].as_str().unwrap_or("unknown");
            if code == 99991663 {
                self.invalidate_token().await;
            }
            anyhow::bail!("lark bot api error code {}: {}", code, msg);
        }
        Ok(())
    }
}

#[async_trait]
impl NotificationSender for LarkChannel {
    async fn send_text(&self, text: &str) -> Result<()> {
        for chunk in split_lark_chunks(text, LARK_MAX_CONTENT_LEN) {
            self.send_chunk(chunk).await?;
        }
        Ok(())
    }
}

fn split_lark_chunks(text: &str, max_len: usize) -> Vec<&str> {
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
