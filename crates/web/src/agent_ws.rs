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
use openatrading_core::utils::attachments::{
    AttachmentService, FileAttachment, ImageAttachment, StoredAttachmentScope,
};
use serde::Deserialize;
use tokio::sync::mpsc;

use crate::AppState;

// Re-export the event protocol for the Tauri layer and other consumers.
pub use openatrading_core::llm::{convert_agent_event, convert_session_event, SessionEventState};

// ── WS handler ────────────────────────────────────────────────────────────

/// 单条 WS 消息（= 单帧）的体积上限：64MB。
///
/// 前端把一条 prompt 的全部附件塞进**一次** `ws.send()`，浏览器不会帮我们
/// 分帧，所以一帧必须装得下整条消息。按后端附件预算（合计 40MB 原始字节）
/// 算：base64 膨胀 4/3 ≈ 53.4MB，再加 JSON 包装与文本，64MB 有安全余量。
///
/// 不设这个值就是个隐藏地雷：`tungstenite` 默认 `max_frame_size` 只有 16MB
/// （`max_message_size` 64MB），超过时连接直接被判 `Capacity` 断开，用户看到
/// 的是「连接已断开」而不是「文件太大」——22MB 以上的附件根本发不出去。
const MAX_WS_MESSAGE_BYTES: usize = 64 << 20;

// 编译期护栏：这两条是常量语义，写错就该编不过。
// - 退回 16MB（tungstenite 默认）会让 20MB 附件直接断连；
// - 低于 64MB 就装不下后端 40MB 附件预算的 base64 膨胀（约 53.4MB）。
const _: () = assert!(
    MAX_WS_MESSAGE_BYTES > (16 << 20),
    "回到 16MB 单帧会让 20MB 附件直接断连"
);
const _: () = assert!(
    MAX_WS_MESSAGE_BYTES >= (64 << 20),
    "单帧上限低于 64MB 装不下 40MB 附件预算"
);

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
    ws.max_frame_size(MAX_WS_MESSAGE_BYTES)
        .max_message_size(MAX_WS_MESSAGE_BYTES)
        // 写缓冲至少要容纳「缓冲区 + 一整条消息」，否则大 prompt 的
        // 回显/事件回放会在 flush 前被写入上限挤断。
        .max_write_buffer_size(MAX_WS_MESSAGE_BYTES.saturating_mul(2))
        .on_upgrade(move |socket| handle_session(socket, state, query.id, query.last_seq))
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
                            Ok(ClientCommand::Prompt { text, images, files }) => {
                                // Session-level slash commands (model/compact/name/session/help)
                                // are executed by the actor; `/skill:` and unknown `/` commands
                                // fall through to pi-rs `prompt()` (skill expansion).
                                if let Some((cmd_name, cmd_args)) =
                                    crate::chat::parse_session_slash_command(&text)
                                {
                                    let handle = handle.clone();
                                    let notice_tx = tx.clone();
                                    let slash_state = Arc::clone(state);
                                    let slash_sid = session_id.to_string();
                                    let slash_worker = tokio::spawn(async move {
                                        let (text, is_error) =
                                            match handle.slash(&cmd_name, &cmd_args).await {
                                                Ok(text) => (text, false),
                                                Err(e) => (format!("[Slash Error] {e}"), true),
                                            };
                                        // Route through the session event state like every
                                        // other notice (seq + ring), so reconnect resume sees
                                        // it and the wire shape matches the desktop path.
                                        emit_notice(
                                            &slash_state,
                                            &notice_tx,
                                            &slash_sid,
                                            text,
                                            is_error,
                                        )
                                        .await;
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
                                let images = images.unwrap_or_default();
                                let files = files.unwrap_or_default();
                                let worker = tokio::spawn(async move {
                                    log::info!("ws: agent prompt received for {sid}");
                                    // Decoding and resizing images is CPU-bound;
                                    // keep it off the async worker threads.
                                    // 落盘附件（`POST /api/attachments` 的 id）按会话目录解析；
                                    // 内联 base64 的老路径不受影响。
                                    let scope = StoredAttachmentScope::new(sid.clone());
                                    let prepared = match tokio::task::spawn_blocking(move || {
                                        // 读盘期间登记为"在途"：这期间用户点清缓存也不会
                                        // 把正在解析的文件删掉。
                                        let _in_flight = crate::in_flight::InFlightGuard::register(
                                            &scope.data_dir.join("attachments"),
                                            &scope.session_id,
                                            stored_attachment_ids(&images, &files),
                                        );
                                        AttachmentService::prepare_prompt(
                                            &text,
                                            &images,
                                            &files,
                                            Some(&scope),
                                        )
                                    })
                                    .await
                                    {
                                        Ok(Ok(prepared)) => prepared,
                                        Ok(Err(e)) => {
                                            log::warn!(
                                                "ws: prompt attachments rejected for {sid}: {e}"
                                            );
                                            emit_notice(&prompt_state, &prompt_tx, &sid, e.to_string(), true)
                                                .await;
                                            return;
                                        }
                                        Err(e) => {
                                            log::error!(
                                                "ws: attachment task panicked for {sid}: {e}"
                                            );
                                            emit_notice(
                                                &prompt_state,
                                                &prompt_tx,
                                                &sid,
                                                "附件处理失败，请重试".to_string(),
                                                true,
                                            )
                                            .await;
                                            return;
                                        }
                                    };
                                    match handle.prompt(&prepared.text, prepared.options).await {
                                        Ok(()) => {
                                            log::info!("ws: agent prompt accepted for {sid}");
                                        }
                                        Err(e) => {
                                            log::warn!("ws: prompt failed for {sid}: {e}");
                                            // Route the error through the session's event state so it
                                            // gets a seq + lands in the ring (recoverable on reconnect).
                                            emit_notice(&prompt_state, &prompt_tx, &sid, e, true)
                                                .await;
                                        }
                                    }
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

/// prompt 里引用到的落盘附件 id（内联字节的没有 id，自然不在其中）。
fn stored_attachment_ids(images: &[ImageAttachment], files: &[FileAttachment]) -> Vec<String> {
    images
        .iter()
        .filter_map(|image| image.id.clone())
        .chain(files.iter().filter_map(|file| file.id.clone()))
        .collect()
}

// ── Client commands ───────────────────────────────────────────────────────

/// Emit a session notice through the event state (seq + ring) and forward it
/// to this socket's event channel.
async fn emit_notice(
    state: &Arc<AppState>,
    tx: &mpsc::Sender<String>,
    session_id: &str,
    text: String,
    is_error: bool,
) {
    let event_state = state.ctx.pool.event_state(session_id).await;
    let json = event_state.emit(TranscriptEvent::Session(SessionEventPayload::Notice {
        text,
        is_error,
    }));
    let _ = tx.send(json).await;
}

// TODO: 协议里声明了 4 个客户端命令，目前只有 `Prompt` 被处理；
// `Interrupt` / `Ping` / `PermissionResponse` 尚未实现（权限应答对应
// 「桌面端 respond_permission 仍是 eprintln 桩」这一缺口）。保留变体是为了
// 让协议形状完整可见，实现时逐个接上。
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)]
enum ClientCommand {
    Prompt {
        text: String,
        /// Wire format mirrors the frontend `PromptImage`: raw base64 `data`
        /// (no `data:image/…;base64,` prefix) plus client metadata. The MIME
        /// type is informational — the service detects the real type.
        #[serde(default)]
        images: Option<Vec<ImageAttachment>>,
        /// Any file (image / PDF / Office document / text), base64 encoded.
        /// The type is detected from the decoded bytes, never the client MIME.
        #[serde(default)]
        files: Option<Vec<FileAttachment>>,
    },
    Interrupt,
    Ping,
    PermissionResponse {
        id: String,
        approved: bool,
    },
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn prompt_json(text: &str, images_json: &str) -> String {
        format!(r#"{{"type":"prompt","text":{text},"images":{images_json}}}"#)
    }

    #[test]
    fn prompt_with_images_and_files_deserializes() {
        let raw = r#"{
            "type":"prompt",
            "text":"look at this chart",
            "images":[{"name":"chart.png","data":"aGVsbG8=","mimeType":"image/png"}],
            "files":[{"name":"notes.md","data":"aGVsbG8=","mimeType":"text/markdown","size":5}]
        }"#;
        let cmd: ClientCommand = serde_json::from_str(raw).expect("parse");
        match cmd {
            ClientCommand::Prompt {
                text,
                images,
                files,
            } => {
                assert_eq!(text, "look at this chart");
                let images = images.expect("images present");
                assert_eq!(images.len(), 1);
                assert_eq!(images[0].name.as_deref(), Some("chart.png"));
                assert_eq!(images[0].data.as_deref(), Some("aGVsbG8="));
                assert_eq!(images[0].mime_type, "image/png");

                let files = files.expect("files present");
                assert_eq!(files.len(), 1);
                assert_eq!(files[0].name, "notes.md");
                assert_eq!(files[0].data.as_deref(), Some("aGVsbG8="));
                assert_eq!(files[0].mime_type.as_deref(), Some("text/markdown"));
                assert_eq!(files[0].size, Some(5));
            }
            other => panic!("expected Prompt, got {other:?}"),
        }
    }

    #[test]
    fn prompt_tolerates_composer_display_metadata() {
        // The composer sends `id`/`size` for local rendering. `id` 现在是落盘
        // 附件的引用字段，但只要 `data` 同时存在就走内联路径（data 优先），
        // 所以老客户端把本地渲染 id 一起发过来不会被打成"附件不存在"。
        // 2c 切到落盘引用时，前端必须换成上传接口返回的真 id。
        let raw = prompt_json(
            r#""x""#,
            r#"[{"id":"abc","name":"a.png","size":10,"data":"aGVsbG8=","mimeType":"image/png"}]"#,
        );
        let cmd: ClientCommand = serde_json::from_str(&raw).expect("parse");
        match cmd {
            ClientCommand::Prompt { images, .. } => {
                let images = images.expect("images present");
                assert_eq!(images.len(), 1);
                assert_eq!(images[0].data.as_deref(), Some("aGVsbG8="));
            }
            other => panic!("expected Prompt, got {other:?}"),
        }
    }

    #[test]
    fn prompt_missing_attachment_fields_is_allowed() {
        let raw = r#"{"type":"prompt","text":"hello"}"#;
        let cmd: ClientCommand = serde_json::from_str(raw).expect("parse");
        match cmd {
            ClientCommand::Prompt {
                text,
                images,
                files,
            } => {
                assert_eq!(text, "hello");
                assert!(images.is_none());
                assert!(files.is_none());
            }
            other => panic!("expected Prompt, got {other:?}"),
        }
    }

    #[test]
    fn prompt_with_image_missing_payload_is_rejected_by_the_service() {
        // `data` 现在可选（改由 `id` 引用落盘附件），所以解构能过；缺载荷必须
        // 在服务层被挡住，而不是悄悄发出一个没有内容的 prompt。
        use openatrading_core::utils::attachments::{AttachmentError, AttachmentService};

        let raw = r#"{"type":"prompt","text":"x","images":[{"mimeType":"image/png"}]}"#;
        let cmd: ClientCommand = serde_json::from_str(raw).expect("data 已可选");
        match cmd {
            ClientCommand::Prompt { images, files, .. } => {
                match AttachmentService::process(
                    "x",
                    &images.unwrap_or_default(),
                    &files.unwrap_or_default(),
                    None,
                ) {
                    Err(AttachmentError::MissingPayload { .. }) => {}
                    other => panic!("应报缺少载荷，得到 {other:?}"),
                }
            }
            other => panic!("expected Prompt, got {other:?}"),
        }
    }

    /// 单帧上限必须装得下「后端附件总预算 × base64 膨胀 + JSON 包装」。
    ///
    /// 这条测试是给未来的人看的：`MAX_TOTAL_BYTES` 一涨，帧上限就得跟着涨，
    /// 否则现象是连接被对端判超限断开（`Reason::Capacity`），而不是一个能看懂
    /// 的错误——这正是这次修掉的 bug。
    #[test]
    fn frame_limit_covers_the_attachment_budget_with_headroom() {
        use openatrading_core::utils::attachments::MAX_TOTAL_BYTES;

        // 4 个 base64 字符编码 3 字节，向上取整。
        let base64_bytes = MAX_TOTAL_BYTES.div_ceil(3) * 4;
        let json_headroom = 4 << 20;

        assert!(
            base64_bytes + json_headroom <= MAX_WS_MESSAGE_BYTES,
            "单帧上限 {MAX_WS_MESSAGE_BYTES} 装不下附件预算 {MAX_TOTAL_BYTES}（base64 {base64_bytes} + 包装 {json_headroom}）"
        );
    }
}
