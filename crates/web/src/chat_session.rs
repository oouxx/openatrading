//! Shared chat-session logic for the WebSocket (web) and Tauri IPC (desktop)
//! backends.
//!
//! Centralizes session lifecycle (`ensure_session_with_events`) so the
//! WebSocket handler and the Tauri IPC layer share one source of truth.

use std::sync::Arc;

use pi_coding_agent::core::agent_session::SessionEventUnsubscribeHandle;
use pi_coding_agent::core::agent_session_runtime::AgentSessionRuntime;
use tokio::sync::{mpsc, Mutex};

use crate::agent_ws::{setup_event_channel, SessionEventState};

/// Result of ensuring a session with events.
pub struct SessionWithEvents {
    pub runtime: Arc<Mutex<AgentSessionRuntime>>,
    pub tx: mpsc::Sender<String>,
    pub rx: mpsc::Receiver<String>,
    pub _subscription_handle: SessionEventUnsubscribeHandle,
}

/// Ensure a session exists (resume from disk or create new) and set up its
/// event channel.
///
/// Returns the runtime reference and the event receiver. This is the shared
/// session lifecycle logic used by both the WebSocket handler and the Tauri
/// IPC layer.
pub async fn ensure_session_with_events(
    pool: &openatrading_core::llm::SessionPool,
    session_id: &str,
    event_state: &Arc<SessionEventState>,
) -> Result<SessionWithEvents, String> {
    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let runtime = match pool.get_or_resume(session_id).await {
        Ok(rt) => rt,
        Err(_) => {
            pool.create_session_with_id(&cwd, Some(session_id))
                .await
                .map_err(|e| format!("failed to create session: {e}"))?;
            pool.get_session(session_id)
                .await
                .ok_or_else(|| "session created but not found in pool".to_string())?
        }
    };

    let (tx, rx, _subscription_handle) = {
        let rt = runtime.lock().await;
        setup_event_channel(rt.session(), event_state).await
    };

    Ok(SessionWithEvents {
        runtime,
        tx,
        rx,
        _subscription_handle,
    })
}
