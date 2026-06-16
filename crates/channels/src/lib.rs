use anyhow::Result;
use async_trait::async_trait;

pub mod discord;
pub mod lark;
pub mod slack;
pub mod telegram;

// ── Channel (low-level transport) ──────────────────────

#[async_trait]
pub trait Channel: Send + Sync {
    async fn send_text(&self, text: &str) -> Result<()>;
}

pub struct NoopChannel;

#[async_trait]
impl Channel for NoopChannel {
    async fn send_text(&self, _: &str) -> Result<()> {
        Ok(())
    }
}

pub struct StdoutChannel;

#[async_trait]
impl Channel for StdoutChannel {
    async fn send_text(&self, text: &str) -> Result<()> {
        println!("{}", text);
        println!("{}", "─".repeat(40));
        Ok(())
    }
}

// ── NotificationSender (mid-level event routing) ───────

#[async_trait]
pub trait NotificationSender: Send + Sync {
    async fn send_text(&self, text: &str) -> Result<()>;

    async fn notify_collect(&self, text: &str) -> Result<()> {
        self.send_text(text).await
    }
    async fn notify_decision(&self, text: &str) -> Result<()> {
        self.send_text(text).await
    }
    async fn notify_trade(&self, text: &str) -> Result<()> {
        self.send_text(text).await
    }
    async fn notify_news(&self, text: &str) -> Result<()> {
        self.send_text(text).await
    }
}

/// Every [`Channel`] automatically implements [`NotificationSender`].
/// New social platforms only need to implement [`Channel::send_text`].
#[async_trait]
impl<T: Channel + Send + Sync> NotificationSender for T {
    async fn send_text(&self, text: &str) -> Result<()> {
        Channel::send_text(self, text).await
    }
}
