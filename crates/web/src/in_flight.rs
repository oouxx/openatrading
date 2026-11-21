//! 附件在途登记表。
//!
//! `prepare_prompt` 读盘解析附件期间，文件正在被使用。此时用户点「清除缓存」
//! 如果把它删了，那一轮 prompt 会直接失败，而用户毫无线索。所以：
//!
//! - 读盘前把 `<session>/<id>` 路径登记进来；
//! - 解析结束（成功或失败）自动注销（[`InFlightGuard`] 的 Drop）；
//! - 清理缓存时把这些路径跳过，并在 `errors` 里说明。
//!
//! 进程内全局单例：注册与查询分别发生在 WS/Tauri 两条路径和缓存清理里，
//! 没有更好的共同宿主。

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

fn registry() -> &'static Mutex<HashSet<PathBuf>> {
    static REGISTRY: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashSet::new()))
}

/// 在途路径的快照。清理缓存时用它决定跳过谁。
#[must_use]
pub fn in_flight_paths() -> HashSet<PathBuf> {
    registry()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

/// 把一批附件路径登记为"在途"，Drop 时自动注销。
///
/// 有 guard 就不用到处写 `unregister`：`prepare_prompt` 有多个提前返回的分支。
pub struct InFlightGuard {
    paths: Vec<PathBuf>,
}

impl InFlightGuard {
    /// 直接按绝对路径登记（prompt 里引用的就是路径）。
    #[must_use]
    pub fn register_paths(paths: impl IntoIterator<Item = String>) -> Self {
        let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
        if !paths.is_empty() {
            let mut guard = registry()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for path in &paths {
                guard.insert(path.clone());
            }
        }
        Self { paths }
    }

    /// 空 guard（没有落盘附件时用）。
    #[must_use]
    pub fn empty() -> Self {
        Self { paths: Vec::new() }
    }
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        if self.paths.is_empty() {
            return;
        }
        let mut guard = registry()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for path in &self.paths {
            guard.remove(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// 登记表是进程级单例，同进程内多个测试会并发读写同一份状态：
    /// 断言只看自己登记的那几条路径，不对总体集合做"必须为空"的假设。
    fn is_tracked(path: &str) -> bool {
        in_flight_paths().contains(Path::new(path))
    }

    #[test]
    fn paths_are_tracked_only_while_the_guard_lives() {
        assert!(
            !is_tracked("/data/attachments/sess-single/a"),
            "初始不该在册"
        );

        {
            let _guard = InFlightGuard::register_paths([
                "/data/attachments/sess-single/a".to_string(),
                "/data/attachments/sess-single/b".to_string(),
            ]);
            assert!(is_tracked("/data/attachments/sess-single/a"));
            assert!(is_tracked("/data/attachments/sess-single/b"));
        }

        assert!(
            !is_tracked("/data/attachments/sess-single/a"),
            "guard 释放后应注销"
        );
        assert!(!is_tracked("/data/attachments/sess-single/b"));
    }

    #[test]
    fn empty_guard_touches_nothing() {
        let before = in_flight_paths();
        let _guard = InFlightGuard::empty();
        assert_eq!(in_flight_paths(), before);
    }

    /// 多个 prompt 并行解析同一附件时，先结束的那个不能把另一个的登记删掉。
    #[test]
    fn nested_registrations_do_not_cancel_each_other() {
        let outer = InFlightGuard::register_paths(["/data/attachments/sess-nested/a".to_string()]);
        let inner = InFlightGuard::register_paths(["/data/attachments/sess-nested/b".to_string()]);
        drop(inner);
        assert!(
            is_tracked("/data/attachments/sess-nested/a"),
            "外层登记不能被内层释放掉"
        );
        assert!(!is_tracked("/data/attachments/sess-nested/b"));
        drop(outer);
        assert!(!is_tracked("/data/attachments/sess-nested/a"));
    }
}
