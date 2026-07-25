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
use tokio::sync::Mutex;

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
    /// Resync signal for reconnection (§9.1).
    ResyncRequired,
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
}

// ── Client commands ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ClientCommand {
    Prompt { text: String },
    Interrupt,
    Ping,
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
    let _h1 = agent_facade.session.subscribe(async_listener).await;

    // AgentSessionEvent (sync) → durable channel (try_send since sync callback)
    let durable_tx2 = durable_tx.clone();
    let sync_listener: AgentSessionEventListener = Arc::new(move |event: AgentSessionEvent| {
        let payload = convert_session_event(event);
        let _ = durable_tx2.try_send(TranscriptEvent::Session(payload));
    });
    let _h2 = agent_facade.session.subscribe_session_events(sync_listener);

    (delta_rx, durable_rx)
}

/// Set up the event channel and buffer for a session with backpressure (§14).
///
/// Returns the delta receiver, durable receiver, and the buffer. The buffer is
/// used for reconnection replay (§9.1). Both channels feed into the same buffer
/// so reconnection replay preserves event ordering.
pub async fn setup_event_channel_with_buffer(
    agent_facade: &AgentFacade,
) -> (
    mpsc::Receiver<TranscriptEvent>,
    mpsc::Receiver<TranscriptEvent>,
    Arc<tokio::sync::Mutex<SessionEventBuffer>>,
) {
    let (delta_rx, durable_rx) = setup_event_channel(agent_facade).await;
    let buffer = Arc::new(tokio::sync::Mutex::new(SessionEventBuffer::new(500)));
    (delta_rx, durable_rx, buffer)
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

    // Create the per-session event buffer.
    let buffer = Arc::new(Mutex::new(SessionEventBuffer::new(500)));

    // Replay missed events if reconnecting (§9.1).
    {
        let buf = buffer.lock().await;
        match buf.events_since(last_seq) {
            Some(events) => {
                for envelope in events {
                    let Ok(json) = serde_json::to_string(envelope) else {
                        continue;
                    };
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

    log::info!("ws: session {session_id} connected (last_seq={last_seq:?})");

    loop {
        tokio::select! {
            // Delta events (text/thinking/tool-call deltas) — can drop if slow
            Some(event) = delta_rx.recv() => {
                let envelope = EventEnvelope {
                    schema_version: SCHEMA_VERSION,
                    seq: 0,
                    event,
                };
                let json = serde_json::to_string(&envelope);
                let Ok(json) = json else { continue };
                let _seq = buffer.lock().await.push(envelope);
                if socket.send(Message::Text(json)).await.is_err() {
                    break;
                }
            }
            // Durable events (tool results, lifecycle, compaction, retry) — must deliver
            Some(event) = durable_rx.recv() => {
                let envelope = EventEnvelope {
                    schema_version: SCHEMA_VERSION,
                    seq: 0,
                    event,
                };
                let json = serde_json::to_string(&envelope);
                let Ok(json) = json else { continue };
                let _seq = buffer.lock().await.push(envelope);
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
                                        serde_json::to_string(&EventEnvelope {
                                            schema_version: SCHEMA_VERSION,
                                            seq: 0,
                                            event: TranscriptEvent::Session(notice),
                                        })
                                        .unwrap_or_default(),
                                    )).await;
                                    continue;
                                }
                                agent_facade.send(&text).await;
                            }
                            Ok(ClientCommand::Interrupt) => {
                                agent_facade.session.abort().await;
                            }
                            Ok(ClientCommand::Ping) => {
                                // Application-level ping acknowledged; no response needed.
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
