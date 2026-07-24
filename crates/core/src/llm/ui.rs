//! State persistence — stores only the active session ID.

use super::types::{DesktopState, GlobalModelSettings};
use serde_json::{json, Value};
use std::path::PathBuf;

fn agent_dir() -> PathBuf {
    pi_coding_agent::config::get_agent_dir()
}

fn get_state_path() -> Option<PathBuf> {
    let dir = agent_dir();
    let _ = std::fs::create_dir_all(&dir);
    Some(dir.join("ui-state.json"))
}

/// Persist only the active session ID.
pub fn persist_state(state: &DesktopState) {
    if let Some(path) = get_state_path() {
        let slim = json!({
            "selectedSessionId": state.selected_session_id,
        });
        if let Ok(json) = serde_json::to_string_pretty(&slim) {
            let _ = std::fs::write(&path, &json);
        }
    }
}

/// Return a minimal state skeleton with the last active session ID restored.
#[must_use] 
pub fn restore_state() -> DesktopState {
    let persisted: Option<Value> = get_state_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok());
    let mut state = default_state();
    if let Some(p) = persisted {
        if let Some(sid) = p["selectedSessionId"].as_str().filter(|x| !x.is_empty()) {
            state.selected_session_id = sid.to_string();
        }
    }
    state
}

#[must_use] 
pub const fn default_state() -> DesktopState {
    DesktopState {
        revision: 1,
        sessions: vec![],
        selected_session_id: String::new(),
        global_model_settings: GlobalModelSettings {
            enabled_model_patterns: vec![],
            default_provider: None,
            default_model_id: None,
            default_thinking_level: None,
        },
    }
}
