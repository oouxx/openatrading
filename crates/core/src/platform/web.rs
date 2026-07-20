// platform/web.rs
use arc_swap::ArcSwap;
use async_trait::async_trait;
use axum::extract::ws::Message;
use tokio::sync::mpsc;

use crate::context::AppContext;
use crate::llm::runtime::{EventEmitter, StatePersister};
use crate::llm::types::{DesktopState, FrontendEvent};
use crate::memory::sqlite::MemoryStore;
use crate::trade::sqlite::TradeManager;

pub struct WsEmitter {
    tx: mpsc::Sender<Message>,
}

#[async_trait]
impl EventEmitter for WsEmitter {
    async fn emit_agent_event(&self, event: FrontendEvent) {
        if let Ok(text) =
            serde_json::to_string(&serde_json::json!({"type": "agent_event", "payload": event}))
        {
            let _ = self.tx.send(Message::Text(text)).await;
        }
    }
    async fn emit_state_changed(&self, state: &DesktopState) {
        if let Ok(text) =
            serde_json::to_string(&serde_json::json!({"type": "state_changed", "payload": state}))
        {
            let _ = self.tx.send(Message::Text(text)).await;
        }
    }
    async fn emit_transcript_changed(&self, session_id: &str, transcript: Vec<serde_json::Value>) {
        let payload = serde_json::json!({
            "type": "transcript_changed",
            "sessionId": session_id,
            "transcript": transcript,
        });
        let _ = self.tx.send(Message::Text(payload.to_string())).await;
    }
}

/// 可热替换的 emitter 包装，供断线重连时替换底层 WsEmitter 而不重建
/// 整个 AppContext / AgentRuntimeCore（内存里跑着的 session 不受影响）。
pub struct SwappableEmitter(ArcSwap<dyn EventEmitter>);

impl SwappableEmitter {
    pub fn new(initial: Arc<dyn EventEmitter>) -> Self {
        Self(ArcSwap::from(initial))
    }
    pub fn swap(&self, new_emitter: Arc<dyn EventEmitter>) {
        self.0.store(new_emitter);
    }
}

#[async_trait]
impl EventEmitter for SwappableEmitter {
    async fn emit_agent_event(&self, event: FrontendEvent) {
        self.0.load().emit_agent_event(event).await;
    }
    async fn emit_state_changed(&self, state: &DesktopState) {
        self.0.load().emit_state_changed(state).await;
    }
    async fn emit_transcript_changed(&self, session_id: &str, transcript: Vec<serde_json::Value>) {
        self.0
            .load()
            .emit_transcript_changed(session_id, transcript)
            .await;
    }
}

pub struct DbPersister {
    pool: sqlx::PgPool,
    user_id: String,
}

impl DbPersister {
    pub fn new(pool: sqlx::PgPool, user_id: String) -> Self {
        Self { pool, user_id }
    }
}

#[async_trait]
impl StatePersister for DbPersister {
    async fn persist(&self, state: &DesktopState) {
        let data = serde_json::to_value(state).unwrap_or_default();
        let _ = sqlx::query!(
            "insert into agent_state (user_id, data, updated_at) values ($1, $2, now())
             on conflict (user_id) do update set data = $2, updated_at = now()",
            self.user_id,
            data,
        )
        .execute(&self.pool)
        .await;
    }
}

pub struct WebPushNotifier {
    user_id: String,
}

impl WebPushNotifier {
    pub fn new(user_id: &str) -> Self {
        Self {
            user_id: user_id.to_string(),
        }
    }
}

#[async_trait]
impl crate::social::NotificationSender for WebPushNotifier {
    async fn send_text(&self, text: &str) -> anyhow::Result<()> {
        // Web push notification — placeholder for future implementation
        tracing::info!("[web-push] send to {}: {}", self.user_id, text);
        Ok(())
    }

    async fn notify_collect(&self, text: &str) -> anyhow::Result<()> {
        self.send_text(text).await
    }

    async fn notify_decision(&self, text: &str) -> anyhow::Result<()> {
        self.send_text(text).await
    }

    async fn notify_trade(&self, text: &str) -> anyhow::Result<()> {
        self.send_text(text).await
    }

    async fn notify_news(&self, text: &str) -> anyhow::Result<()> {
        self.send_text(text).await
    }
}

pub struct RuntimeRegistry {
    pool: sqlx::PgPool,
    contexts: dashmap::DashMap<String, (Arc<AppContext>, Arc<SwappableEmitter>)>,
}

impl RuntimeRegistry {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self {
            pool,
            contexts: dashmap::DashMap::new(),
        }
    }

    pub async fn get_or_create(
        &self,
        user_id: &str,
        ws_tx: mpsc::Sender<Message>,
    ) -> Arc<AppContext> {
        let ws_emitter: Arc<dyn EventEmitter> = Arc::new(WsEmitter { tx: ws_tx });

        if let Some(entry) = self.contexts.get(user_id) {
            // 已有 AppContext（可能是重连）：只替换底层 emitter，
            // session map 里在跑的 agent loop 完全不受影响。
            entry.1.swap(ws_emitter);
            return entry.0.clone();
        }

        let swappable = Arc::new(SwappableEmitter::new(ws_emitter));
        let persister = Arc::new(DbPersister::new(self.pool.clone(), user_id.to_string()));

        let ctx = Arc::new(
            AppContext::build(
                None,
                None,
                &[],
                Arc::new(MemoryStore::for_user(&self.pool, user_id).await),
                Arc::new(TradeManager::for_user(&self.pool, user_id)),
                Arc::new(WebPushNotifier::new(user_id)),
                swappable.clone() as Arc<dyn EventEmitter>,
                persister,
            )
            .await,
        );

        self.contexts
            .insert(user_id.to_string(), (ctx.clone(), swappable));
        ctx
    }

    /// 连接彻底关闭且不会重连时才调用（比如用户主动登出），
    /// 一般断线不必调用这个，让 AppContext 留在内存等重连。
    pub fn evict_user(&self, user_id: &str) {
        self.contexts.remove(user_id);
    }
}
