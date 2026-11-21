//! 附件落盘存储（Phase 2a）：目录布局、会话配额、元信息读写。
//!
//! 落盘即事实源：原始字节存 `<data_dir>/attachments/<session_id>/<id>`，元信息
//! 旁挂同名 `.json`。不建表是刻意的——附件生命周期比会话短，`rm -rf <session>`
//! 就等于彻底清理，能和「设置 → 缓存」的语义对齐，也避免给 `data.db` 加迁移。
//!
//! 这一层只做同步小操作（建目录、配额判定、元信息）；**字节流由调用方写**
//! （web 层的 multipart handler 边收边写），所以这里不引入 async。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::temp_files::ATTACHMENTS_DIR_NAME;

/// 单个文件上限：对齐 ChatGPT 文档口径（512MB）。
pub const MAX_UPLOAD_BYTES: u64 = 512 * 1024 * 1024;
/// 单会话附件总配额。
pub const SESSION_QUOTA_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// 单会话附件数配额。
pub const SESSION_QUOTA_FILES: u64 = 50;
/// 会话 id 的最大长度（同时用作目录名，必须收窄）。
const MAX_SESSION_ID_LEN: usize = 128;
/// 元信息里保存的原始文件名上限（只影响展示，不影响落盘路径）。
const MAX_DISPLAY_NAME_CHARS: usize = 200;
/// 半成品后缀：落完后 rename 去掉，见 [`part_path`]。
pub const PART_SUFFIX: &str = ".part";

/// 附件根目录（即「设置 → 缓存」里统计的那个 `attachments/`）。
#[must_use]
pub fn attachments_root(data_dir: &Path) -> PathBuf {
    data_dir.join(ATTACHMENTS_DIR_NAME)
}

/// 会话子目录。`session_id` 必须能当目录名用，否则返回 `None`。
#[must_use]
pub fn session_dir(data_dir: &Path, session_id: &str) -> Option<PathBuf> {
    validate_session_id(session_id)
        .ok()
        .map(|()| attachments_root(data_dir).join(session_id))
}

/// 半成品文件路径。
#[must_use]
pub fn part_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}{PART_SUFFIX}"))
}

/// 会话 id 只允许 `[A-Za-z0-9_-]`，且非空、不超长。
///
/// 直接把它当目录名用，所以白名单比黑名单（过滤 `..`、`/`）安全：未来有人
/// 换了 URL 编码或大小写归一，也不会突然能写到别处去。
///
/// # Errors
/// 非法字符、空串、超长时返回原因。
pub fn validate_session_id(session_id: &str) -> Result<(), &'static str> {
    if session_id.is_empty() {
        return Err("会话 id 不能为空");
    }
    if session_id.len() > MAX_SESSION_ID_LEN {
        return Err("会话 id 过长");
    }
    if !session_id
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err("会话 id 只能包含字母、数字、- 和 _");
    }
    Ok(())
}

/// 新附件 id（UUID v4）。落盘文件名 = 元信息里的 `id`。
#[must_use]
pub fn new_attachment_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// 附件类型：图片走 `ContentBlock::Image`，其余提取文本。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttachmentKind {
    Image,
    File,
}

impl AttachmentKind {
    /// 按 MIME 前缀粗判（真正的类型在 prompt 时按字节重判，这里只给 UI 用）。
    #[must_use]
    pub fn from_mime(mime: &str) -> Self {
        if mime.trim().to_ascii_lowercase().starts_with("image/") {
            Self::Image
        } else {
            Self::File
        }
    }
}

/// 落盘附件的元信息（旁挂 `<id>.json`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentMeta {
    pub id: String,
    pub session_id: String,
    /// 展示用文件名。**不参与路径拼接**，所以只需去控制字符与限长。
    pub name: String,
    pub size: u64,
    #[serde(default)]
    pub mime_type: String,
    pub kind: AttachmentKind,
    /// Unix 毫秒。
    pub created_at: i64,
}

impl AttachmentMeta {
    /// 用客户端给的文件名与 MIME 生成元信息，外加服务端权威的 `size`。
    ///
    /// `size` 由调用方按**实际写入字节数**填，不用客户端声明值：与
    /// `attachments.rs` 里「客户端 size 只是元信息」的既有约定一致。
    #[must_use]
    pub fn new(
        id: String,
        session_id: &str,
        raw_name: Option<&str>,
        mime_type: &str,
        size: u64,
        created_at: i64,
    ) -> Self {
        let name = sanitize_display_name(raw_name.unwrap_or_default());
        Self {
            id,
            session_id: session_id.to_string(),
            name,
            size,
            mime_type: mime_type.trim().to_string(),
            kind: AttachmentKind::from_mime(mime_type),
            created_at,
        }
    }

    /// 原始字节的路径（调用方保证 `dir` 来自 [`session_dir`]）。
    #[must_use]
    pub fn data_path(&self, dir: &Path) -> PathBuf {
        dir.join(&self.id)
    }

    /// 元信息路径。
    #[must_use]
    pub fn meta_path(&self, dir: &Path) -> PathBuf {
        dir.join(format!("{}.json", self.id))
    }
}

/// 把已写好的 `.part` 收尾成正式附件：rename + 写元信息。
///
/// web 的 multipart 流式路径先落 `.part`（避免半成品被当成完整附件），再由
/// 这里收尾；Tauri 的字节路径直接用 [`store_bytes`]。两条入口共用这一段，
/// 保证"元信息里的 size 一定是实际字节数"这条约定只有一处实现。
///
/// # Errors
/// rename 或元信息写入失败时返回原始错误；调用方负责清掉 `.part`。
pub fn finish_stored(
    dir: &Path,
    session_id: &str,
    raw_name: Option<&str>,
    mime_type: &str,
    id: &str,
    part: &Path,
    size: u64,
) -> io::Result<AttachmentMeta> {
    let meta = AttachmentMeta::new(
        id.to_string(),
        session_id,
        raw_name,
        mime_type,
        size,
        crate::utils::time::beijing_now().timestamp_millis(),
    );
    let final_path = meta.data_path(dir);
    fs::rename(part, &final_path)?;
    if let Err(e) = write_meta(dir, &meta) {
        // 元信息写不出来 ≈ 这个附件不可用，回滚字节文件。
        let _ = fs::remove_file(&final_path);
        return Err(e);
    }
    Ok(meta)
}

/// 直接从一个内存块落盘（Tauri 路径：字节经 IPC 过来，不经过 multipart）。
///
/// 配额与上限检查在这里重做一遍，不能依赖调用方：桌面端的调用链和 web 不是
/// 同一段代码，少一处校验就是少一道闸。
///
/// # Errors
/// 会话 id 非法、超限、配额满、IO 失败时返回 [`StoreError`]。
pub fn store_bytes(
    data_dir: &Path,
    session_id: &str,
    raw_name: Option<&str>,
    mime_type: &str,
    bytes: &[u8],
) -> Result<AttachmentMeta, StoreError> {
    validate_session_id(session_id).map_err(StoreError::InvalidSession)?;
    if bytes.is_empty() {
        return Err(StoreError::Empty);
    }
    let size = bytes.len() as u64;
    if size > MAX_UPLOAD_BYTES {
        return Err(StoreError::TooLarge {
            max_mb: MAX_UPLOAD_BYTES / (1024 * 1024),
        });
    }
    let dir =
        session_dir(data_dir, session_id).ok_or(StoreError::InvalidSession("会话 id 不合法"))?;
    fs::create_dir_all(&dir).map_err(|e| StoreError::Io(e.to_string()))?;
    let usage = session_usage(&dir);
    if let Some(reason) = quota_error(usage, size) {
        return Err(StoreError::Quota(reason));
    }

    let id = new_attachment_id();
    let part = part_path(&dir, &id);
    fs::write(&part, bytes).map_err(|e| StoreError::Io(e.to_string()))?;
    finish_stored(&dir, session_id, raw_name, mime_type, &id, &part, size).map_err(|e| {
        let _ = fs::remove_file(&part);
        StoreError::Io(e.to_string())
    })
}

/// 按扩展名粗判 MIME：只影响 UI 上的分类与元信息，**prompt 时仍按字节重判**。
#[must_use]
pub fn guess_mime_from_name(name: &str) -> &'static str {
    let ext = name
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "pdf" => "application/pdf",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "md" | "markdown" => "text/markdown",
        "txt" | "log" => "text/plain",
        "csv" => "text/csv",
        "json" => "application/json",
        _ => "application/octet-stream",
    }
}

/// 直接从磁盘路径落盘（桌面端拖拽用）：**流式复制**，不把文件读进内存。
///
/// 默认 IPC 上传要把文件读成 base64 再传一遍，300MB 的文代会占几百 MB 内存。
/// 桌面端本来就能直接读同一个文件系统，按路径复制一遍才是正确解法。
///
/// # Errors
/// 会话 id 非法、文件读取失败、超限、配额满时返回 [`StoreError`]。
pub fn store_from_path(
    data_dir: &Path,
    session_id: &str,
    path: &Path,
) -> Result<AttachmentMeta, StoreError> {
    validate_session_id(session_id).map_err(StoreError::InvalidSession)?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("attachment");
    let source = fs::File::open(path).map_err(|e| StoreError::Io(e.to_string()))?;
    let size = source.metadata().map(|m| m.len()).unwrap_or(0);
    if size == 0 {
        return Err(StoreError::Empty);
    }
    if size > MAX_UPLOAD_BYTES {
        return Err(StoreError::TooLarge {
            max_mb: MAX_UPLOAD_BYTES / (1024 * 1024),
        });
    }
    let dir =
        session_dir(data_dir, session_id).ok_or(StoreError::InvalidSession("会话 id 不合法"))?;
    fs::create_dir_all(&dir).map_err(|e| StoreError::Io(e.to_string()))?;
    if let Some(reason) = quota_error(session_usage(&dir), size) {
        return Err(StoreError::Quota(reason));
    }

    let id = new_attachment_id();
    let part = part_path(&dir, &id);
    use std::io::Read as _;

    let reader = std::io::BufReader::new(source);
    let mut writer = fs::File::create(&part).map_err(|e| StoreError::Io(e.to_string()))?;
    // `take` 是兜底：文件在 stat 之后变大时也不能写超上限。
    let copied = std::io::copy(
        &mut reader.take(MAX_UPLOAD_BYTES.saturating_add(1)),
        &mut writer,
    )
    .map_err(|e| {
        let _ = fs::remove_file(&part);
        StoreError::Io(e.to_string())
    })?;
    drop(writer);
    if copied > MAX_UPLOAD_BYTES {
        let _ = fs::remove_file(&part);
        return Err(StoreError::TooLarge {
            max_mb: MAX_UPLOAD_BYTES / (1024 * 1024),
        });
    }
    finish_stored(
        &dir,
        session_id,
        Some(name),
        guess_mime_from_name(name),
        &id,
        &part,
        copied,
    )
    .map_err(|e| {
        let _ = fs::remove_file(&part);
        StoreError::Io(e.to_string())
    })
}

/// [`store_bytes`] 的错误分类，供调用方映射状态码。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    InvalidSession(&'static str),
    Empty,
    TooLarge { max_mb: u64 },
    Quota(String),
    Io(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSession(reason) => write!(f, "{reason}"),
            Self::Empty => write!(f, "文件为空"),
            Self::TooLarge { max_mb } => write!(f, "超过单文件上限 {max_mb}MB"),
            Self::Quota(reason) => write!(f, "{reason}"),
            Self::Io(reason) => write!(f, "保存失败：{reason}"),
        }
    }
}

/// 展示名清洗：去掉目录成分与控制字符，压掉多余空白，限长。
///
/// 返回值只进 JSON，不进路径——路径一律用 `id`。所以这里的目标是「看起来
/// 正常」而不是「路径安全」。
#[must_use]
pub fn sanitize_display_name(raw: &str) -> String {
    let base = raw.rsplit(['/', '\\']).next().unwrap_or(raw);
    let cleaned: String = base
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        return "未命名附件".to_string();
    }
    trimmed.chars().take(MAX_DISPLAY_NAME_CHARS).collect()
}

/// 会话目录当前用量（只算常规文件；`.json` 元信息也算在内）。
#[must_use]
pub fn session_usage(dir: &Path) -> super::temp_files::DirUsage {
    super::temp_files::dir_usage(dir)
}

/// 面向「设置 → 缓存」的统计：`files` 只数附件本体，`bytes` 是全部磁盘占用。
///
/// 和 [`session_usage`] 的区别只在计数口径：用户看到的「N 个附件」不该把旁挂
/// 的 `<id>.json` 元信息也算进去（一个附件会变成两个）；但释放空间时的字节数
/// 必须是真实磁盘占用，元信息那点字节也占地方，算进去才不撒谎。
#[must_use]
pub fn dir_usage(dir: &Path) -> super::temp_files::DirUsage {
    let mut usage = super::temp_files::DirUsage::default();
    let Ok(entries) = fs::read_dir(dir) else {
        return usage;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if file_type.is_dir() {
            let nested = dir_usage(&path);
            usage.files = usage.files.saturating_add(nested.files);
            usage.bytes = usage.bytes.saturating_add(nested.bytes);
        } else if file_type.is_file() {
            let bytes = entry.metadata().map_or(0, |m| m.len());
            usage.bytes = usage.bytes.saturating_add(bytes);
            // `.part` 是半成品、`.json` 是元信息：都不是"附件"。
            let is_meta = path.extension().is_some_and(|ext| {
                ext.eq_ignore_ascii_case("json") || ext.eq_ignore_ascii_case("part")
            });
            if !is_meta {
                usage.files = usage.files.saturating_add(1);
            }
        }
    }
    usage
}

/// 配额判定：本批还能不能收下 `incoming` 字节。
///
/// 返回 `None` 表示放行；`Some(reason)` 是可以直接给用户看的原因。
#[must_use]
pub fn quota_error(usage: super::temp_files::DirUsage, incoming: u64) -> Option<String> {
    if usage.files >= SESSION_QUOTA_FILES {
        return Some(format!(
            "单会话最多 {SESSION_QUOTA_FILES} 个附件，请先清理旧附件"
        ));
    }
    let total = usage.bytes.saturating_add(incoming);
    if total > SESSION_QUOTA_BYTES {
        return Some(format!(
            "单会话附件总量不能超过 {}MB",
            SESSION_QUOTA_BYTES / (1024 * 1024)
        ));
    }
    None
}

/// 写入元信息（先写临时文件再 rename，避免读到半个 JSON）。
///
/// # Errors
/// 磁盘写入失败时返回原始错误。
pub fn write_meta(dir: &Path, meta: &AttachmentMeta) -> io::Result<()> {
    let path = meta.meta_path(dir);
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(meta)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(&tmp, bytes)?;
    fs::rename(tmp, path)
}

/// 读取元信息；缺失或损坏时返回 `None`（调用方按"附件已失效"处理）。
#[must_use]
pub fn read_meta(dir: &Path, id: &str) -> Option<AttachmentMeta> {
    // `id` 来自我们自己的 UUID，但仍然校验一次：将来若从历史记录反查，
    // 也不能凭一个 id 读到目录外的东西。
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return None;
    }
    let path = dir.join(format!("{id}.json"));
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// 一次孤儿清扫的结果。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GcReport {
    /// 被删掉的会话目录名。
    pub removed: Vec<String>,
    pub freed: super::temp_files::DirUsage,
    pub errors: Vec<String>,
}

/// 清扫「所属会话已经不存在」的附件目录。
///
/// 判定很保守：
///
/// - 会话文件不存在（扫 `sessions_dir/*.jsonl` 取 id，兼容 `<ts>_<id>` 与老式 `<id>`）；
/// - **且**目录已静默超过 `orphan_grace_ms` —— 会话文件是第一条 prompt 才写的，
///   刚上传完还没发出去时目录必然"看起来是孤儿"，直接删会吃掉用户正在编辑的附件；
/// - 会话还在的一律保留：用户没要求删的东西，GC 不该替他决定。
///
/// # Arguments
/// * `now_ms` — 当前时间（Unix 毫秒），由调用方传入以便测试。
/// * `orphan_grace_ms` — 孤儿目录的静默期。
#[must_use]
pub fn gc_session_dirs(
    data_dir: &Path,
    sessions_dir: &Path,
    now_ms: i64,
    orphan_grace_ms: i64,
) -> GcReport {
    let mut report = GcReport::default();
    let root = attachments_root(data_dir);
    let Ok(entries) = fs::read_dir(&root) else {
        return report;
    };
    // 读不到会话目录就什么都不删：拿不到"谁是活的"时，把所有附件都当孤儿
    // 清掉是最坏的结果。宁可这一轮不干活，也不能误删。
    let Some(live) = live_session_ids(sessions_dir) else {
        report.errors.push(format!(
            "无法读取会话目录 {}，跳过清扫",
            sessions_dir.display()
        ));
        return report;
    };

    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let path = entry.path();
        let Some(session_id) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if live.contains(session_id) {
            continue;
        }
        let age_ms = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(i64::MAX, |d| now_ms.saturating_sub(d.as_millis() as i64));
        if age_ms < orphan_grace_ms {
            continue;
        }
        let usage = dir_usage(&path);
        if !super::temp_files::is_inside_dir(&path, &root) {
            report
                .errors
                .push(format!("拒绝清理数据根之外的目录：{}", path.display()));
            continue;
        }
        match fs::remove_dir_all(&path) {
            Ok(()) => {
                report.removed.push(session_id.to_string());
                report.freed.add(usage);
            }
            Err(e) => report.errors.push(format!("{}: {e}", path.display())),
        }
    }
    report
}

/// 从会话目录里收集所有现存会话 id；目录读不到时返回 `None`（调用方跳过清扫）。
fn live_session_ids(sessions_dir: &Path) -> Option<std::collections::HashSet<String>> {
    let mut ids = std::collections::HashSet::new();
    let entries = fs::read_dir(sessions_dir).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(stem) = name.strip_suffix(".jsonl") else {
            continue;
        };
        // 两种布局：`<id>` 与 `<timestamp>_<id>`。
        let id = stem.rsplit_once('_').map_or(stem, |(_, id)| id);
        ids.insert(id.to_string());
    }
    Some(ids)
}

/// 删掉一个会话的全部附件目录。
///
/// `data_dir` 是数据根，用于挡住"路径跑到品牌根之外"——与缓存清理同一套守卫。
#[must_use]
pub fn remove_session(data_dir: &Path, session_id: &str) -> bool {
    let Some(dir) = session_dir(data_dir, session_id) else {
        return false;
    };
    let root = attachments_root(data_dir);
    if !super::temp_files::is_inside_dir(&dir, &root) {
        return false;
    }
    fs::remove_dir_all(&dir).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::temp_files::DirUsage;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("atrading-attstore-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("mkdir: {e}"));
        dir
    }

    #[test]
    fn session_ids_are_whitelisted_because_they_are_directory_names() {
        for ok in ["abc", "sess-1", "A_b-9", "0123456789abcdef"] {
            assert!(validate_session_id(ok).is_ok(), "{ok} 应合法");
        }
        for bad in ["", "../etc", "a/b", "a b", "中文", "a.b", ".hidden"] {
            assert!(validate_session_id(bad).is_err(), "{bad} 应被拒");
        }
        assert!(validate_session_id(&"a".repeat(129)).is_err(), "超长应被拒");
        assert!(session_dir(Path::new("/data"), "../x").is_none());
    }

    #[test]
    fn attachment_ids_are_unique_and_path_safe() {
        let a = new_attachment_id();
        let b = new_attachment_id();
        assert_ne!(a, b);
        assert_eq!(a.len(), 36, "UUID v4 带连字符应为 36 字符");
        assert!(a.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-'));
    }

    #[test]
    fn meta_round_trips_and_survives_a_broken_file() {
        let dir = scratch("meta");
        let meta = AttachmentMeta::new(
            new_attachment_id(),
            "sess-1",
            Some("/Users/me/年报.pdf"),
            "application/pdf",
            1234,
            1_700_000_000_000,
        );
        assert_eq!(meta.name, "年报.pdf", "只保留文件名，丢掉目录");
        assert_eq!(meta.kind, AttachmentKind::File);
        assert_eq!(meta.size, 1234);

        fs::write(meta.data_path(&dir), b"payload").unwrap_or_else(|e| panic!("write: {e}"));
        write_meta(&dir, &meta).unwrap_or_else(|e| panic!("meta: {e}"));

        assert_eq!(read_meta(&dir, &meta.id), Some(meta.clone()));

        // 损坏的元信息不 panic，返回 None，调用方按"附件已失效"处理。
        fs::write(meta.meta_path(&dir), b"{ not json").unwrap_or_else(|e| panic!("write: {e}"));
        assert_eq!(read_meta(&dir, &meta.id), None);

        // 非法 id 不会读成目录外的文件。
        assert_eq!(read_meta(&dir, "../secret"), None);
        assert_eq!(read_meta(&dir, ""), None);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn display_names_are_cleaned_but_never_used_as_paths() {
        assert_eq!(sanitize_display_name("a/b/c.png"), "c.png");
        assert_eq!(sanitize_display_name("win\\path.docx"), "path.docx");
        assert_eq!(sanitize_display_name("  "), "未命名附件");
        assert_eq!(sanitize_display_name("bad\u{0}name.txt"), "bad name.txt");
        assert_eq!(
            sanitize_display_name(&"长".repeat(300)).chars().count(),
            MAX_DISPLAY_NAME_CHARS
        );
        assert_eq!(
            AttachmentKind::from_mime("IMAGE/PNG"),
            AttachmentKind::Image,
            "大小写不敏感"
        );
        assert_eq!(AttachmentKind::from_mime(""), AttachmentKind::File);
    }

    #[test]
    fn quotas_guard_both_bytes_and_file_count() {
        // 还有空间
        assert_eq!(quota_error(DirUsage::new(1, 1024), 1024), None);
        // 刚好打满字节配额：放行
        assert_eq!(
            quota_error(DirUsage::new(1, SESSION_QUOTA_BYTES - 10), 10),
            None
        );
        // 超一个字节：拒绝
        let over = quota_error(DirUsage::new(1, SESSION_QUOTA_BYTES - 9), 10);
        assert!(
            over.unwrap_or_default().contains("总量"),
            "应是字节配额原因"
        );
        // 文件数打满
        let files = quota_error(DirUsage::new(SESSION_QUOTA_FILES, 0), 1);
        assert!(files.unwrap_or_default().contains("个附件"), "应是数量原因");
        // 溢出不会 panic
        assert!(quota_error(DirUsage::new(0, u64::MAX), u64::MAX).is_some());
    }

    #[test]
    fn cache_facing_usage_counts_payloads_but_charges_all_bytes() {
        let root = scratch("usage");
        let dir = session_dir(&root, "sess-1").unwrap_or_else(|| panic!("dir"));
        fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("mkdir: {e}"));
        // 两个附件本体 + 一个元信息 + 一个半成品
        fs::write(dir.join("id-1"), vec![0u8; 100]).unwrap_or_else(|e| panic!("write: {e}"));
        fs::write(dir.join("id-2"), vec![0u8; 50]).unwrap_or_else(|e| panic!("write: {e}"));
        fs::write(dir.join("id-1.json"), b"{}").unwrap_or_else(|e| panic!("write: {e}"));
        fs::write(dir.join("id-3.part"), vec![0u8; 10]).unwrap_or_else(|e| panic!("write: {e}"));

        let usage = dir_usage(&root);
        assert_eq!(usage.files, 2, "只数附件本体，不含 .json / .part");
        assert_eq!(usage.bytes, 100 + 50 + 2 + 10, "字节数要覆盖全部磁盘占用");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn gc_removes_only_orphans_past_the_grace_period() {
        let root = scratch("gc");
        let sessions = root.join("sessions");
        fs::create_dir_all(&sessions).unwrap_or_else(|e| panic!("mkdir: {e}"));
        // 活会话：`<ts>_<id>` 与老式 `<id>` 两种布局都要认
        fs::write(sessions.join("2026-01-01T00-00-00_alive-1.jsonl"), b"{}")
            .unwrap_or_else(|e| panic!("write: {e}"));
        fs::write(sessions.join("alive-2.jsonl"), b"{}").unwrap_or_else(|e| panic!("write: {e}"));

        let orphan = session_dir(&root, "gone-1").unwrap_or_else(|| panic!("dir"));
        fs::create_dir_all(&orphan).unwrap_or_else(|e| panic!("mkdir: {e}"));
        fs::write(orphan.join("id"), vec![0u8; 10]).unwrap_or_else(|e| panic!("write: {e}"));
        for live in ["alive-1", "alive-2"] {
            let dir = session_dir(&root, live).unwrap_or_else(|| panic!("dir"));
            fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("mkdir: {e}"));
            fs::write(dir.join("id"), b"x").unwrap_or_else(|e| panic!("write: {e}"));
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or_default();

        // 静默期内不删（刚上传、还没发出去的附件就是这个状态）
        let early = gc_session_dirs(&root, &sessions, now, 60 * 60 * 1000);
        assert!(
            early.removed.is_empty(),
            "宽限期内不该删：{:?}",
            early.removed
        );
        assert!(orphan.is_dir());

        // 过了静默期才删
        let later = gc_session_dirs(&root, &sessions, now + 2 * 60 * 60 * 1000, 60 * 60 * 1000);
        assert_eq!(later.removed, vec!["gone-1".to_string()]);
        assert_eq!(later.freed.files, 1);
        assert!(!orphan.exists(), "孤儿目录应被删掉");
        for live in ["alive-1", "alive-2"] {
            let dir = session_dir(&root, live).unwrap_or_else(|| panic!("dir"));
            assert!(dir.is_dir(), "活会话的附件不能动：{live}");
        }

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn gc_without_a_sessions_dir_keeps_everything() {
        let root = scratch("gc-nosessions");
        let dir = session_dir(&root, "sess-1").unwrap_or_else(|| panic!("dir"));
        fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("mkdir: {e}"));

        // 会话目录不存在 = 读不到任何活会话。这时必须"一个都不删"：
        // 宁可留着让用户手动清，也不能因为路径配错就把附件全清了。
        let report = gc_session_dirs(&root, &root.join("no-such-dir"), i64::MAX, 0);
        assert!(report.removed.is_empty());
        assert!(dir.is_dir());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn storing_from_a_path_streams_the_file_and_gets_the_mime_right() {
        let root = scratch("from-path");
        let src = root.join("年报.pdf");
        fs::write(&src, b"%PDF-1.4 hello").unwrap_or_else(|e| panic!("write: {e}"));

        let meta =
            store_from_path(&root, "sess-1", &src).unwrap_or_else(|e| panic!("store: {e:?}"));
        assert_eq!(meta.name, "年报.pdf");
        assert_eq!(meta.size, 14);
        assert_eq!(meta.mime_type, "application/pdf");
        assert_eq!(meta.kind, AttachmentKind::File);

        let dir = session_dir(&root, "sess-1").unwrap_or_else(|| panic!("dir"));
        assert_eq!(
            fs::read(meta.data_path(&dir)).unwrap_or_else(|e| panic!("read: {e}")),
            b"%PDF-1.4 hello"
        );
        assert!(read_meta(&dir, &meta.id).is_some());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn storing_from_a_path_rejects_missing_empty_and_oversized_files() {
        let root = scratch("from-path-bad");
        assert!(matches!(
            store_from_path(&root, "sess-1", &root.join("no-such-file")),
            Err(StoreError::Io(_))
        ));

        let empty = root.join("empty.txt");
        fs::write(&empty, b"").unwrap_or_else(|e| panic!("write: {e}"));
        assert_eq!(
            store_from_path(&root, "sess-1", &empty),
            Err(StoreError::Empty)
        );

        // 超出单文件上限：用稀疏文件撑尺寸，不真占 512MB
        let huge = root.join("huge.bin");
        let file = fs::File::create(&huge).unwrap_or_else(|e| panic!("create: {e}"));
        file.set_len(MAX_UPLOAD_BYTES + 1)
            .unwrap_or_else(|e| panic!("set_len: {e}"));
        assert!(matches!(
            store_from_path(&root, "sess-1", &huge),
            Err(StoreError::TooLarge { .. })
        ));

        // 什么都不能留在会话目录里
        let dir = session_dir(&root, "sess-1").unwrap_or_else(|| panic!("dir"));
        if dir.exists() {
            let leftovers: Vec<_> = fs::read_dir(&dir)
                .unwrap_or_else(|e| panic!("readdir: {e}"))
                .flatten()
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect();
            assert!(leftovers.is_empty(), "不应留下残留：{leftovers:?}");
        }

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn mime_guessing_is_best_effort_only() {
        assert_eq!(guess_mime_from_name("a.PNG"), "image/png");
        assert_eq!(guess_mime_from_name("a.md"), "text/markdown");
        assert_eq!(guess_mime_from_name("noext"), "application/octet-stream");
        assert_eq!(
            guess_mime_from_name("a.unknown"),
            "application/octet-stream"
        );
    }

    #[test]
    fn removing_a_session_drops_only_that_session() {
        let root = scratch("remove");
        let a = session_dir(&root, "sess-a").unwrap_or_else(|| panic!("dir"));
        let b = session_dir(&root, "sess-b").unwrap_or_else(|| panic!("dir"));
        fs::create_dir_all(&a).unwrap_or_else(|e| panic!("mkdir: {e}"));
        fs::create_dir_all(&b).unwrap_or_else(|e| panic!("mkdir: {e}"));
        fs::write(a.join("x"), b"1").unwrap_or_else(|e| panic!("write: {e}"));
        fs::write(b.join("y"), b"2").unwrap_or_else(|e| panic!("write: {e}"));

        assert!(remove_session(&root, "sess-a"));
        assert!(!a.exists(), "sess-a 应被删掉");
        assert!(b.exists(), "sess-b 不能受牵连");

        // 非法 id 与已删除的 id 都返回 false，不 panic。
        assert!(!remove_session(&root, "../etc"));
        assert!(!remove_session(&root, "sess-a"));

        let _ = fs::remove_dir_all(&root);
    }
}
