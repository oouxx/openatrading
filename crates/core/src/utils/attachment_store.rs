//! 附件落盘目录：只负责「把字节放到会话目录里」和「删会话时清掉」。
//!
//! 没有元信息、没有 id、没有配额：prompt 里引用的是**绝对路径**，文件本身
//! 就是事实源（`attachment_store` 不再假装自己是存储系统）。读取一律交给
//! pi-rs 的 `read` 工具，见 [`super::attachments`]。
//!
//! 目录布局：`<数据根>/attachments/<会话 id>/<uuid>-<原始文件名>`。
//! 用可读文件名而不是 id，是为了让模型在 `read` 之后仍能理解它读的是什么，
//! 也让用户 `ls` 时看得懂。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::temp_files::ATTACHMENTS_DIR_NAME;

/// 会话 id 的最大长度（同时用作目录名，必须收窄）。
const MAX_SESSION_ID_LEN: usize = 128;

/// 附件根目录（「设置 → 缓存」统计的就是它）。
#[must_use]
pub fn attachments_root(data_dir: &Path) -> PathBuf {
    data_dir.join(ATTACHMENTS_DIR_NAME)
}

/// 会话子目录；`session_id` 不合法时返回 `None`。
#[must_use]
pub fn session_dir(data_dir: &Path, session_id: &str) -> Option<PathBuf> {
    validate_session_id(session_id)
        .ok()
        .map(|()| attachments_root(data_dir).join(session_id))
}

/// 会话 id 只允许 `[A-Za-z0-9_-]`：它直接当目录名用。
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

/// 文件名清洗：剥掉路径成分与危险字符，其余原样保留。
///
/// **不转 ASCII**：中文文件名是常态，而落盘名会出现在 prompt 里（模型靠它
/// 判断读的是什么），把"季报"抹掉是实打实的信息损失。只去掉路径分隔符、
/// 控制字符和 Windows 保留字符，保证它仍是单个安全路径成分。
#[must_use]
pub fn safe_file_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let cleaned: String = base
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .take(96)
        .collect();
    let trimmed = cleaned.trim_matches(['.', ' ']).to_string();
    if trimmed.is_empty() {
        "file".to_string()
    } else {
        trimmed
    }
}

/// 把一段字节写进会话目录，返回落盘后的绝对路径。
///
/// 先写 `.part` 再 `rename`：中途失败不会留下一个"看起来完整"却被 prompt
/// 引用的文件。
///
/// # Errors
/// 会话 id 非法、目录创建或写入失败时返回。
pub fn spool_bytes(
    data_dir: &Path,
    session_id: &str,
    name: &str,
    bytes: &[u8],
) -> io::Result<PathBuf> {
    let dir = session_dir(data_dir, session_id)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "会话 id 不合法"))?;
    fs::create_dir_all(&dir)?;
    let id = uuid::Uuid::new_v4();
    let final_path = dir.join(format!("{id}-{}", safe_file_name(name)));
    let part = dir.join(format!("{id}.part"));
    fs::write(&part, bytes)?;
    fs::rename(&part, &final_path).map_err(|e| {
        let _ = fs::remove_file(&part);
        e
    })?;
    Ok(final_path)
}

/// 按扩展名粗判 MIME：只用于 UI 分类，真正的类型由 prompt 时的字节判定。
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
        _ => "application/octet-stream",
    }
}

/// 「设置 → 缓存」的统计：文件数 + 磁盘占用（`.part` 不算附件）。
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
            usage.bytes = usage
                .bytes
                .saturating_add(entry.metadata().map_or(0, |m| m.len()));
            let is_part = path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("part"));
            if !is_part {
                usage.files = usage.files.saturating_add(1);
            }
        }
    }
    usage
}

/// 删掉一个会话的附件目录（删会话时调用）。
#[must_use]
pub fn remove_session(data_dir: &Path, session_id: &str) -> bool {
    let Some(dir) = session_dir(data_dir, session_id) else {
        return false;
    };
    if !super::temp_files::is_inside_dir(&dir, &attachments_root(data_dir)) {
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
        for ok in ["abc", "sess-1", "A_b-9"] {
            assert!(validate_session_id(ok).is_ok(), "{ok} 应合法");
        }
        for bad in ["", "../etc", "a/b", "a b", "中文", ".hidden"] {
            assert!(validate_session_id(bad).is_err(), "{bad} 应被拒");
        }
        assert!(session_dir(Path::new("/data"), "../x").is_none());
    }

    #[test]
    fn spooled_files_keep_a_readable_name_and_no_part_leftovers() {
        let root = scratch("spool");
        let path = spool_bytes(&root, "sess-1", "/Users/me/季报 2026.pdf", b"%PDF-1.4 x")
            .unwrap_or_else(|e| panic!("spool: {e}"));

        assert!(path.starts_with(attachments_root(&root).join("sess-1")));
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        assert!(name.ends_with("季报 2026.pdf"), "文件名应可读：{name}");
        assert_eq!(
            fs::read(&path).unwrap_or_else(|e| panic!("read: {e}")),
            b"%PDF-1.4 x"
        );

        let leftovers: Vec<_> = fs::read_dir(path.parent().unwrap_or(&root))
            .unwrap_or_else(|e| panic!("readdir: {e}"))
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "part"))
            .collect();
        assert!(leftovers.is_empty(), "不应留下 .part");

        // `%PDF-1.4 x` 是 10 字节。
        assert_eq!(dir_usage(&attachments_root(&root)), DirUsage::new(1, 10));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn unsafe_file_names_are_reduced_to_one_safe_component() {
        assert_eq!(safe_file_name("../../etc/passwd"), "passwd");
        assert_eq!(safe_file_name("a\u{0}b.txt"), "ab.txt");
        assert_eq!(safe_file_name("   "), "file");
        assert_eq!(safe_file_name("..."), "file");
        // 中文保留：落盘名会出现在 prompt 里，模型靠它判断读的是什么。
        assert_eq!(safe_file_name("季报 2026.pdf"), "季报 2026.pdf");
        assert_eq!(safe_file_name("a:b*c.pdf"), "abc.pdf");
        assert_eq!(safe_file_name(&"长".repeat(200)).chars().count(), 96);
    }

    #[test]
    fn part_files_are_counted_in_bytes_but_not_as_attachments() {
        let root = scratch("usage");
        let dir = session_dir(&root, "sess-1").unwrap_or_else(|| panic!("dir"));
        fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("mkdir: {e}"));
        fs::write(dir.join("a-uuid-report.pdf"), vec![0u8; 10])
            .unwrap_or_else(|e| panic!("w: {e}"));
        fs::write(dir.join("b.part"), vec![0u8; 5]).unwrap_or_else(|e| panic!("w: {e}"));

        let usage = dir_usage(&attachments_root(&root));
        assert_eq!(usage.files, 1, "半成品不算附件");
        assert_eq!(usage.bytes, 15, "但占的磁盘要算进去");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn removing_a_session_drops_only_that_session() {
        let root = scratch("remove");
        let a = session_dir(&root, "sess-a").unwrap_or_else(|| panic!("dir"));
        let b = session_dir(&root, "sess-b").unwrap_or_else(|| panic!("dir"));
        fs::create_dir_all(&a).unwrap_or_else(|e| panic!("mkdir: {e}"));
        fs::create_dir_all(&b).unwrap_or_else(|e| panic!("mkdir: {e}"));

        assert!(remove_session(&root, "sess-a"));
        assert!(!a.exists());
        assert!(b.exists(), "别的会话不能被牵连");

        assert!(!remove_session(&root, "../etc"));
        assert!(!remove_session(&root, "sess-a"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn mime_guessing_is_best_effort_only() {
        assert_eq!(guess_mime_from_name("a.PNG"), "image/png");
        assert_eq!(guess_mime_from_name("a.pdf"), "application/pdf");
        assert_eq!(guess_mime_from_name("noext"), "application/octet-stream");
    }
}
