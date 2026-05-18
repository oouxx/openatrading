use anyhow::Result;
use async_trait::async_trait;

use super::NotificationSender;
use crate::cron::{CollectSummary, Decision};
use crate::trade::TradeLog;

/// Telegram sender — 通过 HTTP API 发送消息
pub struct TelegramSender {
    pub bot_token: String,
    pub chat_id: String,
    pub notify_on_collect: bool,
    pub notify_on_decision: bool,
    pub notify_on_trade: bool,
}

impl TelegramSender {
    pub async fn send_message(&self, text: &str) -> Result<()> {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token);
        let client = reqwest::Client::new();
        client
            .post(&url)
            .json(&serde_json::json!({
                "chat_id": self.chat_id,
                "text": text,
                "parse_mode": "Markdown",
            }))
            .send()
            .await?;
        Ok(())
    }
}

#[async_trait]
impl NotificationSender for TelegramSender {
    async fn notify_collect(&self, summary: &CollectSummary) -> Result<()> {
        if !self.notify_on_collect {
            return Ok(());
        }
        let text = format!(
            "📊 *采集完成*\n时间: {}\n数据源: {}\n快照: {} 条",
            summary.timestamp.format("%H:%M:%S"),
            summary.sources.join(", "),
            summary.snapshot_count,
        );
        self.send_message(&text).await?;
        Ok(())
    }

    async fn notify_decision(&self, decision: &Decision) -> Result<()> {
        if !self.notify_on_decision {
            return Ok(());
        }
        self.send_message(&decision.summary).await?;
        Ok(())
    }

    async fn notify_trade(&self, log: &TradeLog) -> Result<()> {
        if !self.notify_on_trade {
            return Ok(());
        }
        let text = format!(
            "💹 *持仓变动*\n操作: {}\n价格: {}\n股数: {}\n理由: {}",
            log.action, log.price, log.shares, log.reason,
        );
        self.send_message(&text).await?;
        Ok(())
    }
}