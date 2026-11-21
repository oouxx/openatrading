//! `POST /api/attachments` —— 浏览器没有路径，只能把字节交给服务器。
//!
//! 桌面端不需要这个接口（拖拽/选择器都带绝对路径，直接进 prompt）。Web 端
//! 落盘后返回**绝对路径**，prompt 里引用该路径，由 pi-rs 的 `read` 工具按需
//! 读取；服务端不做任何解析，所以没有"文本上限 / 截断"这类东西。
//!
//! 落盘发生在**发送时**，因此不存在"上传了却没被引用"的孤儿目录。

use std::path::Path;
use std::sync::Arc;

use axum::extract::multipart::{Multipart, MultipartError};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;

use openatrading_core::log;
use openatrading_core::utils::attachment_store;

use crate::AppState;

/// 单次请求的接收上限：只为挡住拖错几十 GB 镜像这类意外。真正的约束是磁盘，
/// `read` 工具本身能处理任意大小的文件。
const MAX_UPLOAD_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Deserialize)]
pub struct UploadQuery {
    /// 目标会话：附件按会话分目录，删会话即等于清理其附件。
    #[serde(default)]
    pub session: String,
}

/// 上传失败的分类，只用来决定状态码。
#[derive(Debug)]
enum UploadError {
    InvalidSession(&'static str),
    BadRequest(String),
    TooLarge,
    Io(String),
}

impl IntoResponse for UploadError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::InvalidSession(reason) => (StatusCode::BAD_REQUEST, reason.to_string()),
            Self::BadRequest(reason) => (StatusCode::BAD_REQUEST, reason),
            Self::TooLarge => (
                StatusCode::PAYLOAD_TOO_LARGE,
                "文件过大，无法接收".to_string(),
            ),
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

/// 收下所有 field，逐个落盘，返回 `{path, name, size, mimeType}`。
async fn store_uploads(
    data_dir: &Path,
    session_id: &str,
    mut multipart: Multipart,
) -> Result<Vec<Value>, UploadError> {
    attachment_store::validate_session_id(session_id).map_err(UploadError::InvalidSession)?;
    let dir = attachment_store::session_dir(data_dir, session_id)
        .ok_or(UploadError::InvalidSession("会话 id 不合法"))?;
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| UploadError::Io(format!("创建附件目录失败: {e}")))?;

    let mut stored = Vec::new();
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| UploadError::BadRequest(reason_of(&e)))?
    {
        let raw_name = field.file_name().unwrap_or("attachment").to_string();
        let safe = attachment_store::safe_file_name(&raw_name);
        let id = uuid::Uuid::new_v4();
        let part = dir.join(format!("{id}.part"));
        let final_path = dir.join(format!("{id}-{safe}"));

        let written = write_field(&mut field, &part).await?;
        if written == 0 {
            let _ = tokio::fs::remove_file(&part).await;
            continue;
        }
        if let Err(e) = tokio::fs::rename(&part, &final_path).await {
            let _ = tokio::fs::remove_file(&part).await;
            return Err(UploadError::Io(format!("落盘失败: {e}")));
        }

        stored.push(json!({
            "path": final_path.to_string_lossy(),
            "name": raw_name,
            "size": written,
            "mimeType": attachment_store::guess_mime_from_name(&safe),
        }));
    }

    Ok(stored)
}

/// 边收边写，返回实际写入的字节数；失败时不留 `.part`。
async fn write_field(
    field: &mut axum::extract::multipart::Field<'_>,
    part: &Path,
) -> Result<u64, UploadError> {
    let mut file = tokio::fs::File::create(part)
        .await
        .map_err(|e| UploadError::Io(format!("创建临时文件失败: {e}")))?;
    let mut written: u64 = 0;
    let mut failure: Option<UploadError> = None;

    loop {
        let chunk = match field.chunk().await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => break,
            Err(e) => {
                failure = Some(UploadError::BadRequest(reason_of(&e)));
                break;
            }
        };
        written = written.saturating_add(chunk.len() as u64);
        if written > MAX_UPLOAD_BYTES {
            failure = Some(UploadError::TooLarge);
            break;
        }
        if let Err(e) = file.write_all(&chunk).await {
            failure = Some(UploadError::Io(format!("写入失败: {e}")));
            break;
        }
    }

    let flush = file.flush().await;
    drop(file);

    if let Some(error) = failure {
        let _ = tokio::fs::remove_file(part).await;
        return Err(error);
    }
    flush.map_err(|e| {
        // 交给调用方统一处理 `.part` 清理需要路径，这里只报错。
        let _ = std::fs::remove_file(part);
        UploadError::Io(format!("写入失败: {e}"))
    })?;
    Ok(written)
}

/// `POST /api/attachments?session=<id>` —— multipart 落盘，返回绝对路径。
pub async fn upload(
    State(_state): State<Arc<AppState>>,
    Query(query): Query<UploadQuery>,
    multipart: Multipart,
) -> Response {
    let data_dir = openatrading_core::atrading_home();
    match store_uploads(&data_dir, &query.session, multipart).await {
        Ok(attachments) => {
            log::info!(
                "attachments: session={} 保存 {} 个",
                query.session,
                attachments.len()
            );
            Json(json!({ "attachments": attachments })).into_response()
        }
        Err(e) => e.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::extract::FromRequest;
    use axum::http::Request;

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
                "Content-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\n\r\n"
            ));
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
    async fn uploads_return_absolute_paths_on_disk() {
        let root = scratch("ok");
        let multipart = multipart_of(&[
            ("file", "季报.pdf", "%PDF-1.4 hello"),
            ("file", "../../escape.md", "# hi"),
        ])
        .await;

        let stored = store_uploads(&root, "sess-1", multipart)
            .await
            .unwrap_or_else(|e| panic!("upload: {e:?}"));
        assert_eq!(stored.len(), 2);

        for entry in &stored {
            let path = entry["path"].as_str().unwrap_or_default();
            assert!(
                path.starts_with(&attachment_store::attachments_root(&root).to_string_lossy()[..]),
                "必须是附件根下的绝对路径：{path}"
            );
            assert!(Path::new(path).is_file(), "文件应真实存在：{path}");
            assert!(!path.contains(".."), "路径成分必须被剥掉：{path}");
        }
        assert_eq!(stored[0]["size"], 14);
        assert_eq!(stored[1]["name"], "../../escape.md", "展示名保留原样");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn invalid_session_never_touches_disk() {
        let root = scratch("bad");
        let multipart = multipart_of(&[("file", "a.pdf", "x")]).await;

        let result = store_uploads(&root, "../escape", multipart).await;
        assert!(matches!(result, Err(UploadError::InvalidSession(_))));
        assert!(!root.join("escape").exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn empty_fields_leave_nothing_behind() {
        let root = scratch("empty");
        let multipart = multipart_of(&[("file", "empty.txt", "")]).await;

        let stored = store_uploads(&root, "sess-1", multipart)
            .await
            .unwrap_or_else(|e| panic!("upload: {e:?}"));
        assert!(stored.is_empty());

        let dir =
            attachment_store::session_dir(&root, "sess-1").unwrap_or_else(|| panic!("session dir"));
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("readdir: {e}"))
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert!(leftovers.is_empty(), "不应留下任何文件：{leftovers:?}");

        let _ = std::fs::remove_dir_all(&root);
    }
}
