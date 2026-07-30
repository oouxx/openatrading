use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// A role declared in `config.toml` `[[roles]]`. Readonly roles cannot be
/// edited or deleted via the API.
#[derive(Debug, Clone, Deserialize)]
pub struct RoleConfig {
    pub name: String,
    /// Human-readable label shown in the UI. Falls back to `name` when absent.
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub readonly: bool,
}

pub mod pnl;
pub mod prompt;

/// Path to the roles directory (`{atrading_home}/roles/`).
///
/// Roles are pure trading-account labels (name + display_name); the AI persona
/// they used to carry is now handled by pi-rs skills under `$PI_RS_HOME/agent/skills`.
pub fn default_roles_dir() -> PathBuf {
    crate::atrading_home().join("roles")
}

/// On-disk role file: a single `display_name` field. Filename stem = role name.
#[derive(Debug, Clone, Deserialize)]
struct RoleFile {
    #[serde(default)]
    display_name: Option<String>,
}

/// Load all editable roles from `~/.atrading/roles/*.toml`.
pub fn list_roles_from_disk() -> Vec<crate::RoleEntry> {
    let dir = default_roles_dir();
    let mut roles = Vec::new();

    let Ok(entries) = fs::read_dir(&dir) else {
        return roles;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let display_name = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| toml::from_str::<RoleFile>(&raw).ok())
            .and_then(|f| f.display_name)
            .unwrap_or_else(|| name.to_string());

        roles.push(crate::RoleEntry {
            name: name.to_string(),
            display_name,
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

#[derive(Debug, Clone, Serialize)]
struct RoleFileOut {
    display_name: String,
}

/// Persist a role as `~/.atrading/roles/{name}.toml`.
pub fn save_role(name: &str, display_name: &str) -> Result<()> {
    validate_role_name(name)?;
    let dir = default_roles_dir();
    fs::create_dir_all(&dir).with_context(|| format!("创建目录失败: {}", dir.display()))?;
    let content = toml::to_string(&RoleFileOut {
        display_name: display_name.to_string(),
    })
    .context("序列化角色失败")?;
    let path = dir.join(format!("{name}.toml"));
    fs::write(&path, content).with_context(|| format!("写入角色失败: {}", path.display()))?;
    Ok(())
}

/// Delete a role file `~/.atrading/roles/{name}.toml`.
pub fn delete_role(name: &str) -> Result<()> {
    validate_role_name(name)?;
    let path = default_roles_dir().join(format!("{name}.toml"));
    if path.exists() {
        fs::remove_file(&path).with_context(|| format!("删除角色失败: {}", path.display()))?;
    }
    Ok(())
}
