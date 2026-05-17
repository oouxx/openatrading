use anyhow::{Context, Result};
use serde::Deserialize;
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

pub mod prompt;
pub mod role;

pub const BUILTIN_STRATEGIES: &[(&str, &str, &str)] = &[(
    "zarili",
    "zarili量价派",
    include_str!("../../strategies/zarili/base.md"),
)];

pub fn get_builtin_strategy(name: &str) -> Option<&'static str> {
    BUILTIN_STRATEGIES
        .iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, _, content)| *content)
}

pub fn default_strategies_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".atrading").join("strategies")
}

/// Returns (name, display_name, strategy) for each user role on disk
pub fn list_user_roles(strategies_dir: &Path) -> Vec<(String, String, String)> {
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
        let base_md = path.join("base.md");
        if !base_md.exists() {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let strategy = fs::read_to_string(&base_md).unwrap_or_default();
        roles.push((name.clone(), name, strategy));
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
    fn test_get_builtin_strategy_not_found() {
        assert!(get_builtin_strategy("nonexistent").is_none());
    }

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
