use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

// Re-export everything from channels crate
pub use openatrading_channels as channels;
// TODO: 断路器与限速装饰器已实现（含单测），但从未装配到任何发送路径——
// 通知发送目前既无重试也无速率控制。接入点：`create_notification_sender` 包装返回值。
pub use openatrading_channels::circuit_breaker::{CircuitBreaker, CircuitBreakerChannel};
pub use openatrading_channels::listing;
pub use openatrading_channels::paced_channel::PacedChannel;
pub use openatrading_channels::Channel;

// Platform channels (NotificationSender)
#[cfg(feature = "channel-discord")]
pub use openatrading_channels::discord::DiscordChannel;
#[cfg(feature = "channel-lark")]
pub use openatrading_channels::lark::LarkChannel;
#[cfg(feature = "channel-slack")]
pub use openatrading_channels::slack::SlackChannel;
#[cfg(feature = "channel-telegram")]
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

// ── 模型信息 ──────────────────────────────────────────

/// 读取 agent 设置中的默认模型，返回 `provider/model` 形式。
pub fn current_model_display() -> String {
    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let sm = pi_coding_agent::core::settings_manager::SettingsManager::create(&cwd, None);
    match (sm.get_default_model(), sm.get_default_provider()) {
        (Some(model), Some(provider)) => format!("{provider}/{model}"),
        (Some(model), None) => model.to_string(),
        (None, Some(provider)) => format!("{provider}/(默认)"),
        (None, None) => "未设置".to_string(),
    }
}

// ── 适配器工厂 ──────────────────────────────────────

/// 从 Social 配置构建平台适配器列表。
///
/// 将适配器构造逻辑从 CLI 移到 core，便于复用和测试。
pub fn build_adapters(social: &Option<crate::config::SocialConfig>) -> Vec<Arc<dyn Channel>> {
    use openatrading_channels::discord::DiscordAdapter;
    use openatrading_channels::telegram::TelegramAdapter;

    let mut adapters: Vec<Arc<dyn Channel>> = Vec::new();

    let Some(sc) = social else {
        return adapters;
    };

    #[cfg(feature = "channel-telegram")]
    if let Some(tg) = &sc.telegram {
        if tg.enabled && tg.bot_polling {
            if let (Some(token), Some(chat_id)) = (&tg.bot_token, &tg.chat_id) {
                if !token.is_empty() && !chat_id.is_empty() {
                    adapters.push(Arc::new(TelegramAdapter::new(
                        token.clone(),
                        chat_id.clone(),
                    )));
                    crate::log::info!("telegram adapter: enabled");
                }
            }
        }
    }

    #[cfg(feature = "channel-discord")]
    if let Some(dc) = &sc.discord {
        if dc.enabled && !adapters.iter().any(|a| a.name() == "discord") {
            if let Some(token) = &dc.bot_token {
                if !token.is_empty() {
                    let mut da = DiscordAdapter::new(token.clone());
                    if let Some(ch) = &dc.channel_id {
                        da = da.with_channel(ch);
                    }
                    adapters.push(Arc::new(da));
                    crate::log::info!("discord adapter: enabled");
                }
            }
        }
    }

    adapters
}

pub mod adapter_handler;
pub mod telegram_bot;

pub use telegram_bot::{TelegramBot, TelegramBotConfig};
