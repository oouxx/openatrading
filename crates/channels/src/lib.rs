use anyhow::Result;
use async_trait::async_trait;

pub mod discord;
pub mod lark;
pub mod slack;
pub mod telegram;

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