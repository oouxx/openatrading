//! WebSocket transport for agent session events.
//!
//! Both `AgentEvent` (async) and `AgentSessionEvent` (sync) are unified into a
//! single [`TranscriptEvent`] enum and sent over a WebSocket connection. This
//! module is shared between the axum web server and the Tauri IPC layer — the
//! event-production logic (`setup_event_channel`) is identical, only the
//! delivery mechanism differs (`socket.send` vs `app.emit`).

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use openatrading_core::llm::{AgentFacade, AgentMessage};
use pi_coding_agent::core::agent_session::{AgentSessionEvent, AgentSessionEventListener};
use pi_coding_agent::pi_agent_core::types::AgentEvent;
use serde::Serialize;
use tokio::sync::mpsc;

use crate::chat::handle_slash_command;
use crate::AppState;

/// Wire-format event: either a core `AgentEvent` or a session-level event.
///
/// `AgentEvent` already derives `Serialize` (with `#[serde(tag = "type")]`),
/// so it is embedded directly. `AgentSessionEvent` does **not** derive
/// `Serialize`, so we convert the variants we care about into
/// [`SessionEventPayload`] before wrapping.
#[derive(Debug, Serialize)]
#[serde(tag = "kind")]
pub enum TranscriptEvent {
    /// Core agent event (text deltas, tool calls, thinking, etc.)
    #[serde(rename = "core")]
    Core(Box<AgentEvent>),
    /// Session-level event (compaction, retry, turn lifecycle, settled)
    #[serde(rename = "session")]
    Session(SessionEventPayload),
}

/// Serializable subset of [`AgentSessionEvent`].
///
/// Only the variants relevant to the frontend are included; unknown/future
/// variants fall through to [`SessionEventPayload::Other`].
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEventPayload {
    TurnStart,
    TurnEnd {
        tool_results: Vec<String>,
    },
    AgentSettled,
    AgentEnd {
        will_retry: bool,
    },
    CompactionStart {
        reason: String,
    },
    CompactionEnd {
        reason: String,
        aborted: bool,
        will_retry: bool,
        error_message: Option<String>,
    },
    AutoRetryStart {
        attempt: u32,
        max_attempts: u32,
        delay_ms: u64,
        error_message: String,
    },
    AutoRetryEnd {
        success: bool,
        attempt: u32,
        final_error: Option<String>,
    },
    QueueUpdate {
        steering: Vec<String>,
        follow_up: Vec<String>,
    },
    SessionInfoChanged {
        name: Option<String>,
    },
    /// Slash command result / error rendered as a system notice block.
    Notice {
        text: String,
        is_error: bool,
    },
    /// Catch-all for variants not explicitly mapped.
    Other,
}

/// Convert an [`AgentSessionEvent`] into the serializable payload.
pub fn convert_session_event(event: AgentSessionEvent) -> SessionEventPayload {
    match event {
        AgentSessionEvent::TurnStart => SessionEventPayload::TurnStart,
        AgentSessionEvent::TurnEnd { tool_results, .. } => SessionEventPayload::TurnEnd {
            tool_results: extract_tool_result_texts(&tool_results),
        },
        AgentSessionEvent::AgentSettled => SessionEventPayload::AgentSettled,
        AgentSessionEvent::AgentEnd { will_retry, .. } => {
            SessionEventPayload::AgentEnd { will_retry }
        }
        AgentSessionEvent::CompactionStart { reason } => SessionEventPayload::CompactionStart {
            reason: format!("{reason:?}"),
        },
        AgentSessionEvent::CompactionEnd {
            reason,
            aborted,
            will_retry,
            error_message,
            ..
        } => SessionEventPayload::CompactionEnd {
            reason: format!("{reason:?}"),
            aborted,
            will_retry,
            error_message,
        },
        AgentSessionEvent::AutoRetryStart {
            attempt,
            max_attempts,
            delay_ms,
            error_message,
        } => SessionEventPayload::AutoRetryStart {
            attempt,
            max_attempts,
            delay_ms,
            error_message,
        },
        AgentSessionEvent::AutoRetryEnd {
            success,
            attempt,
            final_error,
        } => SessionEventPayload::AutoRetryEnd {
            success,
            attempt,
            final_error,
        },
        AgentSessionEvent::QueueUpdate {
            steering,
            follow_up,
        } => SessionEventPayload::QueueUpdate {
            steering,
            follow_up,
        },
        AgentSessionEvent::SessionInfoChanged { name } => {
            SessionEventPayload::SessionInfoChanged { name }
        }
        // Ignore unused variants for now; they map to Other.
        _ => SessionEventPayload::Other,
    }
}

/// Extract human-readable text from a slice of tool-result messages.
fn extract_tool_result_texts(messages: &[AgentMessage]) -> Vec<String> {
    use openatrading_core::llm::pi_ai_types::ContentBlock;
    messages
        .iter()
        .filter_map(|m| match m {
            AgentMessage::ToolResult { content, .. } => {
                let text: String = content
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text, .. } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                if text.is_empty() {
                    None
                } else {
                    Some(text)
                }
            }
            _ => None,
        })
        .collect()
}

/// Client → server commands received over the WebSocket.
#[derive(serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientCommand {
    Prompt { text: String },
    Interrupt,
}

/// Async listener signature for `AgentEvent` subscription.
type AgentEventListener = Arc<
    dyn Fn(
            AgentEvent,
            Option<tokio::sync::watch::Receiver<bool>>,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
        + Send
        + Sync,
>;

/// Set up event subscriptions on an agent facade, returning the receiver end
/// of a single unbounded channel that merges both event streams.
///
/// This function is the **single source of truth** for event production —
/// the axum WS handler and the Tauri IPC layer both call it, then decide
/// independently how to consume `evt_rx`.
pub async fn setup_event_channel(
    agent_facade: &AgentFacade,
) -> mpsc::UnboundedReceiver<TranscriptEvent> {
    let (evt_tx, evt_rx) = mpsc::unbounded_channel::<TranscriptEvent>();

    // AgentEvent (async) → Core
    let tx1 = evt_tx.clone();
    let async_listener: AgentEventListener = Arc::new(move |event, _signal| {
        let tx = tx1.clone();
        Box::pin(async move {
            let _ = tx.send(TranscriptEvent::Core(Box::new(event)));
        })
    });
    let _h1 = agent_facade.session.subscribe(async_listener).await;

    // AgentSessionEvent (sync) → Session
    let tx2 = evt_tx.clone();
    let sync_listener: AgentSessionEventListener = Arc::new(move |event: AgentSessionEvent| {
        let payload = convert_session_event(event);
        let _ = tx2.send(TranscriptEvent::Session(payload));
    });
    let _h2 = agent_facade.session.subscribe_session_events(sync_listener);

    evt_rx
}

/// `GET /api/ai/sessions/:id/ws` — WebSocket upgrade.
///
/// The `:id` path parameter is the session ID. On connect the server resumes
/// (or creates) the agent session, sets up event subscriptions, and enters a
/// select loop that forwards events to the client and receives commands.
pub async fn ws_handler(
    Path(session_id): Path<String>,
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_session(socket, state, session_id))
}

async fn handle_session(mut socket: WebSocket, state: Arc<AppState>, session_id: String) {
    use openatrading_core::log;

    // Resume (or create) the agent session.
    let mut agent_facade = match state
        .ctx
        .runtime
        .create_session(None, None, Some(true), Some(&session_id), None)
        .await
    {
        Ok(h) => h,
        Err(e) => {
            log::error!("ws: failed to create/resume session {session_id}: {e}");
            let _ = socket
                .send(Message::Text(format!(
                    r#"{{"kind":"session","type":"notice","text":"{e}","is_error":true}}"#
                )))
                .await;
            return;
        }
    };

    let mut evt_rx = setup_event_channel(&agent_facade).await;

    log::info!("ws: session {session_id} connected");

    loop {
        tokio::select! {
            // Event → client
            Some(event) = evt_rx.recv() => {
                let Ok(json) = serde_json::to_string(&event) else { continue };
                if socket.send(Message::Text(json)).await.is_err() {
                    break;
                }
            }
            // Client → command
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<ClientCommand>(&text) {
                            Ok(ClientCommand::Prompt { text }) => {
                                if let Err(notice) = try_slash_command(&text, &mut agent_facade).await {
                                    let _ = socket.send(Message::Text(
                                        serde_json::to_string(&TranscriptEvent::Session(notice))
                                            .unwrap_or_default(),
                                    )).await;
                                    continue;
                                }
                                agent_facade.send(&text).await;
                            }
                            Ok(ClientCommand::Interrupt) => {
                                agent_facade.session.abort().await;
                            }
                            Err(_) => {
                                // Ignore unparseable messages; don't break the connection.
                            }
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = socket.send(Message::Pong(data)).await;
                    }
                    _ => break,
                }
            }
        }
    }

    log::info!("ws: session {session_id} disconnected");
}

/// If `text` is a slash command, execute it on the facade and return a
/// `Notice` payload to emit back. Returns `Ok(())` when the text is not a
/// slash command (caller should forward it to the agent).
async fn try_slash_command(
    text: &str,
    agent_facade: &mut Arc<AgentFacade>,
) -> Result<(), SessionEventPayload> {
    let Some((cmd_name, cmd_args)) =
        pi_coding_agent::core::slash_commands::parse_slash_command(text)
    else {
        return Ok(());
    };
    // `handle_slash_command` needs a unique mutable reference to the facade.
    let agent = Arc::get_mut(agent_facade).ok_or_else(|| SessionEventPayload::Notice {
        text: "session busy, cannot run slash command".to_string(),
        is_error: true,
    })?;
    match handle_slash_command(cmd_name, cmd_args, agent).await {
        Ok(text) if !text.is_empty() => Err(SessionEventPayload::Notice {
            text,
            is_error: false,
        }),
        Ok(_) => Err(SessionEventPayload::Notice {
            text: String::new(),
            is_error: false,
        }),
        Err(e) => Err(SessionEventPayload::Notice {
            text: format!("[Slash Error] {e}"),
            is_error: true,
        }),
    }
}
