//! Prompt attachment validation and normalization.
//!
//! Images are detected and normalized by pi-rs so every transport uses the
//! same image semantics. Every other file arrives as raw bytes: PDF and
//! Office Open XML (docx / xlsx / pptx) documents are reduced to text, plain
//! UTF-8 files are embedded verbatim, and anything else is rejected with an
//! explicit error. The extracted text is embedded into the prompt as escaped
//! XML, which keeps a single prompt shape for every transport.

use std::io::{Cursor, Read};

use base64::Engine;
use pi_ai::types::ContentBlock;
use pi_coding_agent::core::agent_session::PromptOptions;
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;
use quick_xml::XmlVersion;
use serde::{Deserialize, Serialize};

pub const MAX_ATTACHMENTS: usize = 20;
pub const MAX_FILE_BYTES: usize = 20 * 1024 * 1024;
pub const MAX_TOTAL_BYTES: usize = 40 * 1024 * 1024;
/// Per-file limit on the text embedded into the prompt. Larger documents are
/// truncated with an explicit note so the model knows content is missing.
pub const MAX_TEXT_BYTES: usize = 1024 * 1024;
/// Prompt-wide text limit across all attachments.
pub const MAX_TOTAL_TEXT_BYTES: usize = 2 * 1024 * 1024;

const MAX_FILE_MEGABYTES: usize = MAX_FILE_BYTES / (1024 * 1024);
const MAX_TOTAL_MEGABYTES: usize = MAX_TOTAL_BYTES / (1024 * 1024);
const MAX_TOTAL_TEXT_MEGABYTES: usize = MAX_TOTAL_TEXT_BYTES / (1024 * 1024);
const ATTACHMENT_MARKER: &str = "openatrading-v1";
const ATTACHMENT_OPEN: &str = "<openatrading_attachments marker=\"openatrading-v1\">";
const ATTACHMENT_CLOSE: &str = "</openatrading_attachments>";
/// Guard against archives with a pathological number of entries; real Office
/// documents stay far below this.
const MAX_ZIP_ENTRIES: usize = 4096;
/// Guard against decompression bombs: XML parts larger than this are rejected
/// instead of being inflated into memory. The text extracted from one part is
/// capped separately by [`MAX_TEXT_BYTES`].
const MAX_XML_ENTRY_BYTES: usize = 8 * 1024 * 1024;
/// Excel's hard column limit; used only to bound padding from cell references.
const MAX_SHEET_COLUMNS: usize = 16_384;

/// An image supplied by a transport. `mime_type` is client metadata only; the
/// service always detects the actual type from decoded bytes.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImageAttachment {
    #[serde(default)]
    pub name: Option<String>,
    /// Standard base64 without a `data:` URL prefix.
    pub data: String,
    #[serde(default, rename = "mimeType")]
    pub mime_type: String,
}

/// A file supplied by a transport. `mime_type` and `size` are client metadata
/// only; the service detects the type from decoded bytes and computes the
/// authoritative size itself.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FileAttachment {
    pub name: String,
    /// Standard base64 without a `data:` URL prefix.
    pub data: String,
    #[serde(default, rename = "mimeType")]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

/// One `<file>` entry recorded in the prompt wrapper. Used when replaying
/// history so the UI can show attachment chips instead of prompt plumbing.
#[derive(Debug, PartialEq, Eq)]
pub struct AttachedFileRef {
    pub name: String,
    pub size: u64,
}

#[derive(Debug)]
pub struct ProcessedAttachments {
    pub images: Vec<ContentBlock>,
    pub prompt_text: String,
}

/// A transport prompt after attachment validation, ready for pi-rs.
pub struct PreparedPrompt {
    pub text: String,
    pub options: PromptOptions,
}

/// Both transports steer the active turn: a prompt submitted while a turn is
/// running is injected into that run instead of queuing a follow-up turn.
const STREAMING_BEHAVIOR_STEER: &str = "steer";

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum AttachmentError {
    #[error("一条消息最多支持 {max} 个附件")]
    TooMany { max: usize },
    #[error("「{name}」超过 {max_mb}MB")]
    FileTooLarge { name: String, max_mb: usize },
    #[error("一条消息的附件总大小不能超过 {max_mb}MB")]
    TotalTooLarge { max_mb: usize },
    #[error("一条消息的附件文本不能超过 {max_mb}MB，请减少附件或换用更小的文件")]
    TextTooLarge { max_mb: usize },
    #[error("「{name}」暂不支持这种格式（支持图片、PDF、.docx、.xlsx、.pptx 和文本文件）")]
    UnsupportedFile { name: String },
    #[error("「{name}」不是有效的文件数据")]
    InvalidFileData { name: String },
    #[error("无法读取「{name}」的内容")]
    ExtractionFailed { name: String },
    #[error("「{name}」不是有效的受支持图片")]
    InvalidImage { name: String },
    #[error("「{name}」包含二进制或危险控制字符")]
    InvalidText { name: String },
}

pub struct AttachmentService;

impl AttachmentService {
    /// Validate one prompt's attachments and build what pi-rs consumes.
    ///
    /// Both transports (WebSocket and Tauri IPC) go through this function so
    /// image normalization and the steering policy cannot drift apart.
    pub fn prepare_prompt(
        text: &str,
        images: &[ImageAttachment],
        files: &[FileAttachment],
    ) -> Result<PreparedPrompt, AttachmentError> {
        let processed = Self::process(text, images, files)?;
        Ok(PreparedPrompt {
            text: processed.prompt_text,
            options: PromptOptions {
                streaming_behavior: Some(STREAMING_BEHAVIOR_STEER.to_string()),
                images: (!processed.images.is_empty()).then_some(processed.images),
                ..Default::default()
            },
        })
    }

    /// Validate one prompt's attachments and normalize them for pi-rs.
    ///
    /// Validation is all-or-nothing: any rejected attachment rejects the
    /// whole prompt. Attachment bytes account for decoded file bytes.
    pub fn process(
        text: &str,
        images: &[ImageAttachment],
        files: &[FileAttachment],
    ) -> Result<ProcessedAttachments, AttachmentError> {
        let attachment_count = images.len().saturating_add(files.len());
        if attachment_count > MAX_ATTACHMENTS {
            return Err(AttachmentError::TooMany {
                max: MAX_ATTACHMENTS,
            });
        }
        if attachment_count == 0 {
            return Ok(ProcessedAttachments {
                images: Vec::new(),
                prompt_text: text.to_string(),
            });
        }

        let mut total_bytes = 0usize;
        let mut total_text_bytes = 0usize;
        let mut image_blocks = Vec::with_capacity(images.len());
        let mut image_hints = Vec::with_capacity(images.len());
        let mut text_entries = Vec::with_capacity(files.len());

        for (index, image) in images.iter().enumerate() {
            let fallback_name = format!("图片 {}", index + 1);
            let name = display_name(image.name.as_deref(), &fallback_name);
            // Bound the payload before allocating the decoded buffer: a client
            // must not be able to force a huge allocation with one frame.
            enforce_encoded_size(&name, &image.data)?;
            let raw = base64::engine::general_purpose::STANDARD
                .decode(image.data.as_bytes())
                .map_err(|_| AttachmentError::InvalidImage { name: name.clone() })?;
            enforce_file_size(&name, raw.len())?;
            total_bytes = checked_total(total_bytes, raw.len())?;

            let (block, hints) = normalize_image(&name, &raw)?;
            image_blocks.push(block);
            image_hints.push((name, raw.len(), hints));
        }

        for (index, file) in files.iter().enumerate() {
            let fallback_name = format!("附件 {}", index + 1);
            let name = sanitize_file_name(&file.name, &fallback_name);
            enforce_encoded_size(&name, &file.data)?;
            let raw = base64::engine::general_purpose::STANDARD
                .decode(file.data.as_bytes())
                .map_err(|_| AttachmentError::InvalidFileData { name: name.clone() })?;
            enforce_file_size(&name, raw.len())?;
            total_bytes = checked_total(total_bytes, raw.len())?;

            // The generic file channel accepts images too: a browser may hand
            // us an image with no MIME type (dropped from the file manager),
            // and pi-rs must still get a real image block rather than text.
            if pi_coding_agent::utils::mime::detect_supported_image_mime_type(&raw).is_some() {
                let (block, hints) = normalize_image(&name, &raw)?;
                image_blocks.push(block);
                image_hints.push((name, raw.len(), hints));
                continue;
            }

            let mut content = extract_text(&name, &raw)?;
            cap_text(&name, &mut content, raw.len());
            total_text_bytes = checked_total_text(total_text_bytes, content.len())?;
            if contains_binary_or_dangerous_controls(&content) {
                return Err(AttachmentError::InvalidText { name });
            }
            text_entries.push((name, raw.len(), content));
        }

        let mut wrapper = String::from("<openatrading_attachments marker=\"");
        wrapper.push_str(ATTACHMENT_MARKER);
        wrapper.push_str("\">\n");
        for (name, size, content) in text_entries {
            wrapper.push_str("  <file name=\"");
            wrapper.push_str(&escape_xml_attribute(&name));
            wrapper.push_str("\" size=\"");
            wrapper.push_str(&size.to_string());
            wrapper.push_str("\">");
            wrapper.push_str(&escape_xml_text(&content));
            wrapper.push_str("</file>\n");
        }
        for (name, size, hints) in image_hints {
            for hint in hints {
                wrapper.push_str("  <image_hint name=\"");
                wrapper.push_str(&escape_xml_attribute(&name));
                wrapper.push_str("\" size=\"");
                wrapper.push_str(&size.to_string());
                wrapper.push_str("\">");
                wrapper.push_str(&escape_xml_text(&hint));
                wrapper.push_str("</image_hint>\n");
            }
        }
        wrapper.push_str("</openatrading_attachments>");

        let prompt_text = if text.is_empty() {
            wrapper
        } else {
            format!("{text}\n\n{wrapper}")
        };

        Ok(ProcessedAttachments {
            images: image_blocks,
            prompt_text,
        })
    }
}

/// Split this service's attachment wrapper off the end of a persisted prompt.
///
/// Returns the user-visible prompt text and the `<file>` entries. Image hints
/// are dropped because images are already stored as separate content blocks.
/// A prompt without our wrapper (older sessions, plain prompts) is returned
/// unchanged.
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

        let Some(size) = rest
            .strip_prefix(" size=\"")
            .and_then(|value| value.split_once('"'))
            .and_then(|(digits, _)| digits.parse::<u64>().ok())
        else {
            continue;
        };
        entries.push(AttachedFileRef { name, size });
    }
    entries
}

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

/// Reduce a non-image attachment to plain text.
///
/// Detection is byte-based, never extension-based: clients are free to label
/// files however they like and the same document may be pasted without a name.
fn extract_text(name: &str, raw: &[u8]) -> Result<String, AttachmentError> {
    if raw.starts_with(b"%PDF-") {
        return pdf_extract::extract_text_from_mem(raw)
            .map(|text| normalize_newlines(&text))
            .map_err(|_| AttachmentError::ExtractionFailed {
                name: name.to_string(),
            });
    }
    if raw.starts_with(b"PK\x03\x04") {
        return extract_ooxml(name, raw);
    }

    let text = std::str::from_utf8(raw).map_err(|_| AttachmentError::UnsupportedFile {
        name: name.to_string(),
    })?;
    Ok(normalize_newlines(text.trim_start_matches('\u{feff}')))
}

fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn extract_ooxml(name: &str, raw: &[u8]) -> Result<String, AttachmentError> {
    let failed = || AttachmentError::ExtractionFailed {
        name: name.to_string(),
    };
    let mut archive =
        zip::ZipArchive::new(Cursor::new(raw)).map_err(|_| AttachmentError::UnsupportedFile {
            name: name.to_string(),
        })?;
    if archive.len() > MAX_ZIP_ENTRIES {
        return Err(failed());
    }

    if archive.by_name("word/document.xml").is_ok() {
        let xml = read_zip_entry(&mut archive, "word/document.xml", name)?.ok_or_else(failed)?;
        return docx_text(&xml).map_err(|_| failed());
    }
    if archive.by_name("xl/workbook.xml").is_ok() {
        return xlsx_text(&mut archive, name);
    }
    if archive.by_name("ppt/presentation.xml").is_ok() {
        return pptx_text(&mut archive, name);
    }

    Err(AttachmentError::UnsupportedFile {
        name: name.to_string(),
    })
}

fn read_zip_entry(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    entry: &str,
    name: &str,
) -> Result<Option<Vec<u8>>, AttachmentError> {
    let failed = || AttachmentError::ExtractionFailed {
        name: name.to_string(),
    };
    let mut file = match archive.by_name(entry) {
        Ok(file) => file,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(_) => return Err(failed()),
    };
    let mut bytes = Vec::new();
    file.by_ref()
        .take(MAX_XML_ENTRY_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| failed())?;
    if bytes.len() > MAX_XML_ENTRY_BYTES {
        return Err(failed());
    }
    Ok(Some(bytes))
}

fn docx_text(xml: &[u8]) -> Result<String, ()> {
    let mut out = String::new();
    let mut in_text = false;
    for_each_xml_event(xml, |event| {
        match event {
            Event::Start(e) => match e.local_name().as_ref() {
                "t" => in_text = true,
                "tab" => out.push('\t'),
                "br" => out.push('\n'),
                _ => {}
            },
            Event::Empty(e) => match e.local_name().as_ref() {
                "tab" => out.push('\t'),
                "br" => out.push('\n'),
                _ => {}
            },
            Event::End(e) => match e.local_name().as_ref() {
                "t" => in_text = false,
                "p" => out.push('\n'),
                _ => {}
            },
            Event::Text(e) if in_text => out.push_str(&decode_xml_text(&e)?),
            _ => {}
        }
        Ok(())
    })?;
    Ok(out)
}

fn pptx_text(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    name: &str,
) -> Result<String, AttachmentError> {
    let failed = || AttachmentError::ExtractionFailed {
        name: name.to_string(),
    };
    let mut slides: Vec<String> = archive
        .file_names()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.starts_with("ppt/slides/slide") && entry.ends_with(".xml"))
        .map(|entry| entry.into_owned())
        .collect();
    slides.sort_by_key(|entry| trailing_number(entry));
    if slides.is_empty() {
        return Err(failed());
    }

    let mut out = String::new();
    for (index, slide) in slides.iter().enumerate() {
        let Some(xml) = read_zip_entry(archive, slide, name)? else {
            continue;
        };
        out.push_str(&format!("## Slide {}\n", index + 1));
        let text = slide_text(&xml).map_err(|_| failed())?;
        out.push_str(&text);
    }
    Ok(out)
}

fn slide_text(xml: &[u8]) -> Result<String, ()> {
    let mut out = String::new();
    let mut in_text = false;
    for_each_xml_event(xml, |event| {
        match event {
            Event::Start(e) if e.local_name().as_ref() == "t" => in_text = true,
            Event::End(e) => match e.local_name().as_ref() {
                "t" => in_text = false,
                "p" => out.push('\n'),
                _ => {}
            },
            Event::Text(e) if in_text => out.push_str(&decode_xml_text(&e)?),
            _ => {}
        }
        Ok(())
    })?;
    Ok(out)
}

fn xlsx_text(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    name: &str,
) -> Result<String, AttachmentError> {
    let failed = || AttachmentError::ExtractionFailed {
        name: name.to_string(),
    };
    let shared = match read_zip_entry(archive, "xl/sharedStrings.xml", name)? {
        Some(xml) => shared_strings(&xml).map_err(|_| failed())?,
        None => Vec::new(),
    };

    let mut sheets: Vec<String> = archive
        .file_names()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.starts_with("xl/worksheets/sheet") && entry.ends_with(".xml"))
        .map(|entry| entry.into_owned())
        .collect();
    sheets.sort_by_key(|entry| trailing_number(entry));
    if sheets.is_empty() {
        return Err(failed());
    }

    let mut out = String::new();
    for (index, sheet) in sheets.iter().enumerate() {
        let Some(xml) = read_zip_entry(archive, sheet, name)? else {
            continue;
        };
        out.push_str(&format!("## Sheet {}\n", index + 1));
        sheet_text(&xml, &shared, &mut out).map_err(|_| failed())?;
    }
    Ok(out)
}

fn shared_strings(xml: &[u8]) -> Result<Vec<String>, ()> {
    let mut strings = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut in_text = false;
    // `<rPh>` (phonetic hints) contains `<t>` runs that are not part of the
    // cell value; skip them.
    let mut phonetic_depth = 0usize;

    for_each_xml_event(xml, |event| {
        match event {
            Event::Start(e) => match e.local_name().as_ref() {
                "si" => {
                    in_string = true;
                    current.clear();
                }
                "rPh" if in_string => phonetic_depth += 1,
                "t" if in_string && phonetic_depth == 0 => in_text = true,
                _ => {}
            },
            Event::End(e) => match e.local_name().as_ref() {
                "si" => {
                    in_string = false;
                    strings.push(std::mem::take(&mut current));
                }
                "rPh" if phonetic_depth > 0 => phonetic_depth -= 1,
                "t" => in_text = false,
                _ => {}
            },
            Event::Empty(e) if e.local_name().as_ref() == "si" => strings.push(String::new()),
            Event::Text(e) if in_text => current.push_str(&decode_xml_text(&e)?),
            _ => {}
        }
        Ok(())
    })?;
    Ok(strings)
}

fn sheet_text(xml: &[u8], shared: &[String], out: &mut String) -> Result<(), ()> {
    let mut cells: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut cell_type: Option<String> = None;
    let mut cell_column: Option<usize> = None;
    let mut in_cell = false;
    let mut in_value = false;
    let mut in_inline_text = false;

    for_each_xml_event(xml, |event| {
        match event {
            Event::Start(e) => match e.local_name().as_ref() {
                "row" => cells.clear(),
                "c" => {
                    in_cell = true;
                    current.clear();
                    cell_type = attribute(&e, "t");
                    cell_column = attribute(&e, "r").as_deref().and_then(column_index);
                }
                "v" if in_cell => in_value = true,
                "t" if in_cell => in_inline_text = true,
                _ => {}
            },
            Event::End(e) => match e.local_name().as_ref() {
                "v" => in_value = false,
                "t" => in_inline_text = false,
                "c" if in_cell => {
                    let value = resolve_cell(cell_type.as_deref(), &current, shared);
                    push_cell(&mut cells, cell_column, value);
                    in_cell = false;
                    cell_type = None;
                    cell_column = None;
                    current.clear();
                }
                "row" => {
                    while cells.last().is_some_and(|cell| cell.is_empty()) {
                        cells.pop();
                    }
                    if !cells.is_empty() {
                        out.push_str(&cells.join("\t"));
                        out.push('\n');
                    }
                }
                _ => {}
            },
            Event::Text(e) if in_value || in_inline_text => {
                current.push_str(&decode_xml_text(&e)?);
            }
            _ => {}
        }
        Ok(())
    })?;
    Ok(())
}

fn resolve_cell(cell_type: Option<&str>, raw: &str, shared: &[String]) -> String {
    match cell_type {
        Some("s") => raw
            .trim()
            .parse::<usize>()
            .ok()
            .and_then(|index| shared.get(index))
            .cloned()
            .unwrap_or_default(),
        Some("b") => match raw.trim() {
            "1" => "TRUE".to_string(),
            "0" => "FALSE".to_string(),
            other => other.to_string(),
        },
        _ => raw.to_string(),
    }
}

fn push_cell(cells: &mut Vec<String>, column: Option<usize>, value: String) {
    if let Some(column) = column.filter(|column| *column < MAX_SHEET_COLUMNS) {
        while cells.len() < column {
            cells.push(String::new());
        }
    }
    cells.push(value);
}

/// `B12` → 1 (zero-based column index).
fn column_index(reference: &str) -> Option<usize> {
    let mut index = 0usize;
    let mut seen = false;
    for byte in reference.bytes() {
        if !byte.is_ascii_alphabetic() {
            break;
        }
        seen = true;
        index = index
            .checked_mul(26)?
            .checked_add(usize::from(byte.to_ascii_uppercase() - b'A') + 1)?;
    }
    seen.then(|| index - 1)
}

/// Numeric suffix of `xl/worksheets/sheet12.xml` → 12, so sheet10 sorts after
/// sheet2 instead of lexicographically before it.
fn trailing_number(entry: &str) -> usize {
    let stem = entry.strip_suffix(".xml").unwrap_or(entry);
    let digits: String = stem
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    digits.parse().unwrap_or(usize::MAX)
}

fn for_each_xml_event<F>(xml: &[u8], mut handle: F) -> Result<(), ()>
where
    F: FnMut(Event<'_>) -> Result<(), ()>,
{
    let mut reader = Reader::from_reader(Cursor::new(xml));
    let mut buffer = Vec::new();
    // quick-xml reports EOF without complaining about unclosed elements, so a
    // truncated Office part would otherwise look like a valid (empty) document.
    let mut depth = 0usize;
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Eof) => return (depth == 0).then_some(()).ok_or(()),
            Ok(event) => {
                match &event {
                    Event::Start(_) => depth += 1,
                    Event::End(_) => depth = depth.saturating_sub(1),
                    _ => {}
                }
                handle(event)?;
            }
            Err(_) => return Err(()),
        }
        buffer.clear();
    }
}

fn decode_xml_text(text: &quick_xml::events::BytesText<'_>) -> Result<String, ()> {
    // quick-xml hands back the raw (still escaped) character data.
    quick_xml::escape::unescape(&text.xml_content(XmlVersion::Explicit1_0))
        .map(|value| value.into_owned())
        .map_err(|_| ())
}

fn attribute(element: &BytesStart<'_>, local_name: &str) -> Option<String> {
    element
        .attributes()
        .flatten()
        .find(|attribute| attribute.key.local_name().as_ref() == local_name)
        .and_then(|attribute| attribute.normalized_value(XmlVersion::Explicit1_0).ok())
        .map(|value| value.into_owned())
}

fn cap_text(name: &str, text: &mut String, original_bytes: usize) {
    if text.len() <= MAX_TEXT_BYTES {
        return;
    }
    let mut end = MAX_TEXT_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text.push_str(&format!(
        "\n\n[内容已截断：「{name}」共 {original_bytes} 字节，以上为前 {end} 字节]"
    ));
}

fn enforce_file_size(name: &str, size: usize) -> Result<(), AttachmentError> {
    if size > MAX_FILE_BYTES {
        return Err(AttachmentError::FileTooLarge {
            name: name.to_string(),
            max_mb: MAX_FILE_MEGABYTES,
        });
    }
    Ok(())
}

/// Reject oversized payloads from their base64 length before decoding.
/// Four base64 characters encode at most three bytes, so this is an upper
/// bound that never rejects a payload the size check below would accept.
fn enforce_encoded_size(name: &str, encoded: &str) -> Result<(), AttachmentError> {
    if encoded.len() / 4 * 3 > MAX_FILE_BYTES {
        return Err(AttachmentError::FileTooLarge {
            name: name.to_string(),
            max_mb: MAX_FILE_MEGABYTES,
        });
    }
    Ok(())
}

fn checked_total(current: usize, added: usize) -> Result<usize, AttachmentError> {
    let total = current
        .checked_add(added)
        .ok_or(AttachmentError::TotalTooLarge {
            max_mb: MAX_TOTAL_MEGABYTES,
        })?;
    if total > MAX_TOTAL_BYTES {
        return Err(AttachmentError::TotalTooLarge {
            max_mb: MAX_TOTAL_MEGABYTES,
        });
    }
    Ok(total)
}

fn checked_total_text(current: usize, added: usize) -> Result<usize, AttachmentError> {
    let total = current
        .checked_add(added)
        .ok_or(AttachmentError::TextTooLarge {
            max_mb: MAX_TOTAL_TEXT_MEGABYTES,
        })?;
    if total > MAX_TOTAL_TEXT_BYTES {
        return Err(AttachmentError::TextTooLarge {
            max_mb: MAX_TOTAL_TEXT_MEGABYTES,
        });
    }
    Ok(total)
}

fn display_name(name: Option<&str>, fallback: &str) -> String {
    sanitize_file_name(name.unwrap_or_default(), fallback)
}

fn sanitize_file_name(name: &str, fallback: &str) -> String {
    let basename = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let sanitized: String = basename
        .chars()
        .map(|c| if c.is_control() { '_' } else { c })
        .collect();
    let trimmed = sanitized.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

fn contains_binary_or_dangerous_controls(text: &str) -> bool {
    text.chars().any(|c| {
        !matches!(c, '\n' | '\r' | '\t')
            && (c.is_control()
                || matches!(
                    c,
                    '\u{200e}'
                        | '\u{200f}'
                        | '\u{202a}'..='\u{202e}'
                        | '\u{2066}'..='\u{2069}'
                ))
    })
}

fn escape_xml_text(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

fn escape_xml_attribute(value: &str) -> String {
    escape_xml_text(value)
}

fn unescape_xml_text(value: &str) -> String {
    // `&amp;` last: unescaping it first would turn `&amp;lt;` into `&lt;`
    // and then into `<`.
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use std::io::Write;

    use super::*;

    /// A real 1x1 PNG — the service decodes and type-detects against actual bytes.
    const PNG_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

    fn png_image(name: &str) -> ImageAttachment {
        ImageAttachment {
            name: Some(name.to_string()),
            data: PNG_BASE64.to_string(),
            mime_type: "image/png".to_string(),
        }
    }

    fn file(name: &str, bytes: &[u8]) -> FileAttachment {
        FileAttachment {
            name: name.to_string(),
            data: base64::engine::general_purpose::STANDARD.encode(bytes),
            mime_type: None,
            size: None,
        }
    }

    fn text_file(name: &str, text: &str) -> FileAttachment {
        file(name, text.as_bytes())
    }

    fn zip_with(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, bytes) in entries {
            writer.start_file(*name, options).expect("start zip entry");
            writer.write_all(bytes).expect("write zip entry");
        }
        writer.finish().expect("finish zip").into_inner()
    }

    fn docx_with(body: &str) -> Vec<u8> {
        let document = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:body>{body}</w:body></w:document>"#
        );
        zip_with(&[
            ("[Content_Types].xml", b"<Types/>"),
            ("word/document.xml", document.as_bytes()),
        ])
    }

    fn xlsx_with(shared: &str, sheet: &str) -> Vec<u8> {
        zip_with(&[
            ("[Content_Types].xml", b"<Types/>"),
            (
                "xl/workbook.xml",
                br#"<workbook><sheets><sheet name="Sheet1"/></sheets></workbook>"#,
            ),
            ("xl/sharedStrings.xml", shared.as_bytes()),
            ("xl/worksheets/sheet1.xml", sheet.as_bytes()),
        ])
    }

    fn pptx_with(slides: &[(&str, &str)]) -> Vec<u8> {
        let mut entries: Vec<(&str, &[u8])> = vec![
            ("[Content_Types].xml", b"<Types/>"),
            ("ppt/presentation.xml", b"<p:presentation xmlns:p=\"p\"/>"),
        ];
        for (name, xml) in slides {
            entries.push((name, xml.as_bytes()));
        }
        zip_with(&entries)
    }

    /// A minimal single-page PDF with a Helvetica text run.
    fn minimal_pdf(text: &str) -> Vec<u8> {
        let content = format!("BT /F1 24 Tf 72 720 Td ({text}) Tj ET");
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
             /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>"
                .to_string(),
            format!(
                "<< /Length {} >>\nstream\n{content}\nendstream",
                content.len()
            ),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
        ];

        let mut pdf = String::from("%PDF-1.4\n");
        let mut offsets = Vec::new();
        for (index, body) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.push_str(&format!("{} 0 obj\n{body}\nendobj\n", index + 1));
        }
        let xref = pdf.len();
        pdf.push_str(&format!(
            "xref\n0 {}\n0000000000 65535 f \n",
            objects.len() + 1
        ));
        for offset in offsets {
            pdf.push_str(&format!("{offset:010} 00000 n \n"));
        }
        pdf.push_str(&format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        ));
        pdf.into_bytes()
    }

    #[test]
    fn image_becomes_pi_rs_image_block() {
        let prepared =
            AttachmentService::prepare_prompt("看看这张图", &[png_image("chart.png")], &[])
                .expect("valid image must be accepted");

        assert_eq!(
            prepared.options.streaming_behavior.as_deref(),
            Some("steer")
        );
        let images = prepared.options.images.expect("image blocks");
        assert_eq!(images.len(), 1);
        match &images[0] {
            ContentBlock::Image { data, mime_type } => {
                assert_eq!(data, PNG_BASE64);
                assert_eq!(mime_type, "image/png");
            }
            other => panic!("expected image block, got {other:?}"),
        }
        assert!(prepared
            .text
            .starts_with("看看这张图\n\n<openatrading_attachments"));
    }

    #[test]
    fn text_file_is_embedded_as_escaped_xml() {
        let prepared = AttachmentService::prepare_prompt(
            "summarize",
            &[],
            &[text_file("notes.md", "<b>&</b>")],
        )
        .expect("supported text file must be accepted");

        assert!(prepared.options.images.is_none());
        assert!(prepared
            .text
            .starts_with("summarize\n\n<openatrading_attachments marker=\"openatrading-v1\">"));
        assert!(prepared
            .text
            .contains("<file name=\"notes.md\" size=\"8\">&lt;b&gt;&amp;&lt;/b&gt;</file>"));
    }

    #[test]
    fn any_utf8_file_is_accepted_regardless_of_extension() {
        let prepared = AttachmentService::prepare_prompt(
            "",
            &[],
            &[text_file("report.unknownext", "平仓\n止损")],
        )
        .expect("utf-8 bytes are text, whatever the extension says");

        assert!(prepared
            .text
            .contains("<file name=\"report.unknownext\" size=\"13\">平仓\n止损</file>"));
    }

    #[test]
    fn image_sent_through_the_file_channel_becomes_an_image_block() {
        let raw = base64::engine::general_purpose::STANDARD
            .decode(PNG_BASE64)
            .expect("valid base64");
        let prepared = AttachmentService::prepare_prompt("", &[], &[file("clip.png", &raw)])
            .expect("image bytes must be accepted");

        assert_eq!(prepared.options.images.expect("image blocks").len(), 1);
        assert!(!prepared.text.contains("<file name="));
    }

    #[test]
    fn docx_document_text_is_extracted() {
        let docx = docx_with(
            "<w:p><w:r><w:t>季度报告</w:t></w:r></w:p>\
             <w:p><w:r><w:t xml:space=\"preserve\">营收 </w:t></w:r>\
             <w:r><w:t>增长 12%</w:t></w:r></w:p>",
        );
        let prepared = AttachmentService::prepare_prompt("", &[], &[file("report.docx", &docx)])
            .expect("docx must be accepted");

        assert!(prepared.text.contains("<file name=\"report.docx\" size=\""));
        assert!(prepared.text.contains("季度报告\n营收 增长 12%\n"));
        assert!(!prepared.text.contains("w:document"));
    }

    #[test]
    fn xlsx_cells_use_shared_strings_and_keep_columns_aligned() {
        let shared = "<sst><si><t>股票</t></si><si><t>涨幅</t></si>\
                      <si><r><t>贵州</t></r><r><t>茅台</t></r></si></sst>";
        let sheet = "<worksheet><sheetData>\
            <row r=\"1\"><c r=\"A1\" t=\"s\"><v>0</v></c><c r=\"B1\" t=\"s\"><v>1</v></c></row>\
            <row r=\"2\"><c r=\"A2\" t=\"s\"><v>2</v></c><c r=\"C2\"><v>1.5</v></c></row>\
            </sheetData></worksheet>";
        let xlsx = xlsx_with(shared, sheet);
        let prepared = AttachmentService::prepare_prompt("", &[], &[file("holdings.xlsx", &xlsx)])
            .expect("xlsx must be accepted");

        assert!(prepared.text.contains("股票\t涨幅\n贵州茅台\t\t1.5\n"));
    }

    #[test]
    fn pptx_slides_are_numbered_in_numeric_order() {
        let pptx = pptx_with(&[
            (
                "ppt/slides/slide10.xml",
                "<p:sld xmlns:p=\"p\" xmlns:a=\"a\"><a:p><a:r><a:t>第十页</a:t></a:r></a:p></p:sld>",
            ),
            (
                "ppt/slides/slide2.xml",
                "<p:sld xmlns:p=\"p\" xmlns:a=\"a\"><a:p><a:r><a:t>第二页</a:t></a:r></a:p></p:sld>",
            ),
        ]);
        let prepared = AttachmentService::prepare_prompt("", &[], &[file("deck.pptx", &pptx)])
            .expect("pptx must be accepted");

        let slide_two = prepared.text.find("## Slide 1\n第二页\n").expect("slide 2");
        let slide_ten = prepared
            .text
            .find("## Slide 2\n第十页\n")
            .expect("slide 10");
        assert!(slide_two < slide_ten);
    }

    #[test]
    fn pdf_text_is_extracted() {
        let pdf = minimal_pdf("Hello PDF");
        let prepared = AttachmentService::prepare_prompt("", &[], &[file("memo.pdf", &pdf)])
            .expect("pdf must be accepted");

        assert!(prepared.text.contains("Hello PDF"));
    }

    #[test]
    fn prompt_without_attachments_is_unchanged() {
        let prepared =
            AttachmentService::prepare_prompt("hi", &[], &[]).expect("no attachments is valid");

        assert_eq!(prepared.text, "hi");
        assert!(prepared.options.images.is_none());
    }

    #[test]
    fn attachment_name_keeps_only_the_basename() {
        let prepared =
            AttachmentService::prepare_prompt("", &[], &[text_file("../../etc/notes.txt", "x")])
                .expect("path names are sanitized, not rejected");

        assert!(prepared
            .text
            .contains("<file name=\"notes.txt\" size=\"1\">x</file>"));
    }

    #[test]
    fn persisted_prompt_splits_back_into_text_and_file_entries() {
        let prepared = AttachmentService::prepare_prompt(
            "总结这两个文件",
            &[png_image("chart.png")],
            &[text_file("a.md", "alpha"), text_file("b.txt", "beta")],
        )
        .expect("prepare");
        let (visible, files) = split_attachment_wrapper(&prepared.text);

        assert_eq!(visible, "总结这两个文件");
        assert_eq!(
            files,
            vec![
                AttachedFileRef {
                    name: "a.md".to_string(),
                    size: 5
                },
                AttachedFileRef {
                    name: "b.txt".to_string(),
                    size: 4
                },
            ]
        );
    }

    #[test]
    fn prompt_without_wrapper_is_not_rewritten() {
        assert_eq!(
            split_attachment_wrapper("plain prompt"),
            ("plain prompt".to_string(), Vec::new())
        );
        let truncated = "x\n\n<openatrading_attachments marker=\"openatrading-v1\">";
        assert_eq!(
            split_attachment_wrapper(truncated),
            (truncated.to_string(), Vec::new())
        );
    }

    #[test]
    fn oversized_text_is_truncated_with_a_note() {
        let oversized = "a".repeat(MAX_TEXT_BYTES + 10);
        let prepared =
            AttachmentService::prepare_prompt("", &[], &[text_file("big.txt", &oversized)])
                .expect("oversized text is truncated, not rejected");

        assert!(prepared.text.contains("内容已截断"));
        assert!(!prepared.text.contains(&oversized));
    }

    #[test]
    fn invalid_base64_image_is_rejected() {
        let image = ImageAttachment {
            name: Some("broken.png".to_string()),
            data: "%%%not-base64%%%".to_string(),
            mime_type: "image/png".to_string(),
        };

        assert_eq!(
            AttachmentService::process("", &[image], &[]).unwrap_err(),
            AttachmentError::InvalidImage {
                name: "broken.png".to_string()
            }
        );
    }

    #[test]
    fn client_mime_type_cannot_spoof_a_non_image() {
        // Valid base64 for "hello", declared as PNG by the client.
        let image = ImageAttachment {
            name: Some("chart.png".to_string()),
            data: "aGVsbG8=".to_string(),
            mime_type: "image/png".to_string(),
        };

        assert_eq!(
            AttachmentService::process("", &[image], &[]).unwrap_err(),
            AttachmentError::InvalidImage {
                name: "chart.png".to_string()
            }
        );
    }

    #[test]
    fn invalid_base64_file_is_rejected() {
        let invalid = FileAttachment {
            name: "broken.bin".to_string(),
            data: "%not-base64%".to_string(),
            mime_type: None,
            size: None,
        };

        assert_eq!(
            AttachmentService::process("", &[], &[invalid]).unwrap_err(),
            AttachmentError::InvalidFileData {
                name: "broken.bin".to_string()
            }
        );
    }

    #[test]
    fn binary_file_is_rejected() {
        // Valid UTF-8 is required; a bare binary blob is not silently lossy.
        let binary = [0x00, 0xff, 0xfe, 0x01];
        assert_eq!(
            AttachmentService::process("", &[], &[file("payload.exe", &binary)]).unwrap_err(),
            AttachmentError::UnsupportedFile {
                name: "payload.exe".to_string()
            }
        );
    }

    #[test]
    fn unknown_zip_archive_is_rejected() {
        let archive = zip_with(&[("data.bin", b"not a document")]);
        assert_eq!(
            AttachmentService::process("", &[], &[file("bundle.zip", &archive)]).unwrap_err(),
            AttachmentError::UnsupportedFile {
                name: "bundle.zip".to_string()
            }
        );
    }

    #[test]
    fn malformed_office_document_reports_extraction_failure() {
        let broken = zip_with(&[("word/document.xml", b"<w:document><w:body>")]);
        assert_eq!(
            AttachmentService::process("", &[], &[file("broken.docx", &broken)]).unwrap_err(),
            AttachmentError::ExtractionFailed {
                name: "broken.docx".to_string()
            }
        );
    }

    #[test]
    fn decompression_bomb_is_rejected() {
        let xml = vec![b'<'; MAX_XML_ENTRY_BYTES + 1];
        let bomb = zip_with(&[("word/document.xml", &xml)]);
        assert_eq!(
            AttachmentService::process("", &[], &[file("bomb.docx", &bomb)]).unwrap_err(),
            AttachmentError::ExtractionFailed {
                name: "bomb.docx".to_string()
            }
        );
    }

    #[test]
    fn control_characters_in_text_are_rejected() {
        assert_eq!(
            AttachmentService::process("", &[], &[text_file("log.txt", "a\u{0}b")]).unwrap_err(),
            AttachmentError::InvalidText {
                name: "log.txt".to_string()
            }
        );
    }

    #[test]
    fn more_than_the_maximum_attachments_is_rejected() {
        let files: Vec<FileAttachment> = (0..=MAX_ATTACHMENTS)
            .map(|index| text_file(&format!("f{index}.txt"), "x"))
            .collect();

        assert_eq!(
            AttachmentService::process("", &[], &files).unwrap_err(),
            AttachmentError::TooMany {
                max: MAX_ATTACHMENTS
            }
        );
    }

    #[test]
    fn oversized_file_is_rejected() {
        let oversized = vec![b'a'; MAX_FILE_BYTES + 1];
        assert_eq!(
            AttachmentService::process("", &[], &[file("big.bin", &oversized)]).unwrap_err(),
            AttachmentError::FileTooLarge {
                name: "big.bin".to_string(),
                max_mb: MAX_FILE_MEGABYTES
            }
        );
    }

    #[test]
    fn encoded_payload_is_bounded_before_decoding() {
        // The encoded bound must not reject a payload the decoded check accepts.
        let at_limit = "A".repeat(MAX_FILE_BYTES / 3 * 4);
        assert_eq!(enforce_encoded_size("ok.bin", &at_limit), Ok(()));

        // One group over the limit pushes the decoded size past it; the guard
        // must reject from the encoded length alone so the decoded buffer is
        // never allocated.
        let over_limit = "A".repeat((MAX_FILE_BYTES / 3 + 1) * 4);
        assert_eq!(
            enforce_encoded_size("big.bin", &over_limit),
            Err(AttachmentError::FileTooLarge {
                name: "big.bin".to_string(),
                max_mb: MAX_FILE_MEGABYTES
            })
        );
    }
}
