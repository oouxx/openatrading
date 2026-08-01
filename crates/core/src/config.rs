//! 配置类型定义。
//!
//! 包含 Social 通知配置等跨 crate 共享的类型。

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct SocialConfig {
    #[serde(default)]
    pub notify_channel: Option<String>,
    #[serde(default)]
    pub telegram: Option<TelegramConfig>,
    #[serde(default)]
    pub discord: Option<DiscordConfig>,
    #[serde(default)]
    pub slack: Option<SlackConfig>,
    #[serde(default)]
    pub lark: Option<LarkConfig>,
    #[serde(default = "default_true")]
    pub notify_on_collect: bool,
    #[serde(default = "default_true")]
    pub notify_on_decision: bool,
    #[serde(default = "default_true")]
    pub notify_on_trade: bool,
    #[serde(default = "default_true")]
    pub notify_on_news: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TelegramConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub bot_polling: bool,
    pub bot_token: Option<String>,
    pub chat_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiscordConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub bot_token: Option<String>,
    pub channel_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SlackConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub bot_token: Option<String>,
    pub channel: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LarkConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub app_id: Option<String>,
    pub app_secret: Option<String>,
    pub receive_id: Option<String>,
}

const fn default_true() -> bool {
    true
}
