use crate::circuit_breaker::CircuitBreaker;
use crate::event::{AdapterError, MessageEvent};
use crate::session::SessionManager;
use async_trait::async_trait;
use tokio::sync::mpsc;

/// 平台适配器——统一多平台消息接入的抽象层。
///
/// 每个平台（Telegram／Discord／Slack 等）实现此 trait，
/// 通过 `start()` 启动监听，将收到的消息转为 `MessageEvent`
/// 并通过 `sender` 送入 Agent 执行链路。
#[async_trait]
pub trait MessageAdapter: Send + Sync {
    /// 平台名称（用于日志／监控）。
    fn platform_name(&self) -> &'static str;

    /// 启动监听，通过 `sender` 将收到的统一事件送入上层。
    async fn start(self: Box<Self>, sender: mpsc::Sender<MessageEvent>)
        -> Result<(), AdapterError>;

    /// 优雅关闭。
    async fn stop(&self) -> Result<(), AdapterError>;

    /// 回复消息（平台相关）。
    async fn reply(&self, event: &MessageEvent, text: &str) -> Result<(), AdapterError>;
}

/// 适配器运行时——负责管理适配器生命周期、会话、断路保护。
///
/// 这是 `BaseAdapter` 的 Rust 等价物：
/// - 持有 `SessionManager` 统一管理所有平台会话
/// - 持有 `CircuitBreaker` 保护每个平台不被打垮
/// - 通过 `on_message` 统一入口将事件送入上游
#[derive(Clone)]
pub struct AdapterRuntime {
    /// 会话守卫
    pub sessions: SessionManager,
    /// 断路保护
    pub circuit_breaker: CircuitBreaker,
}

impl AdapterRuntime {
    /// 创建带默认配置的新运行时。
    pub fn new() -> Self {
        Self {
            sessions: SessionManager::new(),
            circuit_breaker: CircuitBreaker::default(),
        }
    }

    /// 创建带自定义断路器的新运行时。
    pub fn with_circuit_breaker(cb: CircuitBreaker) -> Self {
        Self {
            sessions: SessionManager::new(),
            circuit_breaker: cb,
        }
    }

    /// 统一消息入口——调用前应已通过 `Sender` 收到 `MessageEvent`。
    ///
    /// 此方法：
    /// 1. 更新/创建会话
    /// 2. 检查断路器状态
    /// 3. 返回 `MessageEvent` 供后续处理
    pub async fn on_message(&self, event: MessageEvent) -> Result<MessageEvent, AdapterError> {
        // 1. 检查断路器
        if !self.circuit_breaker.is_allowed() {
            return Err(AdapterError::CircuitOpen);
        }

        // 2. 更新会话
        self.sessions
            .upsert(
                event.platform,
                event.user_id.clone(),
                event.channel_id.clone(),
                event.user_name.clone(),
            )
            .await;

        // 3. 记录成功（断路器计数重置）
        self.circuit_breaker.record_success();

        Ok(event)
    }

    /// 标记处理成功（重置断路器失败计数）。
    pub fn record_success(&self) {
        self.circuit_breaker.record_success();
    }

    /// 标记处理失败（递增断路器失败计数）。
    pub fn record_failure(&self) {
        self.circuit_breaker.record_failure();
    }
}

impl Default for AdapterRuntime {
    fn default() -> Self {
        Self::new()
    }
}

/// 便捷函数：从多个适配器创建统一的消息流。
///
/// 启动所有适配器，每个适配器的输出都路由到同一个 `Receiver`。
pub fn launch_adapters(
    adapters: Vec<Box<dyn MessageAdapter>>,
    buffer: usize,
) -> (
    mpsc::Receiver<MessageEvent>,
    Vec<tokio::task::JoinHandle<()>>,
) {
    let (tx, rx) = mpsc::channel(buffer);
    let mut handles = Vec::with_capacity(adapters.len());

    for adapter in adapters {
        let tx = tx.clone();
        handles.push(tokio::spawn(async move {
            if let Err(e) = adapter.start(tx).await {
                tracing::error!("[adapter] failed to start: {e}");
            }
        }));
    }

    (rx, handles)
}
