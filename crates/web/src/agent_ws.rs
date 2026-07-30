//! WebSocket transport for agent session events.
//!
//! All events are unified into a single [`TranscriptEvent`] enum via
//! [`SessionEventPayload`], which includes both `AgentEvent` passthrough variants
//! and session-level events, wrapped in an [`EventEnvelope`] with schema version,
//! and sent over a WebSocket connection. This module is shared between the axum
//! web server and the Tauri IPC layer — the event-production logic is identical,
//! only the delivery mechanism differs (`socket.send` vs `app.emit`).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use openatrading_core::llm::pi_ai_types::{ContentBlock, StopReason};
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
    /// Monotonic sequence number (starts at 1). `0` is reserved for out-of-band
    /// events that bypass dedup (e.g. session create-failure before a
    /// [`SessionEventState`] exists). The frontend dedups live events by `seq`
    /// and uses `lastSeq` to resume after a WebSocket reconnect.
    pub seq: u64,
    #[serde(flatten)]
    pub event: TranscriptEvent,
}

// ── Session event state: seq + bounded ring buffer (§ reconnect resume) ──

/// Bounded ring buffer capacity for recent emitted event envelopes.
///
/// Must be ≥ the live mpsc channel capacity (256) so that events dropped while
/// the channel is full are still recoverable from the ring on the next
/// reconnect-replay (within this window).
const EVENT_RING_CAPACITY: usize = 512;

/// Per-session state backing monotonic `seq` numbering and a bounded ring
/// buffer of recent serialized [`EventEnvelope`]s.
///
/// Both the live event-listener callback and the prompt-error path route
/// through [`SessionEventState::emit`], so every event the client can receive
/// carries a `seq` and is retained in the ring for reconnect resume.
///
/// The ring also recovers events that the live mpsc `try_send` dropped when
/// full: `emit` pushes to the ring *before* `try_send`, so a dropped live event
/// is still replayed on the next reconnect (if within the window). If the gap
/// is larger than the window, [`SessionEventState::replay_after`] returns
/// `Err(())` and the caller emits [`SessionEventPayload::ResyncRequired`].
pub struct SessionEventState {
    seq: AtomicU64,
    ring: StdMutex<VecDeque<(u64, String)>>,
    capacity: usize,
}

/// Error returned by [`SessionEventState::replay_after`] when the requested
/// `last_seq` points before the oldest entry still held in the ring buffer —
/// the reconnect gap is larger than the retained window, so the caller must
/// signal [`SessionEventPayload::ResyncRequired`] instead of replaying.
#[derive(Debug)]
pub struct ReplayGap;

impl SessionEventState {
    /// Create a fresh state. `seq` starts at 0 so the first emitted event is 1.
    pub fn new() -> Self {
        Self::with_capacity(EVENT_RING_CAPACITY)
    }

    /// Create with an explicit ring capacity (used by tests).
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            seq: AtomicU64::new(0),
            ring: StdMutex::new(VecDeque::with_capacity(capacity)),
            capacity,
        }
    }

    /// Assign the next monotonic `seq`, wrap `event` in an [`EventEnvelope`],
    /// serialize it, push the serialized JSON into the ring buffer (evicting
    /// the oldest entry when at capacity), and return the serialized JSON.
    ///
    /// This is the single seq/ring entry point used by both the live
    /// event-listener and the prompt-error path, so all events are numbered
    /// and retained uniformly.
    pub fn emit(&self, event: TranscriptEvent) -> String {
        let seq = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        let envelope = EventEnvelope {
            schema_version: SCHEMA_VERSION,
            seq,
            event,
        };
        let json = serde_json::to_string(&envelope).unwrap_or_else(|e| {
            // Serialization only fails on non-serializable content; the payload
            // types are all `Serialize` by construction, so this is an
            // invariant violation. Fall back to a minimal error envelope rather
            // than panicking in a producer path.
            openatrading_core::log::error!("agent_ws: serialize event envelope seq={seq}: {e}");
            format!(
                r#"{{"schema_version":{sv},"seq":{s},"kind":"session","type":"notice","text":"internal serialization error","is_error":true}}"#,
                sv = SCHEMA_VERSION,
                s = seq
            )
        });
        let mut ring = self.ring.lock().unwrap_or_else(|e| e.into_inner());
        if ring.len() >= self.capacity {
            ring.pop_front();
        }
        ring.push_back((seq, json.clone()));
        json
    }

    /// Replay all ring entries with `seq > last_seq`, in order.
    ///
    /// Returns `Ok(vec![])` when the ring is empty (nothing to replay) or when
    /// `last_seq` is at/above the newest entry. Returns `Err(())` when
    /// `last_seq + 1` is below the oldest retained entry — the gap is larger
    /// than the window, so the caller must signal [`ResyncRequired`].
    pub fn replay_after(&self, last_seq: u64) -> Result<Vec<String>, ReplayGap> {
        let ring = self.ring.lock().unwrap_or_else(|e| e.into_inner());
        let Some((oldest, _)) = ring.front() else {
            return Ok(Vec::new());
        };
        if last_seq + 1 < *oldest {
            return Err(ReplayGap);
        }
        Ok(ring
            .iter()
            .filter(|(seq, _)| *seq > last_seq)
            .map(|(_, json)| json.clone())
            .collect())
    }
}

impl Default for SessionEventState {
    fn default() -> Self {
        Self::new()
    }
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
                SessionEventPayload::ResyncRequired => "session:resync_required",
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
    /// Reconnection gap too large to replay — the client must reset and
    /// reload full history via `fetchHistory`. Sent (with `seq` from the
    /// session's [`SessionEventState`]) when `lastSeq` points before the
    /// oldest entry still held in the ring buffer.
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

/// Extract a human-readable error from an assistant [`AgentMessage::MessageEnd`].
///
/// The core `MessageEnd` event is a fieldless passthrough on the wire, so a
/// failed turn (builder error, 429, timeout, …) would otherwise render as a
/// silent empty turn. When the assistant message carries an error we surface
/// it as a `Notice { is_error: true }` instead, which the frontend already
/// renders as an error system-notice block and marks the turn terminal.
fn message_end_payload(message: &AgentMessage) -> SessionEventPayload {
    if let AgentMessage::Assistant {
        stop_reason,
        error_message,
        ..
    } = message
    {
        let errored = matches!(stop_reason, Some(StopReason::Error)) || error_message.is_some();
        if errored {
            let text = error_message
                .clone()
                .unwrap_or_else(|| "助手响应失败，未返回任何内容".to_string());
            return SessionEventPayload::Notice {
                text,
                is_error: true,
            };
        }
    }
    SessionEventPayload::MessageEnd
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
        AgentSessionEvent::MessageEnd { message } => message_end_payload(&message),
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
        AgentEvent::MessageEnd { message } => message_end_payload(&message),
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
/// Returns a single receiver (capacity 256, blocks when full) for all event
/// types, carrying already-serialized [`EventEnvelope`] JSON strings (with
/// `seq` assigned by `event_state`). All event types share one channel so
/// backpressure propagates uniformly — a slow consumer blocks the producer
/// regardless of event type.
///
/// `event_state` is shared across reconnects (lives in [`AppState`]), so `seq`
/// and the ring buffer persist across WebSocket reconnects for the same
/// session — enabling `lastSeq` resume.
pub async fn setup_event_channel(
    session: &AgentSession,
    event_state: &Arc<SessionEventState>,
) -> (
    mpsc::Sender<String>,
    mpsc::Receiver<String>,
    pi_coding_agent::core::agent_session::SessionEventUnsubscribeHandle,
) {
    let (tx, rx) = mpsc::channel::<String>(256);

    // All events share a single channel. The listener serializes the event
    // through `event_state.emit` (assigning `seq` + pushing to the ring) and
    // forwards the JSON string. `try_send` is used because the sync callback
    // can't block; when the channel is full the event is still retained in the
    // ring buffer and recovered on the next reconnect-replay.
    let tx2 = tx.clone();
    let state = Arc::clone(event_state);
    let sync_listener: AgentSessionEventListener = Arc::new(move |event: AgentSessionEvent| {
        let payload = convert_session_event(event);
        let json = state.emit(TranscriptEvent::Session(payload));
        let _ = tx2.try_send(json);
    });
    let handle = session.subscribe(sync_listener);

    use openatrading_core::log;
    log::info!(
        "ws: event channel ready (cap=256) for session {}",
        session.get_session_id()
    );
    (tx, rx, handle)
}

// ── WS handler ────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct WsQuery {
    pub id: String,
    /// Last `seq` the client received (reconnect resume). Absent/0 on a cold
    /// start — the client then loads full history via `fetchHistory`.
    #[serde(default, rename = "lastSeq")]
    pub last_seq: Option<u64>,
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
    ws.on_upgrade(move |socket| handle_session(socket, state, query.id, query.last_seq))
}

async fn handle_session(
    mut socket: WebSocket,
    state: Arc<AppState>,
    session_id: String,
    last_seq: Option<u64>,
) {
    use openatrading_core::log;

    log::info!("ws: upgrade accepted session_id={session_id} last_seq={last_seq:?}");

    // The event_state persists across reconnects for this session (lives in
    // AppState), so `seq` and the ring buffer survive a WS reconnect.
    let event_state = state.get_event_state(&session_id).await;

    let session = match crate::chat_session::ensure_session_with_events(
        &state.ctx.pool,
        &session_id,
        &event_state,
    )
    .await
    {
        Ok(s) => s,
        Err(e) => {
            // No event_state-bound session exists yet; send an out-of-band
            // notice with seq=0 (client dedup skips seq=0) and bail.
            log::error!("ws: failed to ensure session {session_id}: {e}");
            let event = TranscriptEvent::Session(SessionEventPayload::Notice {
                text: format!("failed to create session: {e}"),
                is_error: true,
            });
            let Ok(json) = serde_json::to_string(&EventEnvelope {
                schema_version: SCHEMA_VERSION,
                seq: 0,
                event,
            }) else {
                return;
            };
            let _ = socket.send(Message::Text(json)).await;
            return;
        }
    };

    // Reconnect resume: replay the gap from the ring buffer before consuming
    // live events, so events emitted during the disconnect are not lost.
    if let Some(n) = last_seq {
        match event_state.replay_after(n) {
            Ok(jsons) => {
                for json in jsons {
                    if socket.send(Message::Text(json)).await.is_err() {
                        log::warn!("ws: {session_id} replay send failed — breaking");
                        return;
                    }
                }
            }
            Err(_) => {
                // Gap too large (evicted). Signal resync — client resets and
                // reloads full history. seq is assigned by event_state.
                let json = event_state.emit(TranscriptEvent::Session(
                    SessionEventPayload::ResyncRequired,
                ));
                let _ = socket.send(Message::Text(json)).await;
            }
        }
    }

    let mut rx = session.rx;
    let tx = session.tx;
    log::info!("ws: session {session_id} connected — entering event loop");

    forward_events(&mut socket, &mut rx, &state, &session_id, tx, &event_state).await;

    log::info!("ws: session {session_id} disconnected");
}

/// Forward events from the session event channel to the WebSocket, and handle
/// client messages (prompts, pings, close).
async fn forward_events(
    socket: &mut WebSocket,
    rx: &mut mpsc::Receiver<String>,
    state: &Arc<AppState>,
    session_id: &str,
    tx: mpsc::Sender<String>,
    event_state: &Arc<SessionEventState>,
) {
    use openatrading_core::log;

    loop {
        tokio::select! {
            Some(json) = rx.recv() => {
                log::trace!("ws: {session_id} -> event");
                if socket.send(Message::Text(json)).await.is_err() {
                    log::warn!("ws: {session_id} socket send failed — breaking");
                    break;
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        log::debug!("ws: {session_id} <- client text (len={})", text.len());
                        match serde_json::from_str::<ClientCommand>(&text) {
                            Ok(ClientCommand::Prompt { text }) => {
                                let pool = state.ctx.pool.clone();
                                let sid = session_id.to_string();
                                let prompt_tx = tx.clone();
                                let prompt_state = Arc::clone(event_state);
                                let worker = tokio::spawn(async move {
                                    log::info!("ws: agent turn started for {sid}");
                                    // pi-rs `prompt()` checks `is_streaming()` internally and
                                    // uses `streaming_behavior` only to choose steer vs followUp.
                                    // We pass a fixed policy (followUp = queue as a new turn) and
                                    // let pi-rs decide whether a stream is in progress, avoiding
                                    // a racy `is_stream()` TOCTOU check on our side.
                                    let opts = PromptOptions {
                                        streaming_behavior: Some("followUp".to_string()),
                                        ..Default::default()
                                    };
                                    if let Err(e) = pool.prompt_with_opts(&sid, &text, opts).await {
                                        log::warn!("ws: prompt failed for {sid}: {e}");
                                        // Route the error through event_state.emit so it gets a
                                        // seq + lands in the ring (recoverable on reconnect).
                                        let json = prompt_state.emit(TranscriptEvent::Session(
                                            SessionEventPayload::Notice { text: e, is_error: true },
                                        ));
                                        let _ = prompt_tx.send(json).await;
                                    }
                                    log::info!("ws: agent turn completed for {sid}");
                                });
                                // Observe the spawned task so a panic is never silently
                                // swallowed (project rule: no fire-and-forget JoinHandles).
                                let watch_sid = session_id.to_string();
                                tokio::spawn(async move {
                                    if let Err(e) = worker.await {
                                        log::error!("ws: prompt task panicked for {watch_sid}: {e}");
                                    }
                                });
                            }
                            Ok(ClientCommand::Interrupt) => {
                                log::debug!("ws: {session_id} <- interrupt");
                                if let Some(rt) = state.ctx.pool.get_session(session_id).await {
                                    rt.lock().await.session().abort().await;
                                }
                            }
                            Ok(ClientCommand::PermissionResponse { id, approved }) => {
                                log::debug!("ws: {session_id} <- permission_response id={id} approved={approved}");
                                // Permission response forwarding is not yet implemented.
                            }
                            Ok(ClientCommand::Ping) => {
                                // Ignore pings in text form; they're handled as WebSocket pings.
                            }
                            Err(_) => {
                                log::debug!("ws: {session_id} <- unparseable command — ignored");
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use openatrading_core::llm::pi_ai_types::{StopReason, Usage};

    /// Builds an assistant [`AgentMessage`] with the given stop reason / error.
    fn assistant(stop_reason: Option<StopReason>, error: Option<&str>) -> AgentMessage {
        AgentMessage::Assistant {
            content: vec![],
            api: "openai".to_string(),
            provider: "openrouter".to_string(),
            model: "m".to_string(),
            usage: Usage::default(),
            stop_reason,
            error_message: error.map(str::to_string),
            timestamp: 0,
        }
    }

    #[test]
    fn message_end_error_surfaces_as_notice() {
        // builder error / 429 / timeout → stop_reason Error + error_message.
        let msg = assistant(Some(StopReason::Error), Some("builder error"));
        match message_end_payload(&msg) {
            SessionEventPayload::Notice { text, is_error } => {
                assert!(is_error, "error notice must set is_error=true");
                assert_eq!(text, "builder error");
            }
            other => panic!("expected Notice, got {other:?}"),
        }
    }

    #[test]
    fn message_end_error_message_alone_also_surfaces() {
        // Some providers set error_message without stop_reason == Error.
        let msg = assistant(Some(StopReason::Stop), Some("rate limited"));
        assert!(matches!(
            message_end_payload(&msg),
            SessionEventPayload::Notice { is_error: true, .. }
        ));
    }

    #[test]
    fn message_end_normal_passes_through() {
        // A successful turn (content streamed, no error) must still emit MessageEnd
        // so the happy path is unchanged.
        let msg = assistant(Some(StopReason::Stop), None);
        assert!(matches!(
            message_end_payload(&msg),
            SessionEventPayload::MessageEnd
        ));
    }

    #[test]
    fn message_end_aborted_without_error_passes_through() {
        // An aborted turn is an interruption, not a surfaced error — leave as MessageEnd.
        let msg = assistant(Some(StopReason::Aborted), None);
        assert!(matches!(
            message_end_payload(&msg),
            SessionEventPayload::MessageEnd
        ));
    }

    #[test]
    fn message_end_error_serializes_as_notice() {
        // Wire shape: the frontend reducer keys on `type: "notice"` + `is_error`.
        let msg = assistant(Some(StopReason::Error), Some("oops"));
        let payload = message_end_payload(&msg);
        let json = serde_json::to_string(&payload)
            .unwrap_or_else(|e| panic!("serialize notice payload: {e}"));
        assert!(json.contains(r#""type":"notice""#), "json={json}");
        assert!(json.contains(r#""is_error":true"#), "json={json}");
        assert!(json.contains(r#""text":"oops""#), "json={json}");
    }

    // ── SessionEventState ───────────────────────────────────────────────

    fn notice_payload() -> TranscriptEvent {
        TranscriptEvent::Session(SessionEventPayload::Notice {
            text: "x".to_string(),
            is_error: false,
        })
    }

    fn seq_of(json: &str) -> u64 {
        // envelopes serialize as {schema_version, seq, kind, ...}
        let v: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
        v.get("seq").and_then(|x| x.as_u64()).unwrap_or(0)
    }

    #[test]
    fn emit_produces_strictly_increasing_seqs() {
        let st = SessionEventState::new();
        let mut prev = 0u64;
        for _ in 0..10 {
            let seq = seq_of(&st.emit(notice_payload()));
            assert!(seq > prev, "seq {seq} not > prev {prev}");
            prev = seq;
        }
        assert_eq!(prev, 10, "first seq must be 1, so after 10 emits seq=10");
    }

    #[tokio::test]
    async fn emit_concurrent_seqs_are_unique() {
        let st = Arc::new(SessionEventState::new());
        let n = 8;
        let per = 100;
        let mut handles = Vec::new();
        for _ in 0..n {
            let st = Arc::clone(&st);
            handles.push(tokio::spawn(async move {
                let mut out = Vec::new();
                for _ in 0..per {
                    out.push(seq_of(&st.emit(notice_payload())));
                }
                out
            }));
        }
        let mut all = std::collections::HashSet::new();
        for h in handles {
            for s in h.await.unwrap_or_else(|e| panic!("spawn join: {e}")) {
                assert!(all.insert(s), "duplicate seq {s}");
            }
        }
        assert_eq!(all.len(), n * per);
    }

    #[test]
    fn replay_after_returns_only_newer_in_order() {
        let st = SessionEventState::new();
        let s1 = seq_of(&st.emit(notice_payload()));
        let _s2 = seq_of(&st.emit(notice_payload()));
        let s3 = seq_of(&st.emit(notice_payload()));
        let res = st
            .replay_after(s1)
            .unwrap_or_else(|_| panic!("replay_after s1"));
        assert_eq!(res.len(), 2, "should replay s2 and s3");
        assert_eq!(seq_of(&res[0]), s1 + 1);
        assert_eq!(seq_of(&res[1]), s3);
    }

    #[test]
    fn replay_after_empty_returns_empty() {
        let st = SessionEventState::new();
        assert_eq!(
            st.replay_after(0)
                .unwrap_or_else(|_| panic!("replay_after 0"))
                .len(),
            0
        );
    }

    #[test]
    fn replay_after_gap_returns_err() {
        let st = SessionEventState::with_capacity(4);
        for _ in 0..6 {
            st.emit(notice_payload());
        }
        // ring now holds seq 3..=6 (capacity 4). lastSeq=1 → 1+1=2 < oldest=3 → gap.
        assert!(st.replay_after(1).is_err());
    }

    #[test]
    fn ring_evicts_oldest_beyond_capacity() {
        let st = SessionEventState::with_capacity(3);
        for _ in 0..4 {
            st.emit(notice_payload());
        }
        // After 4 emits with cap 3, ring holds seq 2..=4 (seq 1 evicted).
        // replay_after(1): 1+1=2 >= oldest=2 → Ok, returns seq>1 = {2,3,4}.
        let res = st
            .replay_after(1)
            .unwrap_or_else(|_| panic!("replay_after 1"));
        assert_eq!(res.len(), 3, "only capacity entries remain");
        let first = seq_of(&res[0]);
        assert_eq!(first, 2, "oldest surviving seq is 2");
        // replay_after(0) must be a gap (0+1=1 < oldest=2).
        assert!(st.replay_after(0).is_err());
    }
}
