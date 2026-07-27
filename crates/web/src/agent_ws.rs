//! WebSocket transport for agent session events.
//!
//! All events are unified into a single [`TranscriptEvent`] enum via
//! [`SessionEventPayload`], which includes both `AgentEvent` passthrough variants
//! and session-level events, wrapped in an [`EventEnvelope`] with schema version,
//! and sent over a WebSocket connection. This module is shared between the axum
//! web server and the Tauri IPC layer — the event-production logic is identical,
//! only the delivery mechanism differs (`socket.send` vs `app.emit`).

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use openatrading_core::llm::pi_ai_types::ContentBlock;
use openatrading_core::llm::AgentMessage;
use pi_coding_agent::core::agent_session::{AgentSession, PromptOptions};
use pi_coding_agent::core::agent_session::{AgentSessionEvent, AgentSessionEventListener};
use pi_coding_agent::pi_agent_core::types::AgentEvent;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::chat::handle_slash_command;
use crate::AppState;

// ── Schema version (§16.2) ────────────────────────────────────────────────

/// Current schema version. Increment when making incompatible changes to the
/// event wire format so the frontend can conditionally handle old/new shapes.
pub const SCHEMA_VERSION: u32 = 1;

// ── Event envelope (§16.2) ───────────────────────────────────────────────

/// Top-level envelope wrapping every event sent over the wire.
///
/// The frontend uses `schema_version` for conditional handling.
#[derive(Debug, Serialize)]
pub struct EventEnvelope {
    pub schema_version: u32,
    #[serde(flatten)]
    pub event: TranscriptEvent,
}

// ── Event types ────────────────────────────────────────────────────────────

/// Wire-format event: all agent session events, including both `AgentEvent`
/// passthrough variants and session-level events, unified into a single
/// [`SessionEventPayload`] enum.
#[derive(Debug, Serialize)]
#[serde(tag = "kind")]
pub enum TranscriptEvent {
    /// Agent session event (text deltas, tool calls, thinking, lifecycle, etc.)
    #[serde(rename = "session")]
    Session(SessionEventPayload),
}

impl TranscriptEvent {
    /// Short human-readable label for logging (never serialized).
    pub fn kind_label(&self) -> &'static str {
        match &self {
            TranscriptEvent::Session(p) => match p {
                SessionEventPayload::AgentStart => "session:agent_start",
                SessionEventPayload::AgentEnd { .. } => "session:agent_end",
                SessionEventPayload::TurnStart => "session:turn_start",
                SessionEventPayload::TurnEnd { .. } => "session:turn_end",
                SessionEventPayload::MessageStart => "session:message_start",
                SessionEventPayload::MessageUpdate { .. } => "session:message_update",
                SessionEventPayload::MessageEnd => "session:message_end",
                SessionEventPayload::ToolExecutionStart { .. } => "session:tool_start",
                SessionEventPayload::ToolExecutionUpdate { .. } => "session:tool_update",
                SessionEventPayload::ToolExecutionEnd { .. } => "session:tool_end",
                SessionEventPayload::AgentSettled => "session:agent_settled",
                SessionEventPayload::CompactionStart { .. } => "session:compaction_start",
                SessionEventPayload::CompactionEnd { .. } => "session:compaction_end",
                SessionEventPayload::AutoRetryStart { .. } => "session:auto_retry_start",
                SessionEventPayload::AutoRetryEnd { .. } => "session:auto_retry_end",
                _ => "session:other",
            },
        }
    }
}

/// Serializable subset of [`AgentSessionEvent`].
///
/// Only the variants relevant to the frontend are included; unknown/future
/// variants fall through to [`SessionEventPayload::Other`].
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEventPayload {
    // ── AgentEvent passthrough variants ──
    /// Agent started thinking.
    AgentStart,
    /// Agent turn ended (with retry info).
    AgentEnd { will_retry: bool },
    /// A new assistant turn started.
    TurnStart,
    /// An assistant turn ended, with tool results.
    TurnEnd {
        tool_results: Vec<ToolResultPayload>,
    },
    /// A message within a turn started.
    MessageStart,
    /// Streaming update for a message (text/thinking/tool-call delta).
    MessageUpdate {
        assistant_message_event: serde_json::Value,
    },
    /// A message within a turn ended.
    MessageEnd,
    /// A tool execution started.
    ToolExecutionStart {
        tool_call_id: String,
        tool_name: String,
        args: serde_json::Value,
    },
    /// Streaming update for a running tool execution.
    ToolExecutionUpdate {
        tool_call_id: String,
        tool_name: String,
        args: serde_json::Value,
        partial_result: serde_json::Value,
    },
    /// A tool execution completed.
    ToolExecutionEnd {
        tool_call_id: String,
        tool_name: String,
        result: serde_json::Value,
        is_error: bool,
    },
    // ── Session-specific events ──
    /// All agent tasks fully settled — input can be re-enabled.
    AgentSettled,
    /// Session compaction started.
    CompactionStart { reason: String },
    /// Session compaction ended.
    CompactionEnd {
        reason: String,
        aborted: bool,
        will_retry: bool,
        error_message: Option<String>,
    },
    /// Automatic retry started.
    AutoRetryStart {
        attempt: u32,
        max_attempts: u32,
        delay_ms: u64,
        error_message: String,
    },
    /// Automatic retry ended.
    AutoRetryEnd {
        success: bool,
        attempt: u32,
        final_error: Option<String>,
    },
    /// Queue status update.
    QueueUpdate {
        steering: Vec<String>,
        follow_up: Vec<String>,
    },
    /// Session metadata changed.
    SessionInfoChanged { name: Option<String> },
    /// Slash command result / error rendered as a system notice block.
    Notice { text: String, is_error: bool },
    /// Permission request for human-in-the-loop approval (§11).
    PermissionRequest {
        id: String,
        tool_name: String,
        tool_input: serde_json::Value,
    },
    /// Resync signal for reconnection (§9.1).
    ResyncRequired,
    /// Catch-all for variants not explicitly mapped.
    Other,
}

/// Preview payload for tool results, with truncation for long content (§15).
#[derive(Debug, Serialize)]
pub struct ToolResultPayload {
    pub preview: String,
    pub truncated: bool,
    pub full_length: usize,
}

/// Truncation limit for tool result content in event stream (§15).
const TOOL_RESULT_PREVIEW_LIMIT: usize = 2000;

/// Ring buffer capacity: events retained per session for reconnection replay (§9.1).

/// Convert a tool result text into a preview payload, truncating if too long.
fn to_tool_result_payload(content: &str) -> ToolResultPayload {
    let len = content.chars().count();
    if len <= TOOL_RESULT_PREVIEW_LIMIT {
        ToolResultPayload {
            preview: content.to_string(),
            truncated: false,
            full_length: content.len(),
        }
    } else {
        let preview: String = content.chars().take(TOOL_RESULT_PREVIEW_LIMIT).collect();
        ToolResultPayload {
            preview,
            truncated: true,
            full_length: content.len(),
        }
    }
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
            reason: reason.to_string(),
        },
        AgentSessionEvent::CompactionEnd {
            reason,
            aborted,
            will_retry,
            error_message,
            ..
        } => SessionEventPayload::CompactionEnd {
            reason: reason.to_string(),
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

/// Convert an [`AgentEvent`] into the serializable [`SessionEventPayload`].
///
/// All `AgentEvent` variants are mapped to their corresponding
/// `SessionEventPayload` variants so the frontend only needs to handle
/// a single event kind.
pub fn convert_agent_event(event: AgentEvent) -> SessionEventPayload {
    match event {
        AgentEvent::AgentStart => SessionEventPayload::AgentStart,
        AgentEvent::AgentEnd { .. } => SessionEventPayload::AgentEnd { will_retry: false },
        AgentEvent::TurnStart => SessionEventPayload::TurnStart,
        AgentEvent::TurnEnd { tool_results, .. } => SessionEventPayload::TurnEnd {
            tool_results: extract_tool_result_texts(&tool_results),
        },
        AgentEvent::MessageStart { .. } => SessionEventPayload::MessageStart,
        AgentEvent::MessageUpdate {
            assistant_message_event,
            ..
        } => SessionEventPayload::MessageUpdate {
            assistant_message_event: serde_json::to_value(assistant_message_event)
                .unwrap_or_default(),
        },
        AgentEvent::MessageEnd { .. } => SessionEventPayload::MessageEnd,
        AgentEvent::ToolExecutionStart {
            tool_call_id,
            tool_name,
            args,
        } => SessionEventPayload::ToolExecutionStart {
            tool_call_id,
            tool_name,
            args,
        },
        AgentEvent::ToolExecutionUpdate {
            tool_call_id,
            tool_name,
            args,
            partial_result,
        } => SessionEventPayload::ToolExecutionUpdate {
            tool_call_id,
            tool_name,
            args,
            partial_result,
        },
        AgentEvent::ToolExecutionEnd {
            tool_call_id,
            tool_name,
            result,
            is_error,
        } => SessionEventPayload::ToolExecutionEnd {
            tool_call_id,
            tool_name,
            result,
            is_error,
        },
    }
}

/// Extract human-readable text from a slice of tool-result messages.
fn extract_tool_result_texts(messages: &[AgentMessage]) -> Vec<ToolResultPayload> {
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
                    Some(to_tool_result_payload(&text))
                }
            }
            _ => None,
        })
        .collect()
}

// ── Client commands ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ClientCommand {
    Prompt { text: String },
    Interrupt,
    Ping,
    PermissionResponse { id: String, approved: bool },
}

// ── Event channel setup ────────────────────────────────────────────────────

/// Set up the event channel for a session.
///
/// Returns an unbounded receiver that yields [`TranscriptEvent`] values from
/// both `AgentEvent` (async) and `AgentSessionEvent` (sync) sources, merged
/// into a single channel for ordering guarantees (§10.1).
/// Set up the event channel for a session with backpressure support (§14).
///
/// Returns a pair of receivers:
/// - `delta_rx` (capacity 64, drops when full) — for text/thinking/tool-call deltas
/// - `durable_rx` (capacity 256, blocks when full) — for tool results, turn lifecycle,
///   compaction, retry, and other non-delta events
///
/// The two-channel design ensures that high-frequency delta events don't starve
/// important lifecycle events, and that a slow consumer doesn't cause unbounded
/// memory growth.
pub async fn setup_event_channel(
    session: &AgentSession,
) -> (
    mpsc::Receiver<TranscriptEvent>,
    mpsc::Receiver<TranscriptEvent>,
    pi_coding_agent::core::agent_session::SessionEventUnsubscribeHandle,
) {
    let (delta_tx, delta_rx) = mpsc::channel::<TranscriptEvent>(64);
    let (durable_tx, durable_rx) = mpsc::channel::<TranscriptEvent>(256);

    // Route events to the appropriate channel:
    // - Delta events (high-frequency text/thinking/tool-call updates) → delta channel (drops when full)
    // - Durable events (lifecycle, tool results, compaction, retry) → durable channel (blocks when full)
    let delta_tx2 = delta_tx.clone();
    let durable_tx2 = durable_tx.clone();
    let sync_listener: AgentSessionEventListener = Arc::new(move |event: AgentSessionEvent| {
        let payload = convert_session_event(event);
        let is_delta = matches!(
            &payload,
            SessionEventPayload::MessageUpdate { .. }
                | SessionEventPayload::ToolExecutionUpdate { .. }
        );
        let tx: &mpsc::Sender<TranscriptEvent> = if is_delta { &delta_tx2 } else { &durable_tx2 };
        let _ = tx.try_send(TranscriptEvent::Session(payload));
    });
    let handle = session.subscribe(sync_listener);

    use openatrading_core::log;
    log::info!(
        "ws: event channel ready (delta cap=64, durable cap=256) for session {}",
        session.get_session_id()
    );
    (delta_rx, durable_rx, handle)
}

// ── WS handler ────────────────────────────────────────────────────────────────────────────

/// `GET /api/ai/sessions/:id/ws` — WebSocket upgrade.
pub async fn ws_handler(
    Path(session_id): Path<String>,
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_session(socket, state, session_id))
}

async fn handle_session(mut socket: WebSocket, state: Arc<AppState>, session_id: String) {
    use openatrading_core::log;

    log::info!("ws: upgrade accepted session_id={session_id}");

    // Retrieve the live runtime from the SessionPool. We do NOT remove it —
    // reconnections must reuse the same AgentSessionRuntime to preserve the
    // running agent's context and event stream.
    let runtime = match state.ctx.pool.get_session(&session_id).await {
        Some(rt) => {
            log::info!("ws: pool HIT — reusing live runtime for {session_id}");
            rt
        }
        None => {
            log::info!("ws: pool MISS for {session_id} — creating new runtime");
            // Not in pool — create a new runtime. This happens after a
            // backend restart (the in-memory pool is empty) or when a
            // client reconnects to a session whose runtime was evicted.
            // Use the original session_id so REST API calls using the same
            // ID continue to work.
            let cwd = std::env::current_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            match state
                .ctx
                .pool
                .create_session_with_id(&session_id, &cwd)
                .await
            {
                Ok(()) => {
                    log::info!("ws: re-created runtime for {session_id}");
                    match state.ctx.pool.get_session(&session_id).await {
                        Some(rt) => rt,
                        None => {
                            let e = "failed to get runtime after re-creation";
                            log::error!("ws: {e}");
                            let notice = format!(
                                r#"{{"kind":"session","type":"notice","text":"{e}","is_error":true}}"#
                            );
                            let _ = socket.send(Message::Text(notice)).await;
                            return;
                        }
                    }
                }
                Err(e) => {
                    log::error!("ws: failed to create session {session_id}: {e}");
                    let notice = format!(
                        r#"{{"kind":"session","type":"notice","text":"{e}","is_error":true}}"#
                    );
                    log::info!("ws: {session_id} -> error notice");
                    let _ = socket.send(Message::Text(notice)).await;
                    return;
                }
            }
        }
    };

    let (mut delta_rx, mut durable_rx, _subscription_handle) = {
        let rt = runtime.lock().await;
        setup_event_channel(rt.session()).await
    };
    log::info!("ws: event channel subscribed for {session_id}");

    log::info!("ws: session {session_id} connected — entering event loop");

    loop {
        // `biased` ensures durable events (lifecycle, tool results) are
        // drained before delta events when both are ready (§14, finding #8).
        tokio::select! {
            biased;
            // Durable events (tool results, lifecycle, compaction, retry) — must deliver
            Some(event) = durable_rx.recv() => {
                log::debug!("ws: {session_id} -> durable event ({})", event.kind_label());
                // Track running state (§7.4): agent_end/agent_settled → session is idle.
                matches!(&event, TranscriptEvent::Session(payload) if matches!(payload, SessionEventPayload::AgentEnd { .. } | SessionEventPayload::AgentSettled));
                // Serialize the envelope and send to the WebSocket.
                let label = event.kind_label();
                let Ok(json) = serde_json::to_string(&EventEnvelope { schema_version: SCHEMA_VERSION, event }) else { continue };
                log::debug!("ws: {session_id} -> durable event ({label})");
                if socket.send(Message::Text(json)).await.is_err() {
                    log::warn!("ws: {session_id} socket send failed on durable event — breaking");
                    break;
                }
            }
            // Delta events (text/thinking/tool-call deltas) — can drop if slow
            Some(event) = delta_rx.recv() => {
                let label = event.kind_label();
                let Ok(json) = serde_json::to_string(&EventEnvelope { schema_version: SCHEMA_VERSION, event }) else { continue };
                log::trace!("ws: {session_id} -> delta ({label})");
                if socket.send(Message::Text(json)).await.is_err() {
                    log::warn!("ws: {session_id} socket send failed on delta — breaking");
                    break;
                }
            }
            // Client → command
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        log::debug!("ws: {session_id} <- client text (len={})", text.len());
                        match serde_json::from_str::<ClientCommand>(&text) {
                            Ok(ClientCommand::Prompt { text }) => {
                                log::info!(
                                    "ws: {session_id} <- Prompt (len={}) is_slash={}",
                                    text.len(),
                                    text.trim_start().starts_with('/')
                                );
                                if let Err(notice) = try_slash_command(&text, &runtime).await {
                                    let Ok(json) = serde_json::to_string(&EventEnvelope { schema_version: SCHEMA_VERSION, event: TranscriptEvent::Session(notice) }) else { continue };
                                    log::info!("ws: {session_id} -> slash-command notice");
                                    let _ = socket.send(Message::Text(json)).await;
                                    continue;
                                }
                                // Spawn the turn so this select! loop keeps
                                // forwarding events concurrently.
                                // Check streaming state and submit prompt atomically
                                // under the same lock to avoid races.
                                let rt = runtime.clone();
                                let task_id = session_id.clone();
                                let text = text.to_string();
                                tokio::task::spawn_blocking(move || {
                                    let handle = tokio::runtime::Handle::current();
                                    handle.block_on(async {
                                        log::info!("ws: agent turn started for {task_id}");
                                        let session = rt.lock().await;
                                        let streaming_behavior = if session.session().is_streaming().await {
                                            Some("followUp".to_string())
                                        } else {
                                            None
                                        };
                                        let opts = PromptOptions {
                                            streaming_behavior,
                                            ..Default::default()
                                        };
                                        session.session().prompt(&text, Some(opts)).await;
                                        log::info!("ws: agent turn completed for {task_id}");
                                    });
                                });
                            }
                            Ok(ClientCommand::Interrupt) => {
                                log::info!("ws: {session_id} <- Interrupt");
                                let rt = runtime.clone();
                                tokio::spawn(async move {
                                    rt.lock().await.session().abort().await;
                                });
                            }
                            Ok(ClientCommand::Ping) => {
                                log::debug!("ws: {session_id} <- Ping");
                                // Application-level ping acknowledged; no response needed.
                            }
                            Ok(ClientCommand::PermissionResponse { id, approved }) => {
                                // TODO: Wire up to pi-coding-agent's permission system when available.
                                // See docs/agentTransport.md §11 for the planned implementation.
                                log::warn!("ws: {session_id} <- PermissionResponse id={id} approved={approved} (not yet wired)");
                            }
                            Err(e) => {
                                // Ignore unparseable messages; don't break the connection.
                                log::debug!("ws: {session_id} <- unparseable message: {e}");
                            }
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = socket.send(Message::Pong(data)).await;
                    }
                    _ => {
                        log::info!("ws: {session_id} socket closed/error — exiting event loop");
                        break;
                    }
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
    runtime: &Arc<
        tokio::sync::Mutex<pi_coding_agent::core::agent_session_runtime::AgentSessionRuntime>,
    >,
) -> Result<(), SessionEventPayload> {
    let Some((cmd_name, cmd_args)) =
        pi_coding_agent::core::slash_commands::parse_slash_command(text)
    else {
        return Ok(());
    };
    // `handle_slash_command` needs a mutable reference to the session.
    // Hold the lock only for the duration of the command. The lock is
    // released when `rt` is dropped at the end of this block.
    let mut rt = runtime.lock().await;
    let session = rt.session_mut();
    match handle_slash_command(cmd_name, cmd_args, session).await {
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
