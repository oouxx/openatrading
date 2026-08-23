//! WebSocket transport for agent session events.
//!
//! The event protocol (envelope, `seq` + ring, payload types) lives in
//! `openatrading_agent::events`; the per-session actor serializes and numbers
//! every event and fans it out to subscriber channels. This module only
//! pumps already-serialized envelopes between the session handle and the
//! socket, and forwards client commands (prompt / interrupt / slash) as actor
//! messages.

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use openatrading_core::llm::{EventEnvelope, SessionEventPayload, TranscriptEvent, SCHEMA_VERSION};
use pi_coding_agent::core::agent_session::PromptOptions;
use pi_coding_agent::pi_ai_types::ContentBlock;
use serde::Deserialize;
use tokio::sync::mpsc;

use crate::AppState;

// Re-export the event protocol for the Tauri layer and other consumers.
pub use openatrading_core::llm::{convert_agent_event, convert_session_event, SessionEventState};

// ── WS handler ────────────────────────────────────────────────────────────

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

    // The event_state persists across reconnects for this session (owned by
    // the pool, survives actor eviction), so `seq` and the ring buffer
    // survive a WS reconnect.
    let event_state = state.ctx.pool.event_state(&session_id).await;

    let session =
        match crate::chat_session::ensure_session_with_events(&state.ctx.pool, &session_id).await {
            Ok(s) => s,
            Err(e) => {
                // No pool-bound session exists yet; send an out-of-band notice with
                // seq=0 (client dedup skips seq=0) and bail.
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
    let handle = session.handle;
    log::info!("ws: session {session_id} connected — entering event loop");

    forward_events(&mut socket, &mut rx, &state, &session_id, tx, handle).await;

    log::info!("ws: session {session_id} disconnected");
}

/// Forward events from the session's subscriber channel to the WebSocket, and
/// handle client messages (prompts, pings, close).
async fn forward_events(
    socket: &mut WebSocket,
    rx: &mut mpsc::Receiver<String>,
    state: &Arc<AppState>,
    session_id: &str,
    tx: mpsc::Sender<String>,
    handle: openatrading_core::llm::SessionHandle,
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
                            Ok(ClientCommand::Prompt { text, images }) => {
                                // Session-level slash commands (model/compact/name/session/help)
                                // are executed by the actor; `/skill:` and unknown `/` commands
                                // fall through to pi-rs `prompt()` (skill expansion).
                                if let Some((cmd_name, cmd_args)) =
                                    crate::chat::parse_session_slash_command(&text)
                                {
                                    let handle = handle.clone();
                                    let notice_tx = tx.clone();
                                    let slash_worker = tokio::spawn(async move {
                                        let result = handle.slash(&cmd_name, &cmd_args).await;
                                        let payload = match result {
                                            Ok(text) => SessionEventPayload::Notice {
                                                text,
                                                is_error: false,
                                            },
                                            Err(e) => SessionEventPayload::Notice {
                                                text: format!("[Slash Error] {e}"),
                                                is_error: true,
                                            },
                                        };
                                        let json = serde_json::to_string(&EventEnvelope {
                                            schema_version: SCHEMA_VERSION,
                                            seq: 0,
                                            event: TranscriptEvent::Session(payload),
                                        })
                                        .unwrap_or_default();
                                        let _ = notice_tx.send(json).await;
                                    });
                                    // Observe the spawned task so a panic is never silently
                                    // swallowed (project rule: no fire-and-forget JoinHandles).
                                    let watch_sid = session_id.to_string();
                                    tokio::spawn(async move {
                                        if let Err(e) = slash_worker.await {
                                            log::error!(
                                                "ws: slash task panicked for {watch_sid}: {e}"
                                            );
                                        }
                                    });
                                    continue;
                                }
                                let handle = handle.clone();
                                let sid = session_id.to_string();
                                let prompt_tx = tx.clone();
                                let prompt_state = state.clone();
                                // Multimodal: convert attached images to pi-rs
                                // `ContentBlock::Image` and carry them through
                                // `PromptOptions.images` (pi-rs already supports
                                // images — we just thread them end-to-end).
                                let image_blocks: Vec<ContentBlock> = images
                                    .unwrap_or_default()
                                    .iter()
                                    .map(ClientImage::to_content_block)
                                    .collect();
                                let worker = tokio::spawn(async move {
                                    log::info!("ws: agent turn started for {sid}");
                                    // The actor serializes turns (pi-acp turn queue), so a
                                    // concurrent prompt is queued rather than raced.
                                    let opts = PromptOptions {
                                        streaming_behavior: Some("followUp".to_string()),
                                        images: (!image_blocks.is_empty()).then_some(image_blocks),
                                        ..Default::default()
                                    };
                                    if let Err(e) = handle.prompt(&text, opts).await {
                                        log::warn!("ws: prompt failed for {sid}: {e}");
                                        // Route the error through the session's event state so it
                                        // gets a seq + lands in the ring (recoverable on reconnect).
                                        let event_state =
                                            prompt_state.ctx.pool.event_state(&sid).await;
                                        let json = event_state.emit(TranscriptEvent::Session(
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
                                // The actor aborts its own in-flight turn; no shared lock
                                // involved, so an interrupt can never deadlock.
                                state.ctx.pool.signal_abort(session_id).await;
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

// ── Client commands ───────────────────────────────────────────────────────

/// An image attachment sent with a prompt (wire format mirrors the frontend
/// `PromptImage`): raw base64 `data` (no `data:image/…;base64,` prefix) and a
/// `mimeType`. Converted to a pi-rs `ContentBlock::Image` before the turn.
#[derive(Debug, Clone, Deserialize)]
struct ClientImage {
    data: String,
    #[serde(rename = "mimeType")]
    mime_type: String,
}

impl ClientImage {
    fn to_content_block(&self) -> ContentBlock {
        ContentBlock::Image {
            data: self.data.clone(),
            mime_type: self.mime_type.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)]
enum ClientCommand {
    Prompt {
        text: String,
        images: Option<Vec<ClientImage>>,
    },
    Interrupt,
    Ping,
    PermissionResponse {
        id: String,
        approved: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prompt_json(text: &str, images_json: &str) -> String {
        format!(r#"{{"type":"prompt","text":{text},"images":{images_json}}}"#)
    }

    #[test]
    fn prompt_with_images_deserializes_and_converts() {
        let raw = prompt_json(
            r#""look at this chart""#,
            r#"[{"data":"aGVsbG8=","mimeType":"image/png"}]"#,
        );
        let cmd: ClientCommand = serde_json::from_str(&raw).expect("parse");
        match cmd {
            ClientCommand::Prompt { text, images } => {
                assert_eq!(text, "look at this chart");
                let imgs = images.expect("images present");
                assert_eq!(imgs.len(), 1);
                match imgs[0].to_content_block() {
                    ContentBlock::Image { data, mime_type } => {
                        assert_eq!(data, "aGVsbG8=");
                        assert_eq!(mime_type, "image/png");
                    }
                    other => panic!("expected Image block, got {other:?}"),
                }
            }
            other => panic!("expected Prompt, got {other:?}"),
        }
    }

    #[test]
    fn prompt_without_images_field_is_backward_compatible() {
        let raw = prompt_json(r#""hello""#, "[]");
        let cmd: ClientCommand = serde_json::from_str(&raw).expect("parse match");
        match cmd {
            ClientCommand::Prompt { text, images } => {
                assert_eq!(text, "hello");
                // Empty array → None (no images threaded into PromptOptions).
                assert_eq!(images.map(|v| v.len()), Some(0));
            }
            other => panic!("expected Prompt, got {other:?}"),
        }
    }

    #[test]
    fn prompt_missing_images_field_is_allowed() {
        let raw = r#"{"type":"prompt","text":"hello"}"#;
        let cmd: ClientCommand = serde_json::from_str(raw).expect("parse match");
        match cmd {
            ClientCommand::Prompt { text, images } => {
                assert_eq!(text, "hello");
                assert!(images.is_none());
            }
            other => panic!("expected Prompt, got {other:?}"),
        }
    }

    #[test]
    fn prompt_multiple_images_converts_all_in_order() {
        let raw = prompt_json(
            r#""two images""#,
            r#"[
                {"data":"AAA=","mimeType":"image/jpeg"},
                {"data":"BBB=","mimeType":"image/webp"}
            ]"#,
        );
        let cmd: ClientCommand = serde_json::from_str(&raw).expect("parse match");
        match cmd {
            ClientCommand::Prompt { images, .. } => {
                let imgs = images.expect("images present");
                assert_eq!(imgs.len(), 2);
                let blocks: Vec<_> = imgs.iter().map(ClientImage::to_content_block).collect();
                for (i, b) in blocks.iter().enumerate() {
                    match b {
                        ContentBlock::Image { mime_type, .. } => {
                            let expect = if i == 0 { "image/jpeg" } else { "image/webp" };
                            assert_eq!(mime_type, expect);
                        }
                        other => panic!("expected Image block, got {other:?}"),
                    }
                }
            }
            other => panic!("expected Prompt, got {other:?}"),
        }
    }

    #[test]
    fn prompt_with_malformed_image_fails_deserialization() {
        // Missing required `mimeType` field → error propagates to the client.
        let raw = r#"{"type":"prompt","text":"x","images":[{"data":"AAA="}]}"#;
        assert!(serde_json::from_str::<ClientCommand>(raw).is_err());
    }
}
