// platform/web_sse.rs
use async_trait::async_trait;
use tokio::sync::mpsc;

/// SSE 场景下，事件先送进一个统一的 mpsc channel，
/// 再由 handler 端把 channel 转成 axum 的 SSE Stream。
/// 和 emit_agent_event/emit_state_changed/emit_transcript_changed 三种事件
/// 统一编码成 { "type": ..., "payload": ... }，方便前端用一个 EventSource
/// onmessage 里 switch(type) 处理，而不需要用 SSE 的具名 event 字段区分三种。
#[derive(serde::Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
enum SseMessage {
    AgentEvent(FrontendEvent),
    StateChanged(DesktopState),
    TranscriptChanged {
        session_id: String,
        transcript: serde_json::Value,
    },
}

pub struct SseEmitter {
    tx: mpsc::Sender<SseMessage>,
}

#[async_trait]
impl EventEmitter for SseEmitter {
    async fn emit_agent_event(&self, event: FrontendEvent) {
        let _ = self.tx.send(SseMessage::AgentEvent(event)).await;
    }
    async fn emit_state_changed(&self, state: &DesktopState) {
        let _ = self.tx.send(SseMessage::StateChanged(state.clone())).await;
    }
    async fn emit_transcript_changed(&self, session_id: &str, transcript: serde_json::Value) {
        let _ = self
            .tx
            .send(SseMessage::TranscriptChanged {
                session_id: session_id.to_string(),
                transcript,
            })
            .await;
    }
}
use arc_swap::ArcSwap;

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
    async fn emit_transcript_changed(&self, session_id: &str, transcript: serde_json::Value) {
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
