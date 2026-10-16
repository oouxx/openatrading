//! 多平台消息通道抽象。
//!
//! # 架构
//!
//! - [`Channel`] trait：统一双向通信（发送 + 监听 + 回复 + 停止）
//! - [`NotificationSender`] trait：单向推送（`Channel` 的子集，用于系统通知）
//! - 装饰器：`CircuitBreakerChannel` / `PacedChannel` 叠加横切关注点
//! - 各平台实现通过 feature gate 控制编译

use anyhow::Result;
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

pub mod adapter;
pub mod circuit_breaker;
pub mod event;
pub mod listing;
pub mod paced_channel;
pub mod session;

// Feature-gated platform implementations
#[cfg(feature = "channel-discord")]
pub mod discord;
#[cfg(feature = "channel-lark")]
pub mod lark;
#[cfg(feature = "channel-slack")]
pub mod slack;
#[cfg(feature = "channel-telegram")]
pub mod telegram;

// ── Channel trait ───────────────────────────────────────────────

/// 统一消息通道：发送、监听、回复、停止。
///
/// 一个 `Channel` 实现同时处理**出站**（`send` / `reply`）和**入站**
/// （`listen`），与 zeroclaw 的 `Channel` trait 对齐。
/// 装饰器（`CircuitBreakerChannel`、`PacedChannel`）可透明叠加。
#[async_trait]
pub trait Channel: Send + Sync {
    /// 平台名称（用于日志／监控）。
    fn name(&self) -> &'static str;

    /// 启动监听，通过 `sender` 将收到的统一事件送入上层。
    async fn listen(
        &self,
        sender: tokio::sync::mpsc::Sender<event::MessageEvent>,
        cancel: CancellationToken,
    ) -> Result<(), event::AdapterError>;

    /// 回复消息（平台相关）。
    async fn reply(
        &self,
        event: &event::MessageEvent,
        text: &str,
    ) -> Result<(), event::AdapterError>;

    /// 发送文本到指定频道（无关联事件，用于系统推送）。
    async fn send_text(&self, channel_id: &str, text: &str) -> Result<(), event::AdapterError> {
        // 默认实现：用 reply 语义，子类可覆盖
        let ev = event::MessageEvent::new(
            event::Platform::Telegram,
            channel_id.to_string(),
            String::new(),
            String::new(),
            String::new(),
        );
        self.reply(&ev, text).await
    }
}

// ── NotificationSender（单向推送，保持向后兼容） ─────────────

/// 单向通知发送（系统→用户）。
///
/// 与 `Channel` 分离，供只需要推送、不需要监听的场景使用。
/// 各平台实现同时 impl `Channel` 和 `NotificationSender`。
#[async_trait]
pub trait NotificationSender: Send + Sync {
    async fn send_text(&self, text: &str) -> Result<()>;

    async fn notify_collect(&self, text: &str) -> Result<()> {
        self.send_text(text).await
    }
    async fn notify_news(&self, text: &str) -> Result<()> {
        self.send_text(text).await
    }
}

// ── 内置通道 ──────────────────────────────────────────────────

/// 空操作通道（丢弃所有消息）。
pub struct NoopChannel;

#[async_trait]
impl NotificationSender for NoopChannel {
    async fn send_text(&self, _: &str) -> Result<()> {
        Ok(())
    }
}

/// 标准输出通道（打印到 stdout）。
pub struct StdoutChannel;

#[async_trait]
impl NotificationSender for StdoutChannel {
    async fn send_text(&self, text: &str) -> Result<()> {
        println!("{text}");
        println!("{}", "─".repeat(40));
        Ok(())
    }
}
