//! `POST /api/attachments` —— 附件带外上传。
//!
//! 与消息解耦：文件先流式落盘，prompt 里只带 `id`（见
//! `docs/attachments-phase2.md`）。这一层只负责「收字节 + 记账」，不碰 prompt。
//!
//! 三条不变量：
//!
//! 1. **不整体读进内存**：`Multipart` 逐 chunk 写盘，写够就翻脸。
//! 2. **半成品不留痕**：先写 `<id>.part`，校验通过才 `rename` 成 `<id>`。
//! 3. **失败只影响一个文件**：一批里某个超限，其余照收，原因逐个回给客户端。

use std::path::Path;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::multipart::{Multipart, MultipartError};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;

use openatrading_core::log;
use openatrading_core::utils::attachment_store::{self, AttachmentMeta, MAX_UPLOAD_BYTES};

use crate::AppState;

#[derive(Deserialize)]
pub struct UploadQuery {
    /// 目标会话。附件按会话分目录，删除会话即等于清理其附件。
    #[serde(default)]
    pub session: String,
}

/// 上传失败的分类。区分它们只为一件事：状态码要能反映「重试有意义吗」。
#[derive(Debug)]
pub enum UploadError {
    /// 会话 id 不合法（400）。
    InvalidSession(&'static str),
    /// 会话配额已满（413）。
    QuotaExceeded(String),
    /// 请求体不是合法 multipart（400）。
    BadRequest(String),
    /// 磁盘/IO 失败（500）。
    Io(String),
}

impl IntoResponse for UploadError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::InvalidSession(reason) => (StatusCode::BAD_REQUEST, reason.to_string()),
            Self::BadRequest(reason) => (StatusCode::BAD_REQUEST, reason),
            Self::QuotaExceeded(reason) => (StatusCode::PAYLOAD_TOO_LARGE, reason),
            Self::Io(reason) => {
                log::error!("attachments: 上传失败: {reason}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "附件保存失败".to_string(),
                )
            }
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

fn reason_of(e: &MultipartError) -> String {
    format!("multipart 解析失败：{e}")
}

/// 单个 field 的落盘结果。
#[derive(Debug)]
struct StoredUpload {
    meta: AttachmentMeta,
}

/// 把一批 multipart field 收进会话目录。
///
/// 返回 `(已保存, 被拒)`：被拒的条目带原因，整批仍然成功——用户选了 6 个文件，
/// 其中一个超限不该让另外 5 个也白传。
async fn store_uploads(
    data_dir: &Path,
    session_id: &str,
    mut multipart: Multipart,
) -> Result<(Vec<StoredUpload>, Vec<Value>), UploadError> {
    attachment_store::validate_session_id(session_id).map_err(UploadError::InvalidSession)?;

    let dir = attachment_store::session_dir(data_dir, session_id)
        .ok_or(UploadError::InvalidSession("会话 id 不合法"))?;
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| UploadError::Io(format!("创建附件目录失败: {e}")))?;

    let mut usage = attachment_store::session_usage(&dir);
    let mut stored: Vec<StoredUpload> = Vec::new();
    let mut rejected: Vec<Value> = Vec::new();

    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| UploadError::BadRequest(reason_of(&e)))?
    {
        let raw_name = field.file_name().map(str::to_string);
        let mime_type = field.content_type().map(str::to_string).unwrap_or_default();
        let display = attachment_store::sanitize_display_name(raw_name.as_deref().unwrap_or(""));

        // 数量配额先判：省得白收 512MB 再拒。
        if let Some(reason) = attachment_store::quota_error(usage, 0) {
            rejected.push(json!({ "name": display, "error": reason }));
            // 后面的 field 不再读，直接跳出：继续读只是把字节丢掉。
            break;
        }

        let id = attachment_store::new_attachment_id();
        let part = attachment_store::part_path(&dir, &id);
        let final_path = dir.join(&id);
        let mut file = tokio::fs::File::create(&part)
            .await
            .map_err(|e| UploadError::Io(format!("创建临时文件失败: {e}")))?;

        let mut written: u64 = 0;
        let mut failure: Option<String> = None;
        loop {
            let chunk = match field.chunk().await {
                Ok(Some(chunk)) => chunk,
                Ok(None) => break,
                Err(e) => {
                    failure = Some(reason_of(&e));
                    break;
                }
            };
            written = written.saturating_add(chunk.len() as u64);
            if written > MAX_UPLOAD_BYTES {
                failure = Some(format!(
                    "「{display}」超过单文件上限 {}MB",
                    MAX_UPLOAD_BYTES / (1024 * 1024)
                ));
                break;
            }
            // 会话总量边写边判：否则一个 512MB 文件能把 2GB 配额顶穿。
            if let Some(reason) = attachment_store::quota_error(usage, written) {
                failure = Some(reason);
                break;
            }
            if let Err(e) = file.write_all(&chunk).await {
                failure = Some(format!("写入失败: {e}"));
                break;
            }
        }

        // 收尾：无论成败都先把句柄放掉再动文件（Windows 上不能删打开的文件）。
        let flush = file.flush().await;
        drop(file);

        if failure.is_none() {
            if let Err(e) = flush {
                failure = Some(format!("写入失败: {e}"));
            }
        }

        if let Some(reason) = failure {
            let _ = tokio::fs::remove_file(&part).await;
            rejected.push(json!({ "name": display, "error": reason }));
            continue;
        }

        if written == 0 {
            let _ = tokio::fs::remove_file(&part).await;
            rejected.push(json!({ "name": display, "error": "文件为空" }));
            continue;
        }

        // 元信息与 rename 都在 core 里做：size 用**实际写入字节数**，与 Tauri
        // 的字节路径共用同一段实现，两条入口不会各自漂移。
        match attachment_store::finish_stored(
            &dir,
            session_id,
            raw_name.as_deref(),
            &mime_type,
            &id,
            &part,
            written,
        ) {
            Ok(meta) => {
                usage.files = usage.files.saturating_add(1);
                usage.bytes = usage.bytes.saturating_add(written);
                stored.push(StoredUpload { meta });
            }
            Err(e) => {
                let _ = tokio::fs::remove_file(&part).await;
                let _ = tokio::fs::remove_file(&final_path).await;
                rejected.push(json!({
                    "name": display,
                    "error": format!("落盘失败: {e}"),
                }));
            }
        }
    }

    Ok((stored, rejected))
}

/// `POST /api/attachments?session=<id>` —— multipart 上传，返回可引用的 id 列表。
pub async fn upload(
    State(_state): State<Arc<AppState>>,
    Query(query): Query<UploadQuery>,
    multipart: Multipart,
) -> Response {
    let data_dir = openatrading_core::atrading_home();
    match store_uploads(&data_dir, &query.session, multipart).await {
        Ok((stored, rejected)) => {
            let attachments: Vec<Value> = stored
                .iter()
                .map(|s| {
                    json!({
                        "id": s.meta.id,
                        "name": s.meta.name,
                        "size": s.meta.size,
                        "mimeType": s.meta.mime_type,
                        "kind": s.meta.kind,
                    })
                })
                .collect();
            log::info!(
                "attachments: session={} 保存 {} 个，拒绝 {} 个",
                query.session,
                attachments.len(),
                rejected.len()
            );
            Json(json!({ "attachments": attachments, "rejected": rejected })).into_response()
        }
        Err(e) => e.into_response(),
    }
}

/// 孤儿附件的静默期：刚上传还没发消息时，会话文件必然还不存在。
const ORPHAN_GRACE: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);
/// 清扫间隔。
const GC_INTERVAL: std::time::Duration = std::time::Duration::from_secs(6 * 60 * 60);

/// 启动孤儿附件清扫：立刻跑一次，之后每 6 小时一次。
///
/// 进程内 spawn，随 gateway / 桌面进程结束——不额外引入后台服务。
pub fn spawn_gc() {
    tokio::spawn(async move {
        loop {
            let report = tokio::task::spawn_blocking(gc_once).await;
            match report {
                Ok(report) => {
                    if !report.removed.is_empty() {
                        log::info!(
                            "attachments: 清理 {} 个孤儿会话目录（{} 个文件 / {} 字节）",
                            report.removed.len(),
                            report.freed.files,
                            report.freed.bytes
                        );
                    }
                    for error in report.errors {
                        log::warn!("attachments: GC {error}");
                    }
                }
                Err(e) => log::error!("attachments: GC 任务失败: {e}"),
            }
            tokio::time::sleep(GC_INTERVAL).await;
        }
    });
}

/// 跑一轮清扫（同步，便于测试直接调用）。
///
/// 桌面端与 gateway 走的是同一个函数（[`spawn_gc`] → 这里 → 同一个
/// `gc_session_dirs`），所以这里的行为就是两端的真实行为。
pub fn gc_once() -> openatrading_core::utils::attachment_store::GcReport {
    gc_once_with(
        &openatrading_core::atrading_home(),
        &pi_coding_agent::config::get_sessions_dir(),
        openatrading_core::utils::time::beijing_now().timestamp_millis(),
    )
}

/// 与 [`gc_once`] 相同，但目录与时钟由调用方给出（测试用）。
pub fn gc_once_with(
    data_dir: &std::path::Path,
    sessions_dir: &std::path::Path,
    now_ms: i64,
) -> openatrading_core::utils::attachment_store::GcReport {
    openatrading_core::utils::attachment_store::gc_session_dirs(
        data_dir,
        sessions_dir,
        now_ms,
        ORPHAN_GRACE.as_millis() as i64,
    )
}

/// `GET /api/attachments/:id?session=<id>` —— 读回原始字节（图片预览、下载）。
pub async fn download(
    Query(query): Query<UploadQuery>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Response {
    let data_dir = openatrading_core::atrading_home();
    let Some(dir) = attachment_store::session_dir(&data_dir, &query.session) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "会话 id 不合法"})),
        )
            .into_response();
    };
    let Some(meta) = attachment_store::read_meta(&dir, &id) else {
        return (StatusCode::NOT_FOUND, Json(json!({"error": "附件不存在"}))).into_response();
    };
    let path = meta.data_path(&dir);
    match tokio::fs::read(&path).await {
        Ok(bytes) => {
            let content_type = if meta.mime_type.is_empty() {
                "application/octet-stream".to_string()
            } else {
                meta.mime_type.clone()
            };
            (
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, content_type)],
                Body::from(bytes),
            )
                .into_response()
        }
        Err(e) => {
            log::warn!("attachments: 读取 {} 失败: {e}", path.display());
            (StatusCode::NOT_FOUND, Json(json!({"error": "附件已失效"}))).into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::extract::FromRequest;
    use axum::http::Request;
    use openatrading_core::utils::attachment_store::SESSION_QUOTA_BYTES;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("atrading-upload-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("mkdir: {e}"));
        dir
    }

    const BOUNDARY: &str = "----atradingtest";

    async fn multipart_of(fields: &[(&str, &str, &str)]) -> Multipart {
        let mut body = String::new();
        for (name, filename, content) in fields {
            body.push_str(&format!("--{BOUNDARY}\r\n"));
            body.push_str(&format!(
                "Content-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\n"
            ));
            body.push_str("Content-Type: application/pdf\r\n\r\n");
            body.push_str(content);
            body.push_str("\r\n");
        }
        body.push_str(&format!("--{BOUNDARY}--\r\n"));

        let request = Request::builder()
            .method("POST")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={BOUNDARY}"),
            )
            .body(Body::from(body))
            .unwrap_or_else(|e| panic!("request: {e}"));
        Multipart::from_request(request, &())
            .await
            .unwrap_or_else(|e| panic!("multipart: {e}"))
    }

    #[tokio::test]
    async fn stores_each_file_with_meta_and_returns_ids() {
        let root = scratch("ok");
        let multipart = multipart_of(&[
            ("file", "年报.pdf", "%PDF-1.4 hello"),
            ("file", "../逃逸.docx", "PK\x03\x04 data"),
        ])
        .await;

        let (stored, rejected) = store_uploads(&root, "sess-1", multipart)
            .await
            .unwrap_or_else(|e| panic!("upload: {e:?}"));

        assert_eq!(stored.len(), 2);
        assert!(rejected.is_empty(), "{rejected:?}");

        let dir = attachment_store::session_dir(&root, "sess-1").unwrap_or_else(|| panic!("dir"));
        for upload in &stored {
            let meta = attachment_store::read_meta(&dir, &upload.meta.id)
                .unwrap_or_else(|| panic!("meta 应存在"));
            assert_eq!(meta.size, upload.meta.size);
            assert!(meta.data_path(&dir).exists(), "字节文件应存在");
            assert!(
                !attachment_store::part_path(&dir, &upload.meta.id).exists(),
                "不应留下 .part"
            );
        }
        // 路径成分被剥掉，只有展示名保留文件名。
        assert_eq!(stored[1].meta.name, "逃逸.docx");
        assert_eq!(stored[0].meta.kind, attachment_store::AttachmentKind::File);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn empty_file_is_rejected_without_leaving_debris() {
        let root = scratch("empty");
        let multipart = multipart_of(&[("file", "empty.pdf", "")]).await;

        let (stored, rejected) = store_uploads(&root, "sess-1", multipart)
            .await
            .unwrap_or_else(|e| panic!("upload: {e:?}"));

        assert!(stored.is_empty());
        assert_eq!(rejected.len(), 1);
        assert_eq!(rejected[0]["error"], "文件为空");

        let dir = attachment_store::session_dir(&root, "sess-1").unwrap_or_else(|| panic!("dir"));
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("readdir: {e}"))
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert!(leftovers.is_empty(), "不应留下任何文件：{leftovers:?}");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn invalid_session_is_rejected_before_touching_disk() {
        let root = scratch("bad-session");
        let multipart = multipart_of(&[("file", "a.pdf", "x")]).await;

        match store_uploads(&root, "../escape", multipart).await {
            Err(UploadError::InvalidSession(_)) => {}
            other => panic!("非法会话 id 必须被拒，得到：{other:?}"),
        }
        assert!(!root.join("escape").exists(), "不能写出目录");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn quota_rejects_further_files_but_keeps_the_accepted_ones() {
        let root = scratch("quota");
        // 先把会话配额填到只剩 5 字节。
        let dir = attachment_store::session_dir(&root, "sess-1").unwrap_or_else(|| panic!("dir"));
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("mkdir: {e}"));
        // 用 set_len 造稀疏文件：逻辑尺寸 2GB，不真占内存/磁盘。
        let filler =
            std::fs::File::create(dir.join("filler")).unwrap_or_else(|e| panic!("create: {e}"));
        filler
            .set_len(SESSION_QUOTA_BYTES - 5)
            .unwrap_or_else(|e| panic!("set_len: {e}"));

        let multipart = multipart_of(&[
            ("file", "small.pdf", "abc"),
            ("file", "big.pdf", "0123456789"),
        ])
        .await;

        let (stored, rejected) = store_uploads(&root, "sess-1", multipart)
            .await
            .unwrap_or_else(|e| panic!("upload: {e:?}"));

        assert_eq!(stored.len(), 1, "3 字节的能进");
        assert_eq!(stored[0].meta.size, 3);
        assert_eq!(rejected.len(), 1, "10 字节的应被配额拦下");
        assert!(
            rejected[0]["error"]
                .as_str()
                .unwrap_or_default()
                .contains("总量"),
            "原因应是总量配额：{:?}",
            rejected[0]
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn gc_once_uses_the_shared_grace_period_and_keeps_live_sessions() {
        let root = scratch("gc-once");
        let sessions = root.join("sessions");
        std::fs::create_dir_all(&sessions).unwrap_or_else(|e| panic!("mkdir: {e}"));
        // 一个活会话（两种文件名布局都放一份）
        std::fs::write(sessions.join("2026-01-01T00-00-00_live.jsonl"), b"{}")
            .unwrap_or_else(|e| panic!("write: {e}"));

        let make = |session: &str| {
            let dir =
                attachment_store::session_dir(&root, session).unwrap_or_else(|| panic!("dir"));
            std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("mkdir: {e}"));
            std::fs::write(dir.join("id"), b"x").unwrap_or_else(|e| panic!("write: {e}"));
            dir
        };
        let live = make("live");
        let fresh_orphan = make("orphan-fresh");
        let old_orphan = make("orphan-old");

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or_default();

        // 刚进来：都在宽限期内，一个都不删
        let early = gc_once_with(&root, &sessions, now);
        assert!(early.removed.is_empty(), "{:?}", early.removed);
        assert!(fresh_orphan.is_dir() && old_orphan.is_dir());

        // 时间前进到宽限期之后：只删孤儿，活会话不受影响
        let later = now + ORPHAN_GRACE.as_millis() as i64 + 60_000;
        let report = gc_once_with(&root, &sessions, later);
        assert_eq!(
            report.removed.len(),
            2,
            "两个孤儿都该被清：{:?}",
            report.removed
        );
        assert!(live.is_dir(), "活会话的附件不能动");
        assert!(!fresh_orphan.exists() && !old_orphan.exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn reading_back_a_deleted_attachment_reports_missing() {
        let root = scratch("readback");
        let multipart = multipart_of(&[("file", "a.pdf", "payload")]).await;
        let (stored, _) = store_uploads(&root, "sess-1", multipart)
            .await
            .unwrap_or_else(|e| panic!("upload: {e:?}"));

        let dir = attachment_store::session_dir(&root, "sess-1").unwrap_or_else(|| panic!("dir"));
        let id = stored[0].meta.id.clone();
        assert!(attachment_store::read_meta(&dir, &id).is_some());

        std::fs::remove_file(stored[0].meta.data_path(&dir))
            .unwrap_or_else(|e| panic!("remove: {e}"));
        // 元信息还在，但字节没了 —— 读取方必须能区分"没有"与"坏了"。
        assert!(attachment_store::read_meta(&dir, &id).is_some());
        assert!(!stored[0].meta.data_path(&dir).exists());

        let _ = std::fs::remove_dir_all(&root);
    }
}
