use anyhow::Result;
use async_trait::async_trait;

use crate::cron::{CollectSummary, Decision};
use crate::trade::TradeLog;

pub use trending_agent_channels as channels;
pub use trending_agent_channels::discord::DiscordChannel;
pub use trending_agent_channels::lark::LarkChannel;
pub use trending_agent_channels::slack::SlackChannel;
pub use trending_agent_channels::telegram::TelegramChannel;
pub use trending_agent_channels::{Channel, NoopChannel, StdoutChannel};

#[async_trait]
pub trait NotificationSender: Send + Sync {
    async fn send_text(&self, text: &str) -> Result<()>;

    async fn notify_collect(&self, summary: &CollectSummary) -> Result<()> {
        let text = format!(
            "📊 采集完成\n时间: {}\n数据源: {}\n快照: {} 条",
            summary.timestamp.format("%H:%M:%S"),
            summary.sources.join(", "),
            summary.snapshot_count,
        );
        self.send_text(&text).await
    }

    async fn notify_decision(&self, decision: &Decision) -> Result<()> {
        self.send_text(&decision.summary).await
    }

    async fn notify_trade(&self, log: &TradeLog) -> Result<()> {
        let text = format!(
            "💹 持仓变动\n操作: {}\n价格: {}\n股数: {}\n理由: {}",
            log.action, log.price, log.shares, log.reason,
        );
        self.send_text(&text).await
    }

    async fn notify_news(&self, text: &str) -> Result<()> {
        self.send_text(text).await
    }
}

pub struct NoopSender;

#[async_trait]
impl NotificationSender for NoopSender {
    async fn send_text(&self, _: &str) -> Result<()> {
        Ok(())
    }
}

pub struct StdoutSender;

#[async_trait]
impl NotificationSender for StdoutSender {
    async fn send_text(&self, text: &str) -> Result<()> {
        println!("{}", text);
        println!("{}", "─".repeat(40));
        Ok(())
    }
}

pub struct FilteredSender {
    pub inner: Box<dyn NotificationSender>,
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

    async fn notify_collect(&self, summary: &CollectSummary) -> Result<()> {
        if self.notify_on_collect {
            self.inner.notify_collect(summary).await
        } else {
            Ok(())
        }
    }

    async fn notify_decision(&self, decision: &Decision) -> Result<()> {
        if self.notify_on_decision {
            self.inner.notify_decision(decision).await
        } else {
            Ok(())
        }
    }

    async fn notify_trade(&self, log: &TradeLog) -> Result<()> {
        if self.notify_on_trade {
            self.inner.notify_trade(log).await
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

struct ChannelAdapter {
    channel: Box<dyn Channel>,
}

#[async_trait]
impl NotificationSender for ChannelAdapter {
    async fn send_text(&self, text: &str) -> Result<()> {
        self.channel.send_text(text).await
    }
}

pub fn channel_to_sender(channel: Box<dyn Channel>) -> Box<dyn NotificationSender> {
    Box::new(ChannelAdapter { channel })
}

pub mod telegram_bot;