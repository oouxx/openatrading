// platform/web.rs
// Web platform — no emitter or persister needed.
// Events are sent directly via the SSE/WebSocket connection in the handler.

use crate::social::NotificationSender;

pub struct WebPushNotifier {
    user_id: String,
}

impl WebPushNotifier {
    pub fn new(user_id: &str) -> Self {
        Self {
            user_id: user_id.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl NotificationSender for WebPushNotifier {
    async fn send_text(&self, text: &str) -> anyhow::Result<()> {
        tracing::info!("[web-push] send to {}: {}", self.user_id, text);
        Ok(())
    }

    async fn notify_collect(&self, text: &str) -> anyhow::Result<()> {
        self.send_text(text).await
    }

    async fn notify_decision(&self, text: &str) -> anyhow::Result<()> {
        self.send_text(text).await
    }

    async fn notify_trade(&self, text: &str) -> anyhow::Result<()> {
        self.send_text(text).await
    }

    async fn notify_news(&self, text: &str) -> anyhow::Result<()> {
        self.send_text(text).await
    }
}
