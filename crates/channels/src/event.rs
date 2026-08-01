use serde::{Deserialize, Serialize};
use std::fmt;

/// 支持的平台。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Platform {
    Telegram,
    Discord,
    Slack,
    Lark,
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Telegram => write!(f, "telegram"),
            Self::Discord => write!(f, "discord"),
            Self::Slack => write!(f, "slack"),
            Self::Lark => write!(f, "lark"),
        }
    }
}

/// 统一消息事件——所有平台输入都转换为这个结构。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEvent {
    /// 来源平台
    pub platform: Platform,
    /// 平台内的频道 / 群组 / 会话 ID
    pub channel_id: String,
    /// 发送者用户 ID
    pub user_id: String,
    /// 发送者显示名（可选）
    pub user_name: Option<String>,
    /// 消息文本
    pub text: String,
    /// 平台侧的消息 ID（用于回复定位）
    pub message_id: String,
    /// 原始完整 payload，供需要做平台特定操作的 adapter 参考
    pub raw: serde_json::Value,
}

impl MessageEvent {
    pub const fn new(
        platform: Platform,
        channel_id: String,
        user_id: String,
        text: String,
        message_id: String,
    ) -> Self {
        Self {
            platform,
            channel_id,
            user_id,
            user_name: None,
            text,
            message_id,
            raw: serde_json::Value::Null,
        }
    }

    pub fn with_user_name(mut self, name: String) -> Self {
        self.user_name = Some(name);
        self
    }

    pub fn with_raw(mut self, raw: serde_json::Value) -> Self {
        self.raw = raw;
        self
    }
}

/// 平台适配器错误。
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("平台未连接: {0}")]
    NotConnected(String),
    #[error("发送失败: {0}")]
    SendFailed(String),
    #[error("断路器已打开")]
    CircuitOpen,
    #[error("会话未找到: {0}")]
    SessionNotFound(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
