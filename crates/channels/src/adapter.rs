#![allow(clippy::type_complexity)]
//! 通道适配器运行时——管理适配器生命周期、消息流合并。

use crate::event::MessageEvent;
use crate::Channel;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// 便捷函数：从多个 Channel 创建统一的消息流。
///
/// 启动所有通道的监听，每个通道的输出都路由到同一个 `Receiver`。
/// 返回 `(接收端, 通道列表, JoinHandle 列表, 取消令牌)`。
pub fn launch_channels(channels: Vec<Arc<dyn Channel>>, buffer: usize) -> ChannelStream {
    let (tx, rx) = mpsc::channel(buffer);
    let cancel = CancellationToken::new();
    let mut handles = Vec::with_capacity(channels.len());

    for ch in &channels {
        let tx = tx.clone();
        let c = Arc::clone(ch);
        let token = cancel.child_token();
        handles.push(tokio::spawn(async move {
            if let Err(e) = c.listen(tx, token).await {
                tracing::error!("[channel] {} listen failed: {e}", c.name());
            }
        }));
    }

    (rx, channels, handles, cancel)
}

/// `launch_channels` 的返回类型。
pub type ChannelStream = (
    mpsc::Receiver<MessageEvent>,
    Vec<Arc<dyn Channel>>,
    Vec<tokio::task::JoinHandle<()>>,
    CancellationToken,
);

/// 向后兼容的 `launch_adapters`。
#[deprecated(note = "use launch_channels instead")]
pub fn launch_adapters(
    adapters: Vec<Arc<dyn Channel>>,
    buffer: usize,
) -> (
    mpsc::Receiver<MessageEvent>,
    Vec<Arc<dyn Channel>>,
    Vec<tokio::task::JoinHandle<()>>,
) {
    let (rx, channels, handles, _cancel) = launch_channels(adapters, buffer);
    (rx, channels, handles)
}

/// 向后兼容的 `MessageAdapter` trait 别名。
#[deprecated(note = "use Channel trait directly")]
pub trait MessageAdapter: Channel {}
#[allow(deprecated)]
impl<T: Channel + ?Sized> MessageAdapter for T {}
