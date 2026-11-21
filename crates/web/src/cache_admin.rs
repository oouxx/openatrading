//! 设置页「缓存」：临时缓存的用量统计与手动清理。
//!
//! 覆盖三类可随时重新生成的数据：
//!
//! 1. 行情/舆情响应缓存（内存 moka，进程内唯一）
//! 2. 附件落盘缓存（`<data_dir>/attachments`）
//! 3. 工具临时文件（`<data_dir>/tmp`）
//!
//! 会话记录、角色、模拟盘数据**不在这里**：那是用户数据，删了不可恢复。
//!
//! 响应拼装与目录清理都写成不依赖 [`AppState`] 的纯函数
//! （[`stats_payload`] / [`clear_targets`]）：`AppState` 需要 `AppContext` 与
//! 三个 Store 才能构造，把它们混进来会让这两段逻辑只能靠手测。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::{extract::State, Json};
use serde_json::{json, Value};

use openatrading_core::log;
use openatrading_core::utils::attachment_store;

use crate::in_flight;
use openatrading_core::utils::temp_files::{self, CacheState, DirUsage};

use crate::AppState;

/// 一个待清理的缓存目录。
#[derive(Debug, Clone)]
pub struct CacheTarget {
    /// 展示用的短名（`attachments` / `temp`）。
    pub name: &'static str,
    pub path: PathBuf,
    /// 统计口径。附件目录要排除元信息/半成品，普通临时目录直接量磁盘。
    counter: fn(&Path) -> DirUsage,
}

/// 数据根下的缓存目录表。顺序即响应里的展示顺序。
#[must_use]
pub fn cache_targets(data_dir: &Path) -> Vec<CacheTarget> {
    vec![
        CacheTarget {
            name: "attachments",
            path: data_dir.join(temp_files::ATTACHMENTS_DIR_NAME),
            counter: attachment_store::dir_usage,
        },
        CacheTarget {
            name: "temp",
            path: data_dir.join(temp_files::TEMP_DIR_NAME),
            counter: temp_files::dir_usage,
        },
    ]
}

fn dir_json(usage: DirUsage, dir: &Path) -> Value {
    json!({
        "files": usage.files,
        "bytes": usage.bytes,
        "path": dir.display().to_string(),
    })
}

/// 拼装 `GET /api/cache/stats` 的响应。
#[must_use]
pub fn stats_payload(
    http_entries: u64,
    http_bytes: u64,
    targets: &[CacheTarget],
    state: CacheState,
) -> Value {
    let mut total = http_bytes;
    let mut by_name = serde_json::Map::new();
    for target in targets {
        let usage = (target.counter)(&target.path);
        total = total.saturating_add(usage.bytes);
        by_name.insert(target.name.to_string(), dir_json(usage, &target.path));
    }
    let attachments = by_name
        .shift_remove("attachments")
        .unwrap_or_else(|| json!(null));
    let temp = by_name.shift_remove("temp").unwrap_or_else(|| json!(null));

    json!({
        "httpCache": { "entries": http_entries, "bytes": http_bytes },
        "attachments": attachments,
        "temp": temp,
        "totalBytes": total,
        "lastClearedAt": state.last_cleared_at,
        "lastFreedBytes": state.last_freed_bytes,
    })
}

/// 一次清理的结果。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClearedTargets {
    pub removed: DirUsage,
    pub errors: Vec<String>,
    /// 每个目录清掉了多少（响应里的 `dirs`）。
    pub per_dir: Vec<(&'static str, PathBuf, DirUsage)>,
}

/// 清空全部缓存目录。逐目录尽力而为，失败项进 `errors`。
pub fn clear_targets(data_dir: &Path, targets: &[CacheTarget]) -> ClearedTargets {
    let mut outcome = ClearedTargets::default();
    for target in targets {
        // 清理前后各量一次，差值才是真实释放量：中途删不掉的部分不会被
        // 谎报成"已释放"，口径也和 [stats_payload] 完全一致（附件不算 .json）。
        let before = (target.counter)(&target.path);
        // 正在被某个 prompt 读的附件不能删：删了那一轮会直接失败。
        let cleared =
            temp_files::clear_dir_skipping(&target.path, data_dir, &in_flight::in_flight_paths());
        let after = (target.counter)(&target.path);
        let delta = DirUsage::new(
            before.files.saturating_sub(after.files),
            before.bytes.saturating_sub(after.bytes),
        );

        outcome.removed.add(delta);
        outcome.errors.extend(cleared.errors);
        if delta.files > 0 {
            log::info!(
                "cache: 清理 {} 目录 {} 个文件（{} 字节）",
                target.name,
                delta.files,
                delta.bytes
            );
        }
        outcome
            .per_dir
            .push((target.name, target.path.clone(), delta));
    }
    outcome
}

/// 内存缓存的当前占用：条目数与载荷字节数。
fn http_cache_usage(state: &AppState) -> (u64, u64) {
    let entries = state.cache.entry_count();
    let bytes = state
        .cache
        .iter()
        .map(|(_, entry)| entry.data.len() as u64)
        .fold(0u64, u64::saturating_add);
    (entries, bytes)
}

/// `GET /api/cache/stats` — 各类临时缓存的当前用量与上次清理记录。
pub async fn get_cache_stats(State(state): State<Arc<AppState>>) -> Json<Value> {
    let (entries, bytes) = http_cache_usage(&state);
    let data_dir = openatrading_core::atrading_home();
    Json(stats_payload(
        entries,
        bytes,
        &cache_targets(&data_dir),
        temp_files::load_cache_state(&data_dir),
    ))
}

/// `POST /api/cache/clear` — 清空全部临时缓存，返回释放量。
///
/// 语义是"尽力而为"：某一项删不掉（占用中、无权限）不影响其他项，失败原因
/// 原样放进 `errors` 交给 UI 展示——静默失败会让用户以为清干净了。
pub async fn clear_cache(State(state): State<Arc<AppState>>) -> Json<Value> {
    let (entries, http_bytes) = http_cache_usage(&state);
    state.cache.invalidate_all();
    // invalidate_all 只是标记，跑一次 pending tasks 才算真正释放。
    state.cache.run_pending_tasks();
    log::info!("cache: 清理内存缓存 {entries} 条（{} 字节）", http_bytes);

    let data_dir = openatrading_core::atrading_home();
    let outcome = clear_targets(&data_dir, &cache_targets(&data_dir));
    let freed = http_bytes.saturating_add(outcome.removed.bytes);

    // 记录「上次清理」：只在真的动过东西时写，避免用户每次刷新都被覆盖成
    // 一个"刚清理过、释放 0 字节"的假记录。
    let now_ms = openatrading_core::utils::time::beijing_now().timestamp_millis();
    let touched = freed > 0 || outcome.removed.files > 0 || entries > 0;
    let mut saved_at = None;
    if touched {
        let cache_state = CacheState {
            last_cleared_at: Some(now_ms),
            last_freed_bytes: freed,
        };
        match temp_files::save_cache_state(&data_dir, &cache_state) {
            Ok(()) => saved_at = Some(now_ms),
            Err(e) => log::warn!("cache: 写入清理记录失败: {e}"),
        }
    }

    let dirs: Vec<Value> = outcome
        .per_dir
        .iter()
        .map(|(name, path, usage)| {
            json!({
                "name": name,
                "path": path.display().to_string(),
                "files": usage.files,
                "bytes": usage.bytes,
            })
        })
        .collect();

    Json(json!({
        "httpCacheEntries": entries,
        "removedFiles": outcome.removed.files,
        "freedBytes": freed,
        "dirs": dirs,
        "errors": outcome.errors,
        "lastClearedAt": saved_at,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("atrading-cacheadmin-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("mkdir: {e}"));
        dir
    }

    fn seed(targets: &[CacheTarget]) {
        for (target, files) in targets.iter().zip([2usize, 1usize]) {
            std::fs::create_dir_all(target.path.join("sess"))
                .unwrap_or_else(|e| panic!("mkdir: {e}"));
            for i in 0..files {
                std::fs::write(
                    target.path.join("sess").join(format!("{i}.bin")),
                    vec![0u8; 16],
                )
                .unwrap_or_else(|e| panic!("write: {e}"));
            }
        }
    }

    #[test]
    fn stats_reports_every_dir_and_the_totals() {
        let root = scratch("stats");
        let targets = cache_targets(&root);
        seed(&targets);

        let payload = stats_payload(
            7,
            1024,
            &targets,
            CacheState {
                last_cleared_at: Some(1_700_000_000_000),
                last_freed_bytes: 4096,
            },
        );

        assert_eq!(payload["httpCache"]["entries"], 7);
        assert_eq!(payload["httpCache"]["bytes"], 1024);
        assert_eq!(payload["attachments"]["files"], 2);
        assert_eq!(payload["attachments"]["bytes"], 32);
        assert_eq!(payload["temp"]["files"], 1);
        assert_eq!(payload["temp"]["bytes"], 16);
        // 合计把内存缓存也算进去，否则 UI 的「合计」少一块。
        assert_eq!(payload["totalBytes"], 1024 + 32 + 16);
        assert_eq!(payload["lastClearedAt"], 1_700_000_000_000u64);
        assert_eq!(payload["lastFreedBytes"], 4096);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn stats_on_missing_dirs_reports_zero_not_null() {
        let root = scratch("stats-missing");

        let payload = stats_payload(0, 0, &cache_targets(&root), CacheState::default());

        assert_eq!(payload["attachments"]["files"], 0);
        assert_eq!(payload["temp"]["bytes"], 0);
        assert_eq!(payload["totalBytes"], 0);
        assert!(payload["lastClearedAt"].is_null(), "从未清理时为空");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn clear_removes_dir_contents_and_reports_per_dir() {
        let root = scratch("clear");
        let targets = cache_targets(&root);
        seed(&targets);

        let outcome = clear_targets(&root, &targets);

        assert_eq!(outcome.removed, DirUsage::new(3, 48));
        assert!(outcome.errors.is_empty(), "{:?}", outcome.errors);
        assert_eq!(
            outcome
                .per_dir
                .iter()
                .map(|(name, _, usage)| (*name, usage.files))
                .collect::<Vec<_>>(),
            vec![("attachments", 2), ("temp", 1)]
        );
        // 目录保留，下一轮写缓存不用再建。
        for target in &targets {
            assert!(target.path.is_dir(), "{} 应保留", target.path.display());
            assert_eq!(temp_files::dir_usage(&target.path), DirUsage::default());
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn clear_is_idempotent_on_an_already_empty_data_dir() {
        let root = scratch("clear-empty");
        let targets = cache_targets(&root);

        let first = clear_targets(&root, &targets);
        let second = clear_targets(&root, &targets);

        assert_eq!(first.removed, DirUsage::default());
        assert_eq!(second, first);
        assert!(second.errors.is_empty());

        let _ = std::fs::remove_dir_all(&root);
    }
}
