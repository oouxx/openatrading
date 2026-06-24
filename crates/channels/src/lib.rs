use anyhow::Result;
use async_trait::async_trait;

pub mod adapter;
pub mod circuit_breaker;
pub mod event;
pub mod session;

pub mod discord;
pub mod lark;
pub mod slack;
pub mod telegram;

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

pub struct NoopChannel;

#[async_trait]
impl NotificationSender for NoopChannel {
    async fn send_text(&self, _: &str) -> Result<()> {
        Ok(())
    }
}

pub struct StdoutChannel;

#[async_trait]
impl NotificationSender for StdoutChannel {
    async fn send_text(&self, text: &str) -> Result<()> {
        println!("{}", text);
        println!("{}", "─".repeat(40));
        Ok(())
    }
}
