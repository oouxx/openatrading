// platform/tui.rs
// TUI platform — no emitter or persister needed.
// Events are handled directly by the TUI event loop.

use tokio::sync::mpsc;

use crate::llm::types::{DesktopState, FrontendEvent};

/// TUI 主循环消费的统一事件类型，把 agent 事件和其它 UI 事件包在一起，
/// 这样 select! 里可以用一个 channel 而不是好几个。
#[derive(Debug, Clone)]
pub enum TuiEvent {
    Agent(FrontendEvent),
    StateChanged(DesktopState),
    TranscriptChanged {
        session_id: String,
        transcript: Vec<serde_json::Value>,
    },
}

/// 发送 TUI 事件的 channel。
pub type TuiEventSender = mpsc::UnboundedSender<TuiEvent>;
