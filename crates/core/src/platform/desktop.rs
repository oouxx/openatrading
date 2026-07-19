// platform/desktop.rs
use async_trait::async_trait;

pub struct TauriEmitter(pub tauri::AppHandle);

#[async_trait]
impl EventEmitter for TauriEmitter {
    async fn emit_agent_event(&self, event: FrontendEvent) {
        let _ = self.0.emit("agent-event", event);
    }
    async fn emit_state_changed(&self, state: &DesktopState) {
        let _ = self.0.emit("pi-gui:state-changed", state);
    }
    async fn emit_transcript_changed(&self, session_id: &str, transcript: serde_json::Value) {
        let payload = serde_json::json!({"sessionId": session_id, "transcript": transcript});
        let _ = self.0.emit("pi-gui:selected-transcript-changed", &payload);
    }
}
