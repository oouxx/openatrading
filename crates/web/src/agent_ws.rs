//! WebSocket transport for agent session events.
//!
//! Both `AgentEvent` (async) and `AgentSessionEvent` (sync) are unified into a
//! single [`TranscriptEvent`] enum, wrapped in an [`EventEnvelope`] with seq
//! numbering and schema version, and sent over a WebSocket connection. This
//! module is shared between the axum web server and the Tauri IPC layer — the
//! event-production logic is identical, only the delivery mechanism differs
//! (`socket.send` vs `app.emit`).
//!
//! ## Reconnection (§9)
//!
//! Each event is assigned a monotonically increasing `seq`. A ring buffer
//! (`SessionEventBuffer`) retains the last N events per session. When a client
//! reconnects with `?last_seq=...`, the buffer replays missed events before
//! resuming the live stream. If the buffer has already evicted the requested
//! seq, a `resync_required` signal is sent instead.

use std::collections::VecDeque;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;
use openatrading_core::llm::pi_ai_types::ContentBlock;
use openatrading_core::llm::{AgentFacade, AgentMessage};
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
/// The frontend uses `schema_version` for conditional handling and `seq` for
/// reconnection replay tracking.
#[derive(Debug, Serialize)]
pub struct EventEnvelope {
    pub schema_version: u32,
    pub seq: u64,
    #[serde(flatten)]
    pub event: TranscriptEvent,
}

// ── Event types ────────────────────────────────────────────────────────────

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
        tool_results: Vec<ToolResultPayload>,
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
const SESSION_BUFFER_CAPACITY: usize = 500;

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

// ── Session event buffer (§9.1) ────────────────────────────────────────────

/// Ring buffer that retains the last N events per session for reconnection
/// replay. Each event is assigned a monotonically increasing sequence number.
pub struct SessionEventBuffer {
    events: VecDeque<(u64, EventEnvelope)>,
    next_seq: u64,
    capacity: usize,
}

impl SessionEventBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            events: VecDeque::with_capacity(capacity),
            next_seq: 0,
            capacity,
        }
    }

    /// Push an event, assign it a seq, and return the seq.
    pub fn push(&mut self, mut envelope: EventEnvelope) -> u64 {
        let seq = self.next_seq;
        self.next_seq += 1;
        envelope.seq = seq;
        self.events.push_back((seq, envelope));
        if self.events.len() > self.capacity {
            self.events.pop_front();
        }
        seq
    }

    /// Return all events with seq > `last_seq`, or all events if `last_seq`
    /// is `None`. Returns `None` if the requested `last_seq` has already been
    /// evicted from the buffer (reconnection too late).
    pub fn events_since(&self, last_seq: Option<u64>) -> Option<Vec<&EventEnvelope>> {
        match last_seq {
            Some(seq) => {
                if self
                    .events
                    .front()
                    .map_or(true, |(oldest, _)| *oldest > seq)
                {
                    return None;
                }
                Some(
                    self.events
                        .iter()
                        .filter(|(s, _)| *s > seq)
                        .map(|(_, e)| e)
                        .collect(),
                )
            }
            None => Some(self.events.iter().map(|(_, e)| e).collect()),
        }
    }

    /// Return a reference to the envelope assigned `seq`, or `None` if it has
    /// been evicted. Used to serialize the stored (seq-correct) envelope after
    /// [`push`](Self::push) assigned the real seq (finding #1).
    pub fn get(&self, seq: u64) -> Option<&EventEnvelope> {
        self.events
            .iter()
            .find_map(|(s, e)| (*s == seq).then_some(e))
    }
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

/// Async listener signature for `AgentEvent` subscription.
type AgentEventListener = Arc<
    dyn Fn(
            AgentEvent,
            Option<tokio::sync::watch::Receiver<bool>>,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
        + Send
        + Sync,
>;

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
    agent_facade: &AgentFacade,
) -> (
    mpsc::Receiver<TranscriptEvent>,
    mpsc::Receiver<TranscriptEvent>,
) {
    let (delta_tx, delta_rx) = mpsc::channel::<TranscriptEvent>(64);
    let (durable_tx, durable_rx) = mpsc::channel::<TranscriptEvent>(256);

    // AgentEvent (async) → route to appropriate channel based on event type
    let delta_tx1 = delta_tx.clone();
    let durable_tx1 = durable_tx.clone();
    let async_listener: AgentEventListener = Arc::new(move |event, _signal| {
        let delta_tx = delta_tx1.clone();
        let durable_tx = durable_tx1.clone();
        Box::pin(async move {
            // Text/thinking/tool-call deltas → delta channel (drops when full)
            // Everything else → durable channel (awaits capacity)
            let is_delta = matches!(
                &event,
                AgentEvent::MessageUpdate {
                    assistant_message_event,
                    ..
                } if matches!(
                    assistant_message_event,
                    pi_coding_agent::pi_agent_core::pi_ai::types::AssistantMessageEvent::TextDelta { .. }
                    | pi_coding_agent::pi_agent_core::pi_ai::types::AssistantMessageEvent::ThinkingDelta { .. }
                    | pi_coding_agent::pi_agent_core::pi_ai::types::AssistantMessageEvent::ToolCallDelta { .. }
                )
            );
            if is_delta {
                let _ = delta_tx.try_send(TranscriptEvent::Core(Box::new(event)));
            } else {
                let _ = durable_tx
                    .send(TranscriptEvent::Core(Box::new(event)))
                    .await;
            }
        })
    });
    let _h1 = agent_facade.subscribe(async_listener).await;

    // AgentSessionEvent (sync) → durable channel (try_send since sync callback)
    let durable_tx2 = durable_tx.clone();
    let sync_listener: AgentSessionEventListener = Arc::new(move |event: AgentSessionEvent| {
        let payload = convert_session_event(event);
        let _ = durable_tx2.try_send(TranscriptEvent::Session(payload));
    });
    let _h2 = agent_facade.subscribe_session_events(sync_listener);

    (delta_rx, durable_rx)
}

/// Get or create the shared per-session event buffer (§9.1).
///
/// The buffer lives in `AppState` so it survives across connections to the
/// same session: events produced while a client is disconnected are retained
/// and replayed on reconnect.
pub async fn get_session_buffer(
    state: &Arc<AppState>,
    session_id: &str,
) -> Arc<tokio::sync::Mutex<SessionEventBuffer>> {
    let mut buffers = state.session_buffers.lock().await;
    if let Some(buf) = buffers.get(session_id) {
        return buf.clone();
    }
    let buf = Arc::new(tokio::sync::Mutex::new(SessionEventBuffer::new(
        SESSION_BUFFER_CAPACITY,
    )));
    buffers.insert(session_id.to_string(), buf.clone());
    buf
}

/// Assign a real `seq` via the shared buffer, then serialize the envelope.
///
/// This fixes the original ordering bug where the envelope was serialized with
/// `seq: 0` *before* `buffer.push` assigned the real seq (finding #1): the
/// envelope is pushed first (the buffer mutates `seq` in place), then the
/// stored envelope — now carrying the correct seq — is serialized while still
/// holding the buffer lock.
async fn buffer_and_serialize(
    buffer: &Arc<tokio::sync::Mutex<SessionEventBuffer>>,
    event: TranscriptEvent,
) -> Option<String> {
    buffer_and_serialize_value(buffer, event)
        .await
        .and_then(|v| serde_json::to_string(&v).ok())
}

/// Same as [`buffer_and_serialize`] but returns a [`serde_json::Value`].
///
/// Shared by the WebSocket handler (which sends a `String` over the socket)
/// and the Tauri forward task (which emits a JSON object to the frontend).
pub async fn buffer_and_serialize_value(
    buffer: &Arc<tokio::sync::Mutex<SessionEventBuffer>>,
    event: TranscriptEvent,
) -> Option<serde_json::Value> {
    let mut buf = buffer.lock().await;
    let probe = EventEnvelope {
        schema_version: SCHEMA_VERSION,
        seq: 0,
        event,
    };
    let seq = buf.push(probe);
    let stored = buf
        .events
        .iter()
        .find_map(|(s, e)| (*s == seq).then_some(e))?;
    serde_json::to_value(stored).ok()
}

// ── WS handler ────────────────────────────────────────────────────────────────────────────

/// Query parameters for the WS upgrade request.
#[derive(Debug, Deserialize)]
pub struct WsQuery {
    /// The last seq the client received. Omit for first connection.
    pub last_seq: Option<u64>,
}

/// `GET /api/ai/sessions/:id/ws` — WebSocket upgrade.
///
/// The `:id` path parameter is the session ID. Optional `?last_seq=...`
/// enables reconnection replay (§9).
pub async fn ws_handler(
    Path(session_id): Path<String>,
    Query(query): Query<WsQuery>,
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_session(socket, state, session_id, query.last_seq))
}

async fn handle_session(
    mut socket: WebSocket,
    state: Arc<AppState>,
    session_id: String,
    last_seq: Option<u64>,
) {
    use openatrading_core::log;

    // Retrieve the live facade from the shared pool. We do NOT remove it —
    // reconnections must reuse the same AgentFacade to preserve the running
    // agent's context and event stream. If the pool has a pending message
    // (from create_session), take it and clear it so only the first
    // connection sends it.
    let (mut agent_facade, pending_message) = {
        let mut pool = state.active_facades.lock().await;
        if let Some(active) = pool.get_mut(&session_id) {
            let msg = active.pending_message.take();
            (active.facade.clone(), msg)
        } else {
            drop(pool);
            // Not in pool — resume from session file on disk.
            match state
                .ctx
                .runtime
                .create_session(None, None, Some(true), Some(&session_id), None)
                .await
            {
                Ok(h) => (h, None),
                Err(e) => {
                    log::error!("ws: failed to create/resume session {session_id}: {e}");
                    let _ = socket
                        .send(Message::Text(format!(
                            r#"{{"kind":"session","type":"notice","text":"{e}","is_error":true}}"#
                        )))
                        .await;
                    return;
                }
            }
        }
    };

    // Get the shared per-session event buffer (§9.1). It is shared across
    // connections to the same session so events produced while a client is
    // disconnected are retained for replay on reconnect (finding #2).
    let buffer = get_session_buffer(&state, &session_id).await;

    // Replay missed events if reconnecting (§9.1). Collect the serialized
    // JSON while holding the buffer lock, then release the lock before
    // sending so we never hold it across a `.await` point on the socket.
    {
        let buf = buffer.lock().await;
        match buf.events_since(last_seq) {
            Some(events) => {
                let replay: Vec<String> = events
                    .iter()
                    .filter_map(|e| serde_json::to_string(e).ok())
                    .collect();
                drop(buf);
                for json in replay {
                    if socket.send(Message::Text(json)).await.is_err() {
                        return;
                    }
                }
            }
            None => {
                // Buffer has evicted the requested seq — signal resync.
                let resync = serde_json::json!({
                    "schema_version": SCHEMA_VERSION,
                    "seq": null,
                    "kind": "session",
                    "type": "resync_required",
                });
                let _ = socket.send(Message::Text(resync.to_string())).await;
            }
        }
    }

    let (mut delta_rx, mut durable_rx) = setup_event_channel(&agent_facade).await;

    // If there is a pending message from create_session, send it now that
    // event subscribers are in place (prevents losing early events).
    //
    // The send MUST run in a background task. `AgentFacade::send` awaits
    // `add_user_text` → `agent.process()` → the full agent turn, which only
    // completes once the assistant finishes. If we awaited it inline here, the
    // event-forwarding loop below would not run until the turn ended, so:
    //   - text deltas (delta_tx, try_send, cap 64) would overflow and be
    //     silently dropped, and
    //   - durable events (durable_tx, send().await, cap 256) would eventually
    //     deadlock the listener since nothing drains the channel.
    // Spawning lets the loop drain both channels concurrently with the agent.
    if let Some(msg) = pending_message {
        state
            .running_sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(session_id.clone());
        let facade = agent_facade.clone();
        tokio::spawn(async move {
            facade.send(&msg).await;
        });
    }

    log::info!("ws: session {session_id} connected (last_seq={last_seq:?})");

    loop {
        // `biased` ensures durable events (lifecycle, tool results) are
        // drained before delta events when both are ready (§14, finding #8).
        tokio::select! {
            biased;
            // Durable events (tool results, lifecycle, compaction, retry) — must deliver
            Some(event) = durable_rx.recv() => {
                // Track running state (§7.4): agent_end/agent_settled → session is idle.
                if matches!(&event, TranscriptEvent::Session(payload) if matches!(payload, SessionEventPayload::AgentEnd { .. } | SessionEventPayload::AgentSettled)) {
                    // Mutex::lock poisoning is treated as an unrecoverable invariant
                    // violation; `unwrap_or_default` would silently lose state.
                    state.running_sessions.lock().unwrap_or_else(|e| e.into_inner()).remove(&session_id);
                }
                // Push to buffer first to obtain the real seq, then serialize
                // the stored envelope (finding #1).
                let Some(json) = buffer_and_serialize(&buffer, event).await else { continue };
                if socket.send(Message::Text(json)).await.is_err() {
                    break;
                }
            }
            // Delta events (text/thinking/tool-call deltas) — can drop if slow
            Some(event) = delta_rx.recv() => {
                let Some(json) = buffer_and_serialize(&buffer, event).await else { continue };
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
                                    let Some(json) = buffer_and_serialize(&buffer, TranscriptEvent::Session(notice)).await else { continue };
                                    let _ = socket.send(Message::Text(json)).await;
                                    continue;
                                }
                                // Mark session as running (§7.4)
                                state.running_sessions.lock().unwrap_or_else(|e| e.into_inner()).insert(session_id.clone());
                                // Spawn the turn so this select! loop keeps
                                // forwarding events concurrently (see the
                                // pending_message comment above for why we
                                // cannot await send() inline).
                                let facade = agent_facade.clone();
                                tokio::spawn(async move {
                                    facade.send(&text).await;
                                });
                            }
                            Ok(ClientCommand::Interrupt) => {
                                agent_facade.abort().await;
                            }
                            Ok(ClientCommand::Ping) => {
                                // Application-level ping acknowledged; no response needed.
                            }
                            Ok(ClientCommand::PermissionResponse { id, approved }) => {
                                // TODO: Wire up to pi-coding-agent's permission system when available.
                                // See docs/agentTransport.md §11 for the planned implementation.
                                log::warn!("permission response received but not yet wired: id={id}, approved={approved}");
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

    // Session disconnected — remove from running tracking (§7.4).
    // Use `unwrap_or_else(into_inner)` to recover from a poisoned mutex
    // rather than panicking in the handler (finding #15).
    state
        .running_sessions
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&session_id);
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
