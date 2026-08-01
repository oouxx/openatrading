//! 编译时通道清单。
//!
//! 与 zeroclaw 的 `listing.rs` 对齐，提供 `compiled_channels()` 和
//! `is_compiled()` 供 CLI 和配置校验使用。

/// 通道编译时信息。
#[derive(Debug, Clone)]
pub struct ChannelEntry {
    /// 显示名称
    pub name: &'static str,
    /// 配置键名
    pub config_key: &'static str,
    /// 当前二进制是否编译了此通道
    pub compiled: bool,
}

/// 返回当前二进制中所有已知通道的编译状态。
pub fn compiled_channels() -> Vec<ChannelEntry> {
    vec![
        ChannelEntry {
            name: "Telegram",
            config_key: "telegram",
            compiled: cfg!(feature = "channel-telegram"),
        },
        ChannelEntry {
            name: "Discord",
            config_key: "discord",
            compiled: cfg!(feature = "channel-discord"),
        },
        ChannelEntry {
            name: "Slack",
            config_key: "slack",
            compiled: cfg!(feature = "channel-slack"),
        },
        ChannelEntry {
            name: "Lark",
            config_key: "lark",
            compiled: cfg!(feature = "channel-lark"),
        },
    ]
}

/// 检查指定通道是否已编译。
pub fn is_compiled(config_key: &str) -> bool {
    compiled_channels()
        .iter()
        .find(|e| e.config_key == config_key)
        .is_some_and(|e| e.compiled)
}
