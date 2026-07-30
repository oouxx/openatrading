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
    /// Auto-unsubscribes the session event listener when dropped.
    ///
    /// pi-rs' [`SessionEventUnsubscribeHandle`] has no `Drop` impl of its own,
    /// so dropping it silently leaves the listener subscribed on the
    /// [`AgentSession`]. On every WebSocket reconnect a *new* listener would
    /// accumulate while the old one kept emitting into a dead channel — every
    /// event was emitted N times into the ring buffer, so the next
    /// reconnect-replay delivered each event N times, producing duplicated /
    /// garbled content in the live transcript (refresh was clean because the
    /// REST history path reads from disk, not the ring). Unsubscribing here on
    /// disconnect guarantees exactly one live listener per session at a time.
    pub subscription_handle: UnsubscribeOnDrop,
}

/// RAII wrapper that calls `unsubscribe()` on drop — see
/// [`SessionWithEvents::subscription_handle`] for the rationale.
pub struct UnsubscribeOnDrop(Option<SessionEventUnsubscribeHandle>);

impl UnsubscribeOnDrop {
    pub fn new(handle: SessionEventUnsubscribeHandle) -> Self {
        Self(Some(handle))
    }
}

impl Drop for UnsubscribeOnDrop {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            handle.unsubscribe();
        }
    }
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
        subscription_handle: UnsubscribeOnDrop::new(_subscription_handle),
    })
}
