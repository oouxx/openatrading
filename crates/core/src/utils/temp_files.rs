//! 品牌根下的临时缓存目录布局、用量统计与清理。
//!
//! 这里只处理**可随时重新生成的临时数据**：
//!
//! - `attachments/`：附件落盘缓存（大文件先落盘再解析，避免把 base64 塞进单帧）
//! - `tmp/`：工具与解析过程产生的中间文件
//!
//! 会话记录（`agent/`）、角色（`roles/`）、模拟盘（`data.db`）**不属于缓存**，
//! 清理逻辑绝不碰它们：那是用户数据，删掉不可恢复。

use std::fs;
use std::path::{Path, PathBuf};

use crate::atrading_home;

/// 附件缓存目录名：`<data_dir>/attachments`。
pub const ATTACHMENTS_DIR_NAME: &str = "attachments";
/// 通用临时目录名：`<data_dir>/tmp`。
pub const TEMP_DIR_NAME: &str = "tmp";

/// 附件落盘目录。
#[must_use]
pub fn attachments_dir() -> PathBuf {
    atrading_home().join(ATTACHMENTS_DIR_NAME)
}

/// 通用临时目录。
#[must_use]
pub fn temp_dir() -> PathBuf {
    atrading_home().join(TEMP_DIR_NAME)
}

/// 一个目录的用量：常规文件数量与字节数（递归，不跟随符号链接）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DirUsage {
    pub files: u64,
    pub bytes: u64,
}

impl DirUsage {
    #[must_use]
    pub const fn new(files: u64, bytes: u64) -> Self {
        Self { files, bytes }
    }

    pub const fn add(&mut self, other: Self) {
        self.files = self.files.saturating_add(other.files);
        self.bytes = self.bytes.saturating_add(other.bytes);
    }
}

/// 目标是否位于数据根之内（且不等于根本身）。
///
/// 清理是不可逆操作，调用方传进来的路径必须先过这一关：即便将来有人把
/// 目录来源改成配置项，也不会因为一份写错的配置删到数据根之外。
///
/// 用逻辑比较而不是 `canonicalize`：两侧都可能尚未创建，canonicalize 对
/// 不存在的路径会失败，反而让保护失效。
#[must_use]
pub fn is_inside_dir(path: &Path, root: &Path) -> bool {
    path.starts_with(root) && path != root
}

/// 统计目录用量。
///
/// 单项读取失败（无权限、统计过程中被并发删除）直接跳过：这是一个展示型
/// 接口，不该因为一个坏条目整体失败。
#[must_use]
pub fn dir_usage(dir: &Path) -> DirUsage {
    let mut usage = DirUsage::default();
    let Ok(entries) = fs::read_dir(dir) else {
        return usage;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            // 符号链接指向目录时 `file_type` 是 symlink，不会走这里，避免绕出去。
            usage.add(dir_usage(&entry.path()));
        } else if file_type.is_file() {
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            usage.files = usage.files.saturating_add(1);
            usage.bytes = usage.bytes.saturating_add(meta.len());
        }
    }
    usage
}

/// 清理结果。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClearedDir {
    /// 被删除的内容（清理前统计）。
    pub removed: DirUsage,
    /// 删除失败的原因，供 UI 原样展示。
    pub errors: Vec<String>,
}

/// 删除目录下的全部内容，保留目录本身。
///
/// `root` 是允许清理的数据根：`dir` 必须落在它内部（见 [`is_inside_dir`]）。
/// 目录不存在时视为"已经干净"，返回零值而不是错误：用户点清理时缓存目录
/// 通常还没被创建，那不该报错。
///
/// 符号链接只删链接本身（`remove_file`），不跟进目标目录。
pub fn clear_dir(dir: &Path, root: &Path) -> ClearedDir {
    clear_dir_skipping(dir, root, &std::collections::HashSet::new())
}

/// 同 [`clear_dir`]，但跳过 `skip` 里列出的路径。
///
/// 用途：清理缓存时正好有 prompt 在读某个附件（在途），删掉它会让那一轮直接
/// 失败。跳过的条目会在 `errors` 里留一句说明——静默保留比静默删除更容易解释。
pub fn clear_dir_skipping(
    dir: &Path,
    root: &Path,
    skip: &std::collections::HashSet<PathBuf>,
) -> ClearedDir {
    let mut result = ClearedDir::default();
    if !is_inside_dir(dir, root) {
        result
            .errors
            .push(format!("拒绝清理数据根之外的目录：{}", dir.display()));
        return result;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return result;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if skip.contains(&path) {
            result
                .errors
                .push(format!("{} 正在使用中，已跳过", path.display()));
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            result.errors.push(format!("无法读取 {}", path.display()));
            continue;
        };
        // 先统计再删：删除之后就量不到了。
        let removed = if file_type.is_dir() {
            let usage = dir_usage(&path);
            // 子目录里有在途文件就逐个删，不能整棵拔掉。
            let has_skipped = skip.iter().any(|skipped| skipped.starts_with(&path));
            if has_skipped {
                let nested = clear_dir_skipping(&path, root, skip);
                result.errors.extend(nested.errors);
                result.removed.add(nested.removed);
                continue;
            }
            match fs::remove_dir_all(&path) {
                Ok(()) => usage,
                Err(e) => {
                    result.errors.push(format!("{}: {e}", path.display()));
                    DirUsage::default()
                }
            }
        } else {
            let bytes = entry.metadata().map_or(0, |m| m.len());
            match fs::remove_file(&path) {
                Ok(()) => DirUsage::new(1, bytes),
                Err(e) => {
                    result.errors.push(format!("{}: {e}", path.display()));
                    DirUsage::default()
                }
            }
        };
        result.removed.add(removed);
    }
    result
}

/// 上次清理的记录文件名（位于数据根下，不属于任何缓存目录，清理时不会自删）。
pub const CACHE_STATE_FILE_NAME: &str = "cache-state.json";

/// 上次手动清理的记录。
///
/// 放盘上而不是内存：用户想知道的正是「上次是什么时候清的」，进程重启就忘
/// 等于没记。文件损坏/不存在一律当作「从未清理」，不让统计接口因此失败。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheState {
    /// Unix 毫秒时间戳；`None` 表示从未清理。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_cleared_at: Option<i64>,
    /// 上次释放的字节数。
    #[serde(default)]
    pub last_freed_bytes: u64,
}

/// 读取清理记录；缺失或解析失败时返回默认值。
#[must_use]
pub fn load_cache_state(root: &Path) -> CacheState {
    fs::read(root.join(CACHE_STATE_FILE_NAME))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// 写入清理记录。写失败不致命：记录只是提示信息，不该让清理本身报错。
pub fn save_cache_state(root: &Path, state: &CacheState) -> std::io::Result<()> {
    let path = root.join(CACHE_STATE_FILE_NAME);
    let bytes = serde_json::to_vec_pretty(state)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(path, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("atrading-tempfiles-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("mkdir: {e}"));
        dir
    }

    #[test]
    fn usage_counts_nested_files_and_clear_keeps_the_dir() {
        let root = scratch("usage");
        let nested = root.join("sess");
        fs::create_dir_all(&nested).unwrap_or_else(|e| panic!("mkdir: {e}"));
        fs::write(root.join("a.bin"), vec![0u8; 10]).unwrap_or_else(|e| panic!("write: {e}"));
        fs::write(nested.join("b.bin"), vec![0u8; 32]).unwrap_or_else(|e| panic!("write: {e}"));

        let usage = dir_usage(&root);
        assert_eq!(usage, DirUsage::new(2, 42));

        // 数据根 = 系统临时目录，缓存目录是它的子目录（与品牌根的真实关系一致）。
        let cleared = clear_dir(&root, &std::env::temp_dir());
        assert_eq!(cleared.removed, DirUsage::new(2, 42));
        assert!(cleared.errors.is_empty(), "{}", cleared.errors.join("; "));
        assert!(root.is_dir(), "目录本身要保留");
        assert_eq!(dir_usage(&root), DirUsage::default());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_dir_is_clean_not_an_error() {
        let root = scratch("missing");
        let gone = root.join("never-created");
        assert_eq!(dir_usage(&gone), DirUsage::default());
        let cleared = clear_dir(&gone, &root);
        assert_eq!(cleared, ClearedDir::default());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn refuses_to_clear_outside_the_data_dir() {
        let root = scratch("outside");
        fs::write(root.join("keep.bin"), b"do not delete").unwrap_or_else(|e| panic!("write: {e}"));

        let data_dir = atrading_home();
        let cleared = clear_dir(&root, &data_dir);

        assert_eq!(cleared.removed, DirUsage::default());
        assert_eq!(cleared.errors.len(), 1);
        assert!(root.join("keep.bin").exists(), "品牌根之外的文件不能被删除");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn skipping_in_flight_files_keeps_them_and_reports_why() {
        let root = scratch("skip");
        let busy = root.join("busy");
        let idle = root.join("idle");
        fs::create_dir_all(&busy).unwrap_or_else(|e| panic!("mkdir: {e}"));
        fs::write(busy.join("in-use.bin"), vec![0u8; 10]).unwrap_or_else(|e| panic!("write: {e}"));
        fs::write(busy.join("done.bin"), vec![0u8; 20]).unwrap_or_else(|e| panic!("write: {e}"));
        fs::create_dir_all(&idle).unwrap_or_else(|e| panic!("mkdir: {e}"));
        fs::write(idle.join("x.bin"), vec![0u8; 5]).unwrap_or_else(|e| panic!("write: {e}"));

        let mut skip = std::collections::HashSet::new();
        skip.insert(busy.join("in-use.bin"));

        let cleared = clear_dir_skipping(&root, &std::env::temp_dir(), &skip);

        assert!(busy.join("in-use.bin").exists(), "在途文件必须保留");
        assert!(!busy.join("done.bin").exists(), "同目录的其他文件照删");
        assert!(!idle.join("x.bin").exists());
        assert_eq!(cleared.removed, DirUsage::new(2, 25));
        assert!(
            cleared.errors.iter().any(|e| e.contains("正在使用中")),
            "跳过的原因要能解释给用户：{:?}",
            cleared.errors
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn cache_state_round_trips_and_survives_garbage() {
        let root = scratch("state");
        assert_eq!(
            load_cache_state(&root),
            CacheState::default(),
            "从未清理时是默认值"
        );

        let state = CacheState {
            last_cleared_at: Some(1_700_000_000_000),
            last_freed_bytes: 5003,
        };
        save_cache_state(&root, &state).unwrap_or_else(|e| panic!("save: {e}"));
        assert_eq!(load_cache_state(&root), state);

        // 文件被写坏时退回默认值，不把统计接口带下水。
        fs::write(root.join(CACHE_STATE_FILE_NAME), b"not json")
            .unwrap_or_else(|e| panic!("write: {e}"));
        assert_eq!(load_cache_state(&root), CacheState::default());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn clearing_dirs_leaves_the_state_file_alone() {
        let root = scratch("state-keep");
        let cache = root.join(TEMP_DIR_NAME);
        fs::create_dir_all(&cache).unwrap_or_else(|e| panic!("mkdir: {e}"));
        fs::write(cache.join("a.bin"), b"x").unwrap_or_else(|e| panic!("write: {e}"));
        let state = CacheState {
            last_cleared_at: Some(42),
            last_freed_bytes: 1,
        };
        save_cache_state(&root, &state).unwrap_or_else(|e| panic!("save: {e}"));

        let cleared = clear_dir(&cache, &root);
        assert_eq!(cleared.removed, DirUsage::new(1, 1));

        // 记录文件在数据根、不在缓存目录里，清理不应当把它一起删了。
        assert_eq!(load_cache_state(&root), state);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn data_dir_guard_rejects_the_root_itself() {
        let root = atrading_home();
        assert!(!is_inside_dir(&root, &root), "数据根本身不可作为清理目标");
        assert!(is_inside_dir(&root.join(TEMP_DIR_NAME), &root));
        assert!(is_inside_dir(&root.join(ATTACHMENTS_DIR_NAME), &root));
        assert!(!is_inside_dir(Path::new("/tmp/elsewhere"), &root));
    }

    #[test]
    fn clear_refuses_a_sibling_of_the_allowed_root() {
        let root = scratch("sibling");
        let cache = root.join(TEMP_DIR_NAME);
        fs::create_dir_all(&cache).unwrap_or_else(|e| panic!("mkdir: {e}"));
        fs::write(cache.join("a.bin"), b"x").unwrap_or_else(|e| panic!("write: {e}"));

        // 用另一个根去清同一路径：必须被拒。
        let other_root = root.join("other-data");
        let cleared = clear_dir(&cache, &other_root);

        assert_eq!(cleared.removed, DirUsage::default());
        assert_eq!(cleared.errors.len(), 1);
        assert!(cache.join("a.bin").exists());

        let _ = fs::remove_dir_all(&root);
    }
}
