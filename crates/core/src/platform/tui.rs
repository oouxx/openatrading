// platform/tui.rs
use async_trait::async_trait;
use tokio::sync::mpsc;

/// TUI 主循环消费的统一事件类型，把 agent 事件和其它 UI 事件包在一起，
/// 这样 select! 里可以用一个 channel 而不是好几个。
#[derive(Debug, Clone)]
pub enum TuiEvent {
    Agent(FrontendEvent),
    StateChanged(DesktopState),
    TranscriptChanged {
        session_id: String,
        transcript: serde_json::Value,
    },
}

pub struct TuiEmitter {
    tx: mpsc::UnboundedSender<TuiEvent>,
}

impl TuiEmitter {
    pub fn new(tx: mpsc::UnboundedSender<TuiEvent>) -> Self {
        Self { tx }
    }
}

#[async_trait]
impl EventEmitter for TuiEmitter {
    async fn emit_agent_event(&self, event: FrontendEvent) {
        // unbounded + 忽略发送失败：接收端（渲染循环）退出时说明程序在关闭，
        // 此时事件丢了无所谓。
        let _ = self.tx.send(TuiEvent::Agent(event));
    }

    async fn emit_state_changed(&self, state: &DesktopState) {
        let _ = self.tx.send(TuiEvent::StateChanged(state.clone()));
    }

    async fn emit_transcript_changed(&self, session_id: &str, transcript: serde_json::Value) {
        let _ = self.tx.send(TuiEvent::TranscriptChanged {
            session_id: session_id.to_string(),
            transcript,
        });
    }
}

pub struct LocalFilePersister {
    path: std::path::PathBuf,
}

impl LocalFilePersister {
    pub fn new(app_name: &str) -> Self {
        let path = dirs::home_dir()
            .unwrap_or_default()
            .join(format!(".{app_name}/state.json"));
        Self { path }
    }
}

#[async_trait]
impl StatePersister for LocalFilePersister {
    async fn persist(&self, state: &DesktopState) {
        if let Some(parent) = self.path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        if let Ok(json) = serde_json::to_vec_pretty(state) {
            let _ = tokio::fs::write(&self.path, json).await;
        }
    }
}
