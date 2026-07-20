// platform/desktop.rs
use async_trait::async_trait;

use tauri::Emitter;

use crate::llm::runtime::{EventEmitter, StatePersister};
use crate::llm::types::{DesktopState, FrontendEvent};

pub struct TauriEmitter(pub tauri::AppHandle);

#[async_trait]
impl EventEmitter for TauriEmitter {
    async fn emit_agent_event(&self, event: FrontendEvent) {
        let _ = self.0.emit("agent-event", event);
    }
    async fn emit_state_changed(&self, state: &DesktopState) {
        let _ = self.0.emit("pi-gui:state-changed", state);
    }
    async fn emit_transcript_changed(&self, session_id: &str, transcript: Vec<serde_json::Value>) {
        let payload = serde_json::json!({"sessionId": session_id, "transcript": transcript});
        let _ = self.0.emit("pi-gui:selected-transcript-changed", &payload);
    }
}

/// Desktop file-based persister (used by Tauri).
pub struct TauriFilePersister;

#[async_trait]
impl StatePersister for TauriFilePersister {
    async fn persist(&self, state: &DesktopState) {
        crate::llm::ui::persist_state(state);
    }
}
