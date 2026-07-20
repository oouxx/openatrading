// platform/cli.rs
use async_trait::async_trait;
use std::io::Write;

use crate::llm::runtime::{EventEmitter, StatePersister};
use crate::llm::types::{DesktopState, FrontendEvent};

pub struct StdoutEmitter {
    verbose: bool,
}

impl StdoutEmitter {
    pub fn new(verbose: bool) -> Self {
        Self { verbose }
    }
}

#[async_trait]
impl EventEmitter for StdoutEmitter {
    async fn emit_agent_event(&self, event: FrontendEvent) {
        match event.event_type.as_str() {
            "message_update" => {
                if let Some(delta) = event.data.get("delta").and_then(|d| d.as_str()) {
                    print!("{delta}");
                    let _ = std::io::stdout().flush();
                }
            },
            "tool_execution_start" if self.verbose => {
                let name = event
                    .data
                    .get("tool_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                eprintln!("\n[tool] {name} ...");
            },
            "tool_execution_end" if self.verbose => {
                let name = event
                    .data
                    .get("tool_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                let is_error = event
                    .data
                    .get("is_error")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                eprintln!("[tool] {name} {}", if is_error { "failed" } else { "done" });
            },
            "message_end" => println!(),
            _ => {},
        }
    }

    async fn emit_state_changed(&self, _state: &DesktopState) {
        // CLI 无侧边栏/UI 状态需要同步，忽略。
    }

    async fn emit_transcript_changed(
        &self,
        _session_id: &str,
        _transcript: Vec<serde_json::Value>,
    ) {
        // 同上。
    }
}

pub struct NoopPersister;

#[async_trait]
impl StatePersister for NoopPersister {
    async fn persist(&self, _state: &DesktopState) {}
}
