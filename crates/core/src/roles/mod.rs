use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct RoleConfig {
    pub name: String,
    pub display_name: String,
    pub dir: String,
    pub merge_mode: MergeMode,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeMode {
    Supplement,
    Override,
    BaseOnly,
}

pub mod pnl;
pub mod prompt;

/// Template for zarili strategy, embedded for `cmd_init()` file creation.
/// This is the only compile-time strategy inclusion; all runtime strategies
/// are loaded from disk.
pub const ZARILI_INIT_STRATEGY: &str = include_str!("../../strategies/zarili/base.md");

pub fn default_strategies_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".atrading").join("strategies")
}

/// Display-name and metadata persisted alongside a strategy directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleMeta {
    pub display_name: String,
    pub created_at: String,
}

pub fn save_role_meta(strategies_dir: &Path, name: &str, meta: &RoleMeta) -> Result<()> {
    let path = strategies_dir.join(name).join(".meta.json");
    let json = serde_json::to_string_pretty(meta)?;
    fs::write(&path, &json).with_context(|| format!("写入 {} 失败", path.display()))?;
    Ok(())
}

pub fn load_role_meta(strategies_dir: &Path, name: &str) -> Option<RoleMeta> {
    let path = strategies_dir.join(name).join(".meta.json");
    let json = fs::read_to_string(path).ok()?;
    serde_json::from_str(&json).ok()
}

/// Scan strategies dir for directories containing both `base.md` and `.meta.json`.
/// Returns RoleEntry for each valid role found on disk.
pub fn list_roles_from_disk(strategies_dir: &Path) -> Vec<crate::RoleEntry> {
    let mut roles = Vec::new();
    let dir = match fs::read_dir(strategies_dir) {
        Ok(d) => d,
        Err(_) => return roles,
    };
    for entry in dir.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = match path.file_name() {
            Some(n) => n.to_string_lossy().to_string(),
            None => continue,
        };
        let base_md = path.join("base.md");
        if !base_md.exists() {
            continue;
        }
        let strategy = fs::read_to_string(&base_md).unwrap_or_default();
        if strategy.is_empty() {
            continue;
        }
        let display_name = load_role_meta(strategies_dir, &name)
            .map(|m| m.display_name)
            .unwrap_or_else(|| name.clone());
        roles.push(crate::RoleEntry {
            name,
            display_name,
            strategy,
            readonly: false,
        });
    }
    roles
}

fn validate_role_name(name: &str) -> Result<()> {
    if name.contains('/') || name.contains('\\') || name.contains("..") || name.is_empty() {
        anyhow::bail!("无效的角色名: {name}");
    }
    Ok(())
}

pub fn save_role_strategy(strategies_dir: &Path, name: &str, strategy: &str) -> Result<()> {
    validate_role_name(name)?;
    let role_dir = strategies_dir.join(name);
    fs::create_dir_all(&role_dir)
        .with_context(|| format!("创建目录失败: {}", role_dir.display()))?;
    fs::write(role_dir.join("base.md"), strategy)
        .with_context(|| format!("写入策略失败: {}/base.md", name))?;
    Ok(())
}

pub fn delete_role_dir(strategies_dir: &Path, name: &str) -> Result<()> {
    validate_role_name(name)?;
    let role_dir = strategies_dir.join(name);
    if role_dir.exists() {
        fs::remove_dir_all(&role_dir)
            .with_context(|| format!("删除角色目录失败: {}", role_dir.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_mode_deserialize() {
        let config: RoleConfig = toml::from_str(
            r#"name = "test"
display_name = "测试"
dir = "strategies/test"
merge_mode = "supplement"
"#,
        )
        .unwrap();
        assert!(matches!(config.merge_mode, MergeMode::Supplement));
    }
}
