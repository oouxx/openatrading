//! Prompt 附件：**把读取文件的能力交给 pi-rs 的 `read` 工具**。
//!
//! 只有三种情况需要这边动手，其余一律只给绝对路径，让 agent 自己按需读：
//!
//! - **图片** → `ContentBlock::Image`。pi-rs 的 `process_image` 会嗅探格式、
//!   必要时转 PNG、按模型内联上限缩放；图片必须解码才能用，没法懒加载。
//! - **小文本（≤ [`INLINE_MAX_TEXT_BYTES`]）** → 内联进 prompt，省掉一次工具往返。
//! - **其它任何文件** → `<file name="…" path="/abs/…" size="…"/>`，由 `read`
//!   按 2000 行 / 50KB 分页读取（`offset` 续读）。
//!
//! 这样 PDF、几百 MB 的日志、Office 文档都不需要我们自己解析：不抽取就不会
//! 有"截断丢内容"，也不会把全文塞进历史让之后每一轮都重发。
//!
//! 粘贴的字节在 webview 里没有路径，落盘到 `<数据根>/attachments/<会话>/` 后
//! 再引用，保证 prompt 里出现的每个路径都真实可读。

use std::io::Read;
use std::path::{Path, PathBuf};

use base64::Engine;
use pi_ai::types::ContentBlock;
use pi_coding_agent::core::agent_session::PromptOptions;

/// 单条 prompt 的附件数上限。
pub const MAX_ATTACHMENTS: usize = 20;
/// 小文本内联阈值：约等于一次 `read` 能取回的文本量（2000 行 / 50KB）。
pub const INLINE_MAX_TEXT_BYTES: u64 = 64 * 1024;
/// 图片上限：必须解码才能变成 image block，不能像文本那样懒加载。
pub const MAX_IMAGE_BYTES: u64 = 64 * 1024 * 1024;

const ATTACHMENT_OPEN: &str = "<openatrading_attachments marker=\"openatrading-v1\">";
const ATTACHMENT_CLOSE: &str = "</openatrading_attachments>";
/// 只被引用、没有内联的文件：明确告诉模型这些路径要用 `read` 读。
const READ_NOTE: &str = "Files without inline content are on disk. Read them with the read \
tool; use offset/limit to continue a long file.";

/// 图片附件：base64 载荷（粘贴）或绝对路径（文件选择器 / 拖拽）二选一。
///
/// `mime_type` 只是客户端元信息：真正的类型由字节决定。
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ImageAttachment {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, rename = "mimeType")]
    pub mime_type: Option<String>,
}

/// 非图片附件：同样 `data` / `path` 二选一。
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct FileAttachment {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// 解析附件路径所需的最小上下文。
///
/// `root` 非空时只接受该目录下的路径：Web 网关的用户不是这台机器的主人，
/// 不该凭一个 prompt 读到服务器上的任意文件。桌面端（本机单人）传 `None`，
/// 与 pi-rs `read` 工具的实际权限一致。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentScope {
    pub data_dir: std::path::PathBuf,
    pub session_id: String,
    pub restricted_root: Option<std::path::PathBuf>,
}

impl AttachmentScope {
    /// 桌面端：数据根 + 会话，路径不受限制。
    #[must_use]
    pub fn desktop(session_id: impl Into<String>) -> Self {
        Self {
            data_dir: crate::atrading_home(),
            session_id: session_id.into(),
            restricted_root: None,
        }
    }

    /// Web 端：只允许引用附件目录下的路径。
    #[must_use]
    pub fn web(session_id: impl Into<String>) -> Self {
        let data_dir = crate::atrading_home();
        Self {
            restricted_root: Some(crate::utils::attachment_store::attachments_root(&data_dir)),
            data_dir,
            session_id: session_id.into(),
        }
    }
}

/// prompt 里记录的一条 `<file>`，用于历史回放时渲染 chip。
#[derive(Debug, PartialEq, Eq)]
pub struct AttachedFileRef {
    pub name: String,
    pub size: u64,
    /// 内容被内联进 prompt（小文本），而不是只引用路径。
    pub inlined: bool,
}

#[derive(Debug)]
pub struct ProcessedAttachments {
    pub images: Vec<ContentBlock>,
    pub prompt_text: String,
}

/// 一个 transport 的 prompt 校验完成后、交给 pi-rs 的形状。
pub struct PreparedPrompt {
    pub text: String,
    pub options: PromptOptions,
}

/// 两个传输都在 steer：回答过程中再发一条 prompt 是注入当前回合，而不是排队。
const STREAMING_BEHAVIOR_STEER: &str = "steer";

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum AttachmentError {
    #[error("一条消息最多支持 {max} 个附件")]
    TooMany { max: usize },
    #[error("「{name}」既没有内联数据，也没有路径")]
    MissingPayload { name: String },
    #[error("「{name}」无法读取：{reason}")]
    Unreadable { name: String, reason: String },
    #[error("「{name}」超过 {max_mb}MB，图片需要解码后才能使用")]
    ImageTooLarge { name: String, max_mb: u64 },
    #[error("「{name}」不是有效的受支持图片")]
    InvalidImage { name: String },
    #[error("「{name}」不在允许读取的目录内")]
    OutsideRoot { name: String },
}

pub struct AttachmentService;

struct ResolvedFile {
    name: String,
    path: PathBuf,
    size: u64,
}

impl AttachmentService {
    /// 校验一条 prompt 的附件，并组出 pi-rs 需要的东西。
    ///
    /// 两个传输（WebSocket / Tauri IPC）都走这里，图片归一化与引用策略不会分叉。
    ///
    /// # Errors
    /// 附件过多、路径不可读、图片无法解码时返回 [`AttachmentError`]。
    pub fn prepare_prompt(
        text: &str,
        images: &[ImageAttachment],
        files: &[FileAttachment],
        scope: Option<&AttachmentScope>,
    ) -> Result<PreparedPrompt, AttachmentError> {
        let processed = Self::process(text, images, files, scope)?;
        Ok(PreparedPrompt {
            text: processed.prompt_text,
            options: PromptOptions {
                streaming_behavior: Some(STREAMING_BEHAVIOR_STEER.to_string()),
                images: (!processed.images.is_empty()).then_some(processed.images),
                ..Default::default()
            },
        })
    }

    /// 把附件拆成「图片块 / 内联文本 / 仅引用路径」三路。
    ///
    /// # Errors
    /// 同 [`Self::prepare_prompt`]。
    pub fn process(
        text: &str,
        images: &[ImageAttachment],
        files: &[FileAttachment],
        scope: Option<&AttachmentScope>,
    ) -> Result<ProcessedAttachments, AttachmentError> {
        let count = images.len().saturating_add(files.len());
        if count > MAX_ATTACHMENTS {
            return Err(AttachmentError::TooMany {
                max: MAX_ATTACHMENTS,
            });
        }
        if count == 0 {
            return Ok(ProcessedAttachments {
                images: Vec::new(),
                prompt_text: text.to_string(),
            });
        }

        let mut blocks: Vec<ContentBlock> = Vec::new();
        let mut hints: Vec<(String, u64, Vec<String>)> = Vec::new();
        let mut inlined: Vec<(String, u64, String)> = Vec::new();
        let mut referenced: Vec<(String, u64)> = Vec::new();

        for (index, image) in images.iter().enumerate() {
            let name = display_name(image.name.as_deref(), &format!("图片 {}", index + 1));
            let bytes = image_bytes(image, &name, scope)?;
            let (block, image_hints) = normalize_image(&name, &bytes)?;
            blocks.push(block);
            if !image_hints.is_empty() {
                hints.push((name, bytes.len() as u64, image_hints));
            }
        }

        for (index, file) in files.iter().enumerate() {
            let fallback = format!("附件 {}", index + 1);
            let resolved = resolve_file(file, &fallback, scope)?;

            // 文件通道也接受图片：浏览器可能给出没有 MIME 的图片（从文件管理器
            // 拖入），模型必须拿到 image block 而不是读不懂的文本。
            if is_image_path(&resolved.path) {
                let bytes = read_image(&resolved, scope)?;
                let (block, image_hints) = normalize_image(&resolved.name, &bytes)?;
                blocks.push(block);
                if !image_hints.is_empty() {
                    hints.push((resolved.name, bytes.len() as u64, image_hints));
                }
                continue;
            }

            // 记录的是**绝对路径**：`<file name="/abs/…"/>` 就是模型调用
            // `read` 时要用的参数，不需要第二条属性。
            let shown = absolute(&resolved.path);
            match inline_text(&resolved.path, resolved.size) {
                Some(content) => inlined.push((shown, resolved.size, content)),
                None => referenced.push((shown, resolved.size)),
            }
        }

        let wrapper = attachment_wrapper(&inlined, &referenced, &hints);
        let prompt_text = match (text.is_empty(), wrapper.is_empty()) {
            (true, true) => String::new(),
            (false, true) => text.to_string(),
            (true, false) => wrapper.clone(),
            (false, false) => format!("{text}\n\n{wrapper}"),
        };

        Ok(ProcessedAttachments {
            images: blocks,
            prompt_text,
        })
    }
}

fn attachment_wrapper(
    inlined: &[(String, u64, String)],
    referenced: &[(String, u64)],
    hints: &[(String, u64, Vec<String>)],
) -> String {
    if inlined.is_empty() && referenced.is_empty() && hints.is_empty() {
        return String::new();
    }
    let mut out = String::from(ATTACHMENT_OPEN);
    for (name, size, content) in inlined {
        out.push_str("\n  <file name=\"");
        out.push_str(&escape_xml_attribute(name));
        out.push_str("\" size=\"");
        out.push_str(&size.to_string());
        out.push_str("\">");
        out.push_str(&escape_xml_text(content));
        out.push_str("</file>");
    }
    for (name, size) in referenced {
        out.push_str("\n  <file name=\"");
        out.push_str(&escape_xml_attribute(name));
        out.push_str("\" size=\"");
        out.push_str(&size.to_string());
        out.push_str("\"/>");
    }
    for (name, size, notes) in hints {
        for note in notes {
            out.push_str("\n  <image_hint name=\"");
            out.push_str(&escape_xml_attribute(name));
            out.push_str("\" size=\"");
            out.push_str(&size.to_string());
            out.push_str("\">");
            out.push_str(&escape_xml_text(note));
            out.push_str("</image_hint>");
        }
    }
    if !referenced.is_empty() {
        out.push_str("\n  <note>");
        out.push_str(&escape_xml_text(READ_NOTE));
        out.push_str("</note>");
    }
    out.push('\n');
    out.push_str(ATTACHMENT_CLOSE);
    out
}

// ── 载荷解析 ──────────────────────────────────────────────

fn absolute(path: &Path) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string()
}

/// 客户端给的 base64 可能带 `data:` 前缀与换行，统一在这里剥掉。
fn decode_base64(data: &str) -> Option<Vec<u8>> {
    let payload = match data.find(',') {
        Some(comma) if data.starts_with("data:") => &data[comma + 1..],
        _ => data,
    };
    let compact: String = payload.chars().filter(|c| !c.is_whitespace()).collect();
    base64::engine::general_purpose::STANDARD
        .decode(&compact)
        .or_else(|_| {
            base64::engine::general_purpose::STANDARD_NO_PAD.decode(compact.trim_end_matches('='))
        })
        .ok()
}

fn display_name(name: Option<&str>, fallback: &str) -> String {
    let base = name
        .map(|n| n.rsplit(['/', '\\']).next().unwrap_or(n))
        .unwrap_or("")
        .trim();
    if base.is_empty() {
        fallback.to_string()
    } else {
        base.chars().take(200).collect()
    }
}

fn image_bytes(
    image: &ImageAttachment,
    name: &str,
    scope: Option<&AttachmentScope>,
) -> Result<Vec<u8>, AttachmentError> {
    if let Some(data) = image.data.as_deref().filter(|d| !d.trim().is_empty()) {
        return decode_base64(data).ok_or_else(|| AttachmentError::InvalidImage {
            name: name.to_string(),
        });
    }
    let Some(path) = image.path.as_deref().filter(|p| !p.trim().is_empty()) else {
        return Err(AttachmentError::MissingPayload {
            name: name.to_string(),
        });
    };
    let resolved = checked_path(name, Path::new(path), scope)?;
    read_image(&resolved, scope)
}

fn read_image(
    resolved: &ResolvedFile,
    _scope: Option<&AttachmentScope>,
) -> Result<Vec<u8>, AttachmentError> {
    if resolved.size > MAX_IMAGE_BYTES {
        return Err(AttachmentError::ImageTooLarge {
            name: resolved.name.clone(),
            max_mb: MAX_IMAGE_BYTES / (1024 * 1024),
        });
    }
    std::fs::read(&resolved.path).map_err(|e| AttachmentError::Unreadable {
        name: resolved.name.clone(),
        reason: e.to_string(),
    })
}

/// 非图片附件：路径直接用，字节（粘贴）先落盘再引用。
fn resolve_file(
    file: &FileAttachment,
    fallback: &str,
    scope: Option<&AttachmentScope>,
) -> Result<ResolvedFile, AttachmentError> {
    let name = display_name(file.name.as_deref(), fallback);
    if let Some(path) = file.path.as_deref().filter(|p| !p.trim().is_empty()) {
        return checked_path(&name, Path::new(path), scope);
    }
    let Some(data) = file.data.as_deref().filter(|d| !d.trim().is_empty()) else {
        return Err(AttachmentError::MissingPayload { name });
    };
    let bytes = decode_base64(data).ok_or_else(|| AttachmentError::Unreadable {
        name: name.clone(),
        reason: "内容不是合法 base64".to_string(),
    })?;
    let scope = scope.ok_or_else(|| AttachmentError::Unreadable {
        name: name.clone(),
        reason: "缺少会话上下文，无法保存粘贴的文件".to_string(),
    })?;
    let path = crate::utils::attachment_store::spool_bytes(
        &scope.data_dir,
        &scope.session_id,
        &name,
        &bytes,
    )
    .map_err(|e| AttachmentError::Unreadable {
        name: name.clone(),
        reason: e.to_string(),
    })?;
    Ok(ResolvedFile {
        name,
        size: bytes.len() as u64,
        path,
    })
}

fn checked_path(
    name: &str,
    path: &Path,
    scope: Option<&AttachmentScope>,
) -> Result<ResolvedFile, AttachmentError> {
    if let Some(root) = scope.and_then(|s| s.restricted_root.as_deref()) {
        // 两边都要 canonicalize：macOS 的 /tmp 是指向 /private/tmp 的软链，
        // 只规整一边会让同一路径比出不同前缀。
        let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        let absolute = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        if !absolute.starts_with(&root) {
            return Err(AttachmentError::OutsideRoot {
                name: name.to_string(),
            });
        }
    }
    let meta = std::fs::metadata(path).map_err(|e| AttachmentError::Unreadable {
        name: name.to_string(),
        reason: e.to_string(),
    })?;
    if !meta.is_file() {
        return Err(AttachmentError::Unreadable {
            name: name.to_string(),
            reason: "不是一个文件".to_string(),
        });
    }
    Ok(ResolvedFile {
        name: name.to_string(),
        path: path.to_path_buf(),
        size: meta.len(),
    })
}

fn is_image_path(path: &Path) -> bool {
    pi_coding_agent::utils::mime::detect_supported_image_mime_type_from_file(
        &path.to_string_lossy(),
    )
    .ok()
    .flatten()
    .is_some()
}

/// 小文本内联：过大、非 UTF-8、含危险控制字符的一律留给 `read`。
fn inline_text(path: &Path, size: u64) -> Option<String> {
    if size > INLINE_MAX_TEXT_BYTES {
        return None;
    }
    let mut bytes = Vec::with_capacity(size as usize);
    std::fs::File::open(path)
        .and_then(|file| file.take(INLINE_MAX_TEXT_BYTES).read_to_end(&mut bytes))
        .ok()?;
    let text = String::from_utf8(bytes).ok()?;
    if has_dangerous_controls(&text) {
        return None;
    }
    Some(normalize_newlines(text.trim_start_matches('\u{feff}')))
}

fn normalize_newlines(text: &str) -> String {
    if text.contains('\r') {
        text.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        text.to_string()
    }
}

/// 制表/换行之外的连制字符与 bidi 覆盖符：这类文件留在磁盘上，不塞进 prompt。
fn has_dangerous_controls(text: &str) -> bool {
    text.chars().any(|c| {
        !matches!(c, '\n' | '\r' | '\t')
            && (c.is_control()
                || matches!(
                    c,
                    '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
                ))
    })
}

// ── 图片 ──────────────────────────────────────────────────

/// 走 pi-rs 的图片管线（嗅探格式、必要时转 PNG、按模型内联上限缩放）。
fn normalize_image(name: &str, raw: &[u8]) -> Result<(ContentBlock, Vec<String>), AttachmentError> {
    let mime_type = pi_coding_agent::utils::mime::detect_supported_image_mime_type(raw)
        .ok_or_else(|| AttachmentError::InvalidImage {
            name: name.to_string(),
        })?;
    let processed = pi_coding_agent::utils::image_process::process_image(
        raw, mime_type, true, None,
    )
    .map_err(|_| AttachmentError::InvalidImage {
        name: name.to_string(),
    })?;
    Ok((
        ContentBlock::Image {
            data: processed.data,
            mime_type: processed.mime_type,
        },
        processed.hints,
    ))
}

// ── 历史回放 ──────────────────────────────────────────────

/// 把附件 wrapper 从一条已持久化的 prompt 末尾切出来。
///
/// 返回用户可见的文本与 `<file>` 条目；图片 hint 丢弃（图片已是独立 content
/// block）。没有 wrapper 的 prompt（老会话、纯文本）原样返回。
#[must_use]
pub fn split_attachment_wrapper(text: &str) -> (String, Vec<AttachedFileRef>) {
    let Some(close) = text.rfind(ATTACHMENT_CLOSE) else {
        return (text.to_string(), Vec::new());
    };
    if close + ATTACHMENT_CLOSE.len() != text.len() {
        return (text.to_string(), Vec::new());
    }
    let Some(open) = text[..close].rfind(ATTACHMENT_OPEN) else {
        return (text.to_string(), Vec::new());
    };

    let body = &text[open + ATTACHMENT_OPEN.len()..close];
    let mut visible = &text[..open];
    if let Some(stripped) = visible.strip_suffix("\n\n") {
        visible = stripped;
    }
    (visible.to_string(), parse_file_entries(body))
}

fn parse_file_entries(body: &str) -> Vec<AttachedFileRef> {
    const OPEN: &str = "<file name=\"";
    let mut entries = Vec::new();
    let mut rest = body;

    while let Some(start) = rest.find(OPEN) {
        rest = &rest[start + OPEN.len()..];
        let Some(name_end) = rest.find('"') else {
            break;
        };
        let name = unescape_xml_text(&rest[..name_end]);
        rest = &rest[name_end + 1..];

        let Some(after_size) = rest.strip_prefix(" size=\"") else {
            continue;
        };
        let Some((digits, remainder)) = after_size.split_once('"') else {
            continue;
        };
        let Ok(size) = digits.parse::<u64>() else {
            continue;
        };
        // 自闭合（`/>`）= 只引用路径；带 `>` = 内容已内联。
        let inlined = remainder.trim_start().starts_with('>');
        rest = remainder;
        entries.push(AttachedFileRef {
            name,
            size,
            inlined,
        });
    }
    entries
}

fn escape_xml_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn escape_xml_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn unescape_xml_text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.char_indices();
    while let Some((index, c)) = chars.next() {
        if c != '&' {
            out.push(c);
            continue;
        }
        let rest = &value[index..];
        if let Some(escaped) = rest.strip_prefix("&amp;") {
            out.push('&');
            chars.nth(escaped.len().saturating_sub(1));
        } else if let Some(escaped) = rest.strip_prefix("&lt;") {
            out.push('<');
            chars.nth(escaped.len().saturating_sub(1));
        } else if let Some(escaped) = rest.strip_prefix("&gt;") {
            out.push('>');
            chars.nth(escaped.len().saturating_sub(1));
        } else if let Some(escaped) = rest.strip_prefix("&quot;") {
            out.push('"');
            chars.nth(escaped.len().saturating_sub(1));
        } else {
            out.push('&');
        }
    }
    out
}

use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests {
    use super::*;

    const PNG_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("atrading-att-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("mkdir: {e}"));
        dir
    }

    fn file_path(dir: &Path, name: &str, bytes: &[u8]) -> String {
        std::fs::write(dir.join(name), bytes).unwrap_or_else(|e| panic!("write: {e}"));
        dir.join(name).to_string_lossy().to_string()
    }

    fn file_attachment(path: &str) -> FileAttachment {
        FileAttachment {
            name: None,
            data: None,
            path: Some(path.to_string()),
        }
    }

    #[test]
    fn small_text_is_inlined_into_the_prompt() {
        let dir = scratch("inline");
        let path = file_path(&dir, "notes.md", "# 标题\n正文".as_bytes());

        let out = AttachmentService::process("总结", &[], &[file_attachment(&path)], None)
            .unwrap_or_else(|e| panic!("process: {e}"));

        assert!(out.prompt_text.contains("<file name="), "应有 wrapper");
        assert!(out.prompt_text.contains("标题"), "小文本应内联内容");
        assert!(
            !out.prompt_text.contains(READ_NOTE),
            "内联时不该出现 read 提示"
        );

        let (_, refs) = split_attachment_wrapper(&out.prompt_text);
        assert_eq!(refs.len(), 1);
        assert!(refs[0].inlined);
        assert!(refs[0].name.ends_with("notes.md"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn large_text_is_only_referenced_with_a_read_hint() {
        let dir = scratch("referenced");
        let big = vec![b'a'; (INLINE_MAX_TEXT_BYTES + 1) as usize];
        let path = file_path(&dir, "huge.log", &big);

        let out = AttachmentService::process("看看", &[], &[file_attachment(&path)], None)
            .unwrap_or_else(|e| panic!("process: {e}"));

        assert!(!out.prompt_text.contains(&"a".repeat(4096)), "不能内联内容");
        assert!(out.prompt_text.contains("huge.log"), "应给出路径");
        assert!(out.prompt_text.contains(READ_NOTE), "应提示用 read 读");

        let (_, refs) = split_attachment_wrapper(&out.prompt_text);
        assert_eq!(refs.len(), 1);
        assert!(!refs[0].inlined);
        assert_eq!(refs[0].size, INLINE_MAX_TEXT_BYTES + 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn binary_files_are_referenced_not_decoded() {
        let dir = scratch("binary");
        // PDF 头 + 二进制：不抽取、不报错，只给路径。
        let path = file_path(&dir, "年报.pdf", b"%PDF-1.4\n\x00\x01\x02binary");
        let out = AttachmentService::process("", &[], &[file_attachment(&path)], None)
            .unwrap_or_else(|e| panic!("process: {e}"));

        assert!(out.prompt_text.contains("年报.pdf"));
        assert!(!out.prompt_text.contains("%PDF"), "不应把二进制塞进 prompt");
        assert!(out.prompt_text.contains(READ_NOTE));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn images_come_from_a_path_as_an_image_block() {
        let dir = scratch("image");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(PNG_BASE64)
            .unwrap_or_else(|e| panic!("decode: {e}"));
        let path = file_path(&dir, "chart.png", &bytes);

        let out = AttachmentService::process(
            "看图",
            &[ImageAttachment {
                name: Some("chart.png".to_string()),
                data: None,
                path: Some(path.clone()),
                mime_type: None,
            }],
            &[],
            None,
        )
        .unwrap_or_else(|e| panic!("process: {e}"));

        assert_eq!(out.images.len(), 1);
        assert!(matches!(&out.images[0], ContentBlock::Image { .. }));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn images_sent_through_the_file_channel_become_image_blocks() {
        let dir = scratch("file-image");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(PNG_BASE64)
            .unwrap_or_else(|e| panic!("decode: {e}"));
        let path = file_path(&dir, "clip.png", &bytes);

        let out = AttachmentService::process("", &[], &[file_attachment(&path)], None)
            .unwrap_or_else(|e| panic!("process: {e}"));

        assert_eq!(out.images.len(), 1, "文件通道里的图片也要变 image block");
        assert!(!out.prompt_text.contains("clip.png"), "图片不进文件列表");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn paths_outside_the_restricted_root_are_rejected() {
        let dir = scratch("outside");
        let path = file_path(&dir, "secret.txt", b"top secret");

        let scope = AttachmentScope {
            data_dir: dir.clone(),
            session_id: "sess-1".to_string(),
            restricted_root: Some(dir.join("allowed")),
        };

        match AttachmentService::process("", &[], &[file_attachment(&path)], Some(&scope)) {
            Err(AttachmentError::OutsideRoot { .. }) => {}
            other => panic!("Web 端不得引用附件目录之外的路径，得到 {other:?}"),
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_payloads_fail_loudly() {
        assert!(matches!(
            AttachmentService::process("", &[], &[file_attachment("/nonexistent/x.txt")], None),
            Err(AttachmentError::Unreadable { .. })
        ));
        assert!(matches!(
            AttachmentService::process(
                "",
                &[],
                &[FileAttachment {
                    name: Some("ghost.txt".to_string()),
                    data: None,
                    path: None,
                }],
                None,
            ),
            Err(AttachmentError::MissingPayload { .. })
        ));
    }

    #[test]
    fn too_many_attachments_are_rejected() {
        let dir = scratch("count");
        let path = file_path(&dir, "a.txt", b"x");
        let files: Vec<FileAttachment> = (0..=MAX_ATTACHMENTS)
            .map(|_| file_attachment(&path))
            .collect();

        assert!(matches!(
            AttachmentService::process("", &[], &files, None),
            Err(AttachmentError::TooMany { .. })
        ));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pasted_bytes_are_spooled_and_referenced() {
        let dir = scratch("spool");
        let scope = AttachmentScope {
            data_dir: dir.clone(),
            session_id: "sess-1".to_string(),
            restricted_root: None,
        };

        let out = AttachmentService::process(
            "",
            &[],
            &[FileAttachment {
                name: Some("粘贴.txt".to_string()),
                data: Some(base64::engine::general_purpose::STANDARD.encode("内联内容")),
                path: None,
            }],
            Some(&scope),
        )
        .unwrap_or_else(|e| panic!("process: {e}"));

        // 落盘后才能被 read 读到；小文本同时内联，省一次工具往返。
        let spooled = crate::utils::attachment_store::attachments_root(&dir).join("sess-1");
        assert!(spooled.is_dir(), "粘贴的字节应落盘");
        assert!(out.prompt_text.contains("内联内容"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn wrapper_round_trips_through_history_replay() {
        let dir = scratch("replay");
        let small = file_path(&dir, "a.md", b"small");
        let big = file_path(
            &dir,
            "b.log",
            &vec![b'x'; (INLINE_MAX_TEXT_BYTES + 1) as usize],
        );

        let out = AttachmentService::process(
            "看看这两个",
            &[],
            &[file_attachment(&small), file_attachment(&big)],
            None,
        )
        .unwrap_or_else(|e| panic!("process: {e}"));

        let (visible, refs) = split_attachment_wrapper(&out.prompt_text);
        assert_eq!(visible, "看看这两个", "wrapper 不能留在可见文本里");
        assert_eq!(refs.len(), 2);
        assert!(refs[0].inlined && !refs[1].inlined);
        // 名字是绝对路径（`read` 的参数），UI 需要时自己取 basename。
        assert!(refs.iter().all(|r| r.name.starts_with('/')));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn plain_prompts_are_untouched() {
        let out = AttachmentService::process("只有文字", &[], &[], None)
            .unwrap_or_else(|e| panic!("process: {e}"));
        assert_eq!(out.prompt_text, "只有文字");
        assert!(out.images.is_empty());

        let (visible, refs) = split_attachment_wrapper("没有 wrapper 的老消息");
        assert_eq!(visible, "没有 wrapper 的老消息");
        assert!(refs.is_empty());
    }
}
