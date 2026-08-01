//! 按收件人限速的装饰器。
//!
//! 包装任意 `Channel`，对每个收件人独立限速，超出队列深度时丢弃最旧消息。

use crate::event::{AdapterError, MessageEvent};
use crate::Channel;
use async_trait::async_trait;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

struct RecipientState {
    next_allowed_at: Instant,
    queue: VecDeque<PendingReply>,
    worker_running: bool,
}

struct PendingReply {
    channel_id: String,
    text: String,
}

/// 按收件人限速的装饰器。
///
/// 对每个 `channel_id` 独立限速：两次回复之间至少间隔 `min_interval`。
/// 超出 `queue_depth` 时丢弃最旧的消息。
pub struct PacedChannel {
    inner: Arc<dyn Channel>,
    min_interval: Duration,
    queue_depth: usize,
    recipients: Arc<Mutex<HashMap<String, RecipientState>>>,
}

impl PacedChannel {
    /// 创建限速装饰器。
    ///
    /// - `min_interval`：同一收件人两次回复的最小间隔
    /// - `queue_depth`：每个收件人的最大排队数（超出丢弃最旧）
    pub fn new(inner: Arc<dyn Channel>, min_interval: Duration, queue_depth: usize) -> Self {
        Self {
            inner,
            min_interval,
            queue_depth,
            recipients: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn paced_reply(&self, channel_id: &str, text: &str) -> Result<(), AdapterError> {
        let mut recipients = self.recipients.lock().await;
        let state = recipients
            .entry(channel_id.to_string())
            .or_insert(RecipientState {
                next_allowed_at: Instant::now(),
                queue: VecDeque::new(),
                worker_running: false,
            });

        let now = Instant::now();
        if now >= state.next_allowed_at && !state.worker_running {
            state.next_allowed_at = now + self.min_interval;
            drop(recipients);
            return self.inner.reply_raw(channel_id, text).await;
        }

        // 排队
        if state.queue.len() >= self.queue_depth {
            state.queue.pop_front();
        }
        state.queue.push_back(PendingReply {
            channel_id: channel_id.to_string(),
            text: text.to_string(),
        });

        if !state.worker_running {
            state.worker_running = true;
            let inner = Arc::clone(&self.inner);
            let min_interval = self.min_interval;
            let recipients = Arc::clone(&self.recipients);
            let key = channel_id.to_string();
            tokio::spawn(async move {
                loop {
                    let next = {
                        let mut r = recipients.lock().await;
                        let s = match r.get_mut(&key) {
                            Some(s) => s,
                            None => return,
                        };
                        let pending = match s.queue.pop_front() {
                            Some(p) => p,
                            None => {
                                s.worker_running = false;
                                return;
                            }
                        };
                        let wait = s.next_allowed_at.saturating_duration_since(Instant::now());
                        s.next_allowed_at = Instant::now() + wait + min_interval;
                        (pending, wait)
                    };
                    if !next.1.is_zero() {
                        tokio::time::sleep(next.1).await;
                    }
                    let _ = inner.reply_raw(&next.0.channel_id, &next.0.text).await;
                }
            });
        }

        Ok(())
    }
}

#[async_trait]
impl Channel for PacedChannel {
    fn name(&self) -> &'static str {
        self.inner.name()
    }

    async fn listen(
        &self,
        sender: tokio::sync::mpsc::Sender<MessageEvent>,
        cancel: CancellationToken,
    ) -> Result<(), AdapterError> {
        self.inner.listen(sender, cancel).await
    }

    async fn reply(&self, event: &MessageEvent, text: &str) -> Result<(), AdapterError> {
        self.paced_reply(&event.channel_id, text).await
    }

    async fn send_text(&self, channel_id: &str, text: &str) -> Result<(), AdapterError> {
        self.paced_reply(channel_id, text).await
    }
}

/// 内部辅助：不依赖 MessageEvent 的回复
#[async_trait]
trait ReplyRaw: Channel {
    async fn reply_raw(&self, channel_id: &str, text: &str) -> Result<(), AdapterError>;
}

#[async_trait]
impl<T: Channel + ?Sized> ReplyRaw for T {
    async fn reply_raw(&self, channel_id: &str, text: &str) -> Result<(), AdapterError> {
        let ev = MessageEvent::new(
            crate::event::Platform::Telegram,
            channel_id.to_string(),
            String::new(),
            String::new(),
            String::new(),
        );
        self.reply(&ev, text).await
    }
}
