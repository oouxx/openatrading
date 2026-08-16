//! Shared chat-session logic for the WebSocket (web) and Tauri IPC (desktop)
//! backends.
//!
//! Centralizes session lifecycle (`ensure_session_with_events`) so the
//! WebSocket handler and the Tauri IPC layer share one source of truth.
//!
//! With the actor architecture the pool never hands out the raw
//! `AgentSessionRuntime` — callers get a [`SessionHandle`] (mailbox) and a
//! subscribed event channel. The actor owns the pi-rs event subscription for
//! the whole session lifetime and fans events out to every live subscriber
//! channel; a dropped receiver (connection closed) is pruned lazily by the
//! fan-out task, so no per-connection unsubscribe bookkeeping is needed.

use openatrading_core::llm::SessionHandle;
use tokio::sync::mpsc;

/// Result of ensuring a session with events.
pub struct SessionWithEvents {
    pub handle: SessionHandle,
    /// Multi-producer sender into this connection's event stream — the actor's
    /// fan-out task holds a clone for live events; the caller can send local
    /// notices (slash results, errors) on its own clone.
    pub tx: mpsc::Sender<String>,
    pub rx: mpsc::Receiver<String>,
}

/// Ensure a session exists (resume from disk or create new) and subscribe its
/// event channel.
///
/// Returns the session handle and the event receiver. This is the shared
/// session lifecycle logic used by both the WebSocket handler and the Tauri
/// IPC layer.
pub async fn ensure_session_with_events(
    pool: &openatrading_core::llm::SessionPool,
    session_id: &str,
) -> Result<SessionWithEvents, String> {
    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let handle = match pool.get_or_resume(session_id).await {
        Ok(h) => h,
        Err(_) => {
            pool.create_session_with_id(&cwd, Some(session_id))
                .await
                .map_err(|e| format!("failed to create session: {e}"))?;
            pool.get_session(session_id)
                .await
                .ok_or_else(|| "session created but not found in pool".to_string())?
        }
    };

    let (tx, rx) = handle.subscribe().await;

    Ok(SessionWithEvents { handle, tx, rx })
}
