use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct RoleConfig {
    pub name: String,
    #[serde(default)]
    pub readonly: bool,
}

pub mod pnl;
pub mod prompt;

/// Path to the skills directory (`{atrading_home}/skills/`).
pub fn default_skills_dir() -> PathBuf {
    crate::atrading_home().join("skills")
}

/// Parse YAML frontmatter from `---\n...\n---` delimited content.
pub fn parse_frontmatter(content: &str) -> (HashMap<String, String>, String) {
    match crate::utils::parse_yaml_frontmatter(content) {
        Some((matter, body)) => {
            let map: HashMap<String, String> = serde_yaml::from_str(&matter).unwrap_or_default();
            (map, body)
        },
        None => (HashMap::new(), content.trim().to_string()),
    }
}

/// Load all SKILL.md entries from `~/.atrading/skills/` as RoleEntry.
pub fn list_roles_from_disk() -> Vec<crate::RoleEntry> {
    let dir = default_skills_dir();
    let mut roles = Vec::new();

    let entries = match fs::read_dir(&dir) {
        Ok(d) => d,
        Err(_) => return roles,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = match path.file_name() {
            Some(n) => n.to_string_lossy().to_string(),
            None => continue,
        };
        let skill_path = path.join("SKILL.md");
        let raw = match fs::read_to_string(&skill_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let (fm, body) = parse_frontmatter(&raw);
        if body.is_empty() {
            continue;
        }
        let display_name = fm
            .get("description")
            .cloned()
            .or_else(|| fm.get("name").cloned())
            .unwrap_or_else(|| name.clone());

        roles.push(crate::RoleEntry {
            name,
            display_name,
            strategy: body.to_string(),
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

/// Save a role strategy as `~/.atrading/skills/{name}/SKILL.md`.
pub fn save_role_strategy(name: &str, strategy: &str) -> Result<()> {
    validate_role_name(name)?;
    let dir = default_skills_dir().join(name);
    fs::create_dir_all(&dir).with_context(|| format!("创建目录失败: {}", dir.display()))?;
    let content = format!("---\nname: {name}\ndescription: {name}\n---\n\n{strategy}");
    fs::write(dir.join("SKILL.md"), &content)
        .with_context(|| format!("写入策略失败: {}/SKILL.md", name))?;
    Ok(())
}

/// Delete a role directory from `~/.atrading/skills/{name}/`.
pub fn delete_role_dir(name: &str) -> Result<()> {
    validate_role_name(name)?;
    let dir = default_skills_dir().join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir).with_context(|| format!("删除角色目录失败: {}", dir.display()))?;
    }
    Ok(())
}
