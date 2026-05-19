use anyhow::Result;
use async_trait::async_trait;

use crate::cron::{CollectSummary, Decision};
use crate::trade::TradeLog;

#[async_trait]
pub trait NotificationSender: Send + Sync {
    async fn notify_collect(&self, summary: &CollectSummary) -> Result<()>;
    async fn notify_decision(&self, decision: &Decision) -> Result<()>;
    async fn notify_trade(&self, log: &TradeLog) -> Result<()>;
}

/// 空实现 — 当 telegram 未启用时使用
pub struct NoopSender;

#[async_trait]
impl NotificationSender for NoopSender {
    async fn notify_collect(&self, _: &CollectSummary) -> Result<()> {
        Ok(())
    }
    async fn notify_decision(&self, _: &Decision) -> Result<()> {
        Ok(())
    }
    async fn notify_trade(&self, _: &TradeLog) -> Result<()> {
        Ok(())
    }
}

pub mod telegram;
pub mod telegram_bot;
