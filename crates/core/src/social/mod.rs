use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

// Re-export everything from channels crate
pub use openatrading_channels as channels;
pub use openatrading_channels::discord::DiscordChannel;
pub use openatrading_channels::lark::LarkChannel;
pub use openatrading_channels::slack::SlackChannel;
pub use openatrading_channels::telegram::TelegramChannel;
pub use openatrading_channels::{NoopChannel, NotificationSender, StdoutChannel};

// ── FilteredSender ────────────────────────────────────

pub struct FilteredSender {
    pub inner: Arc<dyn NotificationSender>,
    pub notify_on_collect: bool,
    pub notify_on_decision: bool,
    pub notify_on_trade: bool,
    pub notify_on_news: bool,
}

#[async_trait]
impl NotificationSender for FilteredSender {
    async fn send_text(&self, text: &str) -> Result<()> {
        self.inner.send_text(text).await
    }

    async fn notify_collect(&self, text: &str) -> Result<()> {
        if self.notify_on_collect {
            self.inner.notify_collect(text).await
        } else {
            Ok(())
        }
    }

    async fn notify_decision(&self, text: &str) -> Result<()> {
        if self.notify_on_decision {
            self.inner.notify_decision(text).await
        } else {
            Ok(())
        }
    }

    async fn notify_trade(&self, text: &str) -> Result<()> {
        if self.notify_on_trade {
            self.inner.notify_trade(text).await
        } else {
            Ok(())
        }
    }

    async fn notify_news(&self, text: &str) -> Result<()> {
        if self.notify_on_news {
            self.inner.notify_news(text).await
        } else {
            Ok(())
        }
    }
}

pub mod adapter_handler;
pub mod telegram_bot;
