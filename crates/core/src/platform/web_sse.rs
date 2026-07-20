// platform/web_sse.rs
use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::llm::runtime::{EventEmitter, StatePersister};
use crate::llm::types::{DesktopState, FrontendEvent};

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
        transcript: Vec<serde_json::Value>,
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
    async fn emit_transcript_changed(&self, session_id: &str, transcript: Vec<serde_json::Value>) {
        let _ = self
            .tx
            .send(SseMessage::TranscriptChanged {
                session_id: session_id.to_string(),
                transcript,
            })
            .await;
    }
}
