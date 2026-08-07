//! 首次运行初始化：创建数据目录与默认配置模板。

use std::path::Path;

use anyhow::Result;

use crate::atrading_home;

/// 幂等初始化 `.atrading` 数据目录。
///
/// 以 `config.toml` 是否存在为准（而非目录是否存在）：`Gateway::init` 会先
/// 创建目录和 `data.db`，若按目录判断会导致 config.toml 永不生成。
/// 已存在时跳过，绝不覆盖用户配置。
///
/// 返回 `true` 表示本次创建了配置模板（首次运行）。
pub fn ensure_initialized() -> Result<bool> {
    ensure_initialized_at(&atrading_home())
}

fn ensure_initialized_at(home: &Path) -> Result<bool> {
    let config_path = home.join("config.toml");
    let created = if !config_path.exists() {
        std::fs::create_dir_all(home)?;
        std::fs::write(&config_path, include_str!("../../../config.example.toml"))?;
        true
    } else {
        false
    };
    // roles 目录始终确保存在（幂等）
    std::fs::create_dir_all(home.join("roles"))?;
    Ok(created)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_initialized_creates_config_once_and_is_idempotent() {
        let tmp = std::env::temp_dir().join(format!("atrading-init-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let home = tmp.join(".atrading");

        assert!(ensure_initialized_at(&home).unwrap_or_else(|e| panic!("init: {e}")));
        assert!(home.join("config.toml").exists(), "config.toml 应被创建");
        assert!(home.join("roles").is_dir(), "roles 目录应被创建");

        // 幂等：再次调用不覆盖已有配置
        let content = std::fs::read_to_string(home.join("config.toml"))
            .unwrap_or_else(|e| panic!("read: {e}"));
        assert!(
            !ensure_initialized_at(&home).unwrap_or_else(|e| panic!("init again: {e}")),
            "第二次调用不应再创建"
        );
        assert_eq!(
            std::fs::read_to_string(home.join("config.toml"))
                .unwrap_or_else(|e| panic!("read2: {e}")),
            content,
            "已有配置不应被覆盖"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
