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
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use openatrading_core::llm::pi_ai_types::ContentBlock;
use openatrading_core::llm::AgentMessage;
use pi_coding_agent::core::agent_session::{AgentSession, PromptOptions};
use pi_coding_agent::core::agent_session::{AgentSessionEvent, AgentSessionEventListener};
use pi_coding_agent::pi_agent_core::types::AgentEvent;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

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
        AgentSessionEvent::AgentStart => SessionEventPayload::AgentStart,
        AgentSessionEvent::MessageStart { .. } => SessionEventPayload::MessageStart,
        AgentSessionEvent::MessageUpdate {
            assistant_message_event,
            ..
        } => SessionEventPayload::MessageUpdate {
            assistant_message_event: serde_json::to_value(assistant_message_event)
                .unwrap_or_default(),
        },
        AgentSessionEvent::MessageEnd { .. } => SessionEventPayload::MessageEnd,
        AgentSessionEvent::ToolExecutionStart {
            tool_call_id,
            tool_name,
            args,
        } => SessionEventPayload::ToolExecutionStart {
            tool_call_id,
            tool_name,
            args,
        },
        AgentSessionEvent::ToolExecutionUpdate {
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
        AgentSessionEvent::ToolExecutionEnd {
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
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)]
enum ClientCommand {
    Prompt { text: String },
    Interrupt,
    Ping,
    PermissionResponse { id: String, approved: bool },
}

// ── Event channel setup ────────────────────────────────────────────────────

/// Set up the event channel for a session with backpressure support (§14).
///
/// Returns a single receiver (capacity 256, blocks when full) for all event types:
/// text/thinking/tool-call deltas, tool results, turn lifecycle, compaction, retry, etc.
/// All events share one channel so that backpressure propagates uniformly — a slow
/// consumer blocks the producer regardless of event type.
pub async fn setup_event_channel(
    session: &AgentSession,
) -> (
    mpsc::Receiver<TranscriptEvent>,
    pi_coding_agent::core::agent_session::SessionEventUnsubscribeHandle,
) {
    let (tx, rx) = mpsc::channel::<TranscriptEvent>(256);

    // All events share a single channel. `send().await` blocks when the channel
    // is full, applying backpressure uniformly across all event types.
    let tx2 = tx.clone();
    let sync_listener: AgentSessionEventListener = Arc::new(move |event: AgentSessionEvent| {
        let payload = convert_session_event(event);
        // In the sync callback we can't block, so use try_send. The channel is
        // large enough (256) that this should rarely fail under normal operation.
        let _ = tx2.try_send(TranscriptEvent::Session(payload));
    });
    let handle = session.subscribe(sync_listener);

    use openatrading_core::log;
    log::info!(
        "ws: event channel ready (cap=256) for session {}",
        session.get_session_id()
    );
    (rx, handle)
}

// ── WS handler ────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct WsQuery {
    pub id: String,
}

/// `GET /api/ai/sessions/ws?id=xxxx` — WebSocket upgrade.
///
/// If the session ID exists (in pool or on disk), resume it.
/// Otherwise, create a new session with the given ID.
pub async fn ws_handler(
    Query(query): Query<WsQuery>,
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_session(socket, state, query.id))
}

async fn handle_session(mut socket: WebSocket, state: Arc<AppState>, session_id: String) {
    use openatrading_core::log;

    log::info!("ws: upgrade accepted session_id={session_id}");

    let session = match crate::chat_session::ensure_session_with_events(&state.ctx.pool, &session_id).await {
        Ok(s) => s,
        Err(e) => {
            log::error!("ws: failed to ensure session {session_id}: {e}");
            let notice = format!(
                r#"{{"kind":"session","type":"notice","text":"failed to create session: {e}","is_error":true}}"#
            );
            let _ = socket.send(Message::Text(notice)).await;
            return;
        }
    };

    let mut rx = session.rx;
    log::info!("ws: session {session_id} connected — entering event loop");

    forward_events(&mut socket, &mut rx, &state, &session_id).await;

    log::info!("ws: session {session_id} disconnected");
}

/// Forward events from the session event channel to the WebSocket, and handle
/// client messages (prompts, pings, close).
async fn forward_events(
    socket: &mut WebSocket,
    rx: &mut mpsc::Receiver<TranscriptEvent>,
    state: &Arc<AppState>,
    session_id: &str,
) {
    use openatrading_core::log;

    loop {
        tokio::select! {
            Some(event) = rx.recv() => {
                let label = event.kind_label();
                let Ok(json) = serde_json::to_string(&EventEnvelope { schema_version: SCHEMA_VERSION, event }) else { continue };
                log::trace!("ws: {session_id} -> event ({label})");
                if socket.send(Message::Text(json)).await.is_err() {
                    log::warn!("ws: {session_id} socket send failed — breaking");
                    break;
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        log::debug!("ws: {session_id} <- client text (len={})", text.len());
                        if let Ok(ClientCommand::Prompt { text }) = serde_json::from_str::<ClientCommand>(&text) {
                            let pool = state.ctx.pool.clone();
                            let sid = session_id.to_string();
                            tokio::spawn(async move {
                                log::info!("ws: agent turn started for {sid}");
                                let streaming_behavior = if pool.is_stream(&sid).await {
                                    Some("followUp".to_string())
                                } else {
                                    None
                                };
                                let opts = PromptOptions {
                                    streaming_behavior,
                                    ..Default::default()
                                };
                                let _ = pool.prompt_with_opts(&sid, &text, opts).await;
                                log::info!("ws: agent turn completed for {sid}");
                            });
                        } else {
                            log::debug!("ws: {session_id} <- unparseable command — ignored");
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
}
