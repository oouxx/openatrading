//! Thin wrapper over pi-rs skill loading.
//!
//! Project Skills live in the shared `.agents/skills` directory, while user
//! Skills use pi-rs's default `$PI_RS_HOME/agent/skills`. Both paths are passed
//! to pi-rs, which discovers and parses the files, injects the
//! `<available_skills>` catalog into the system prompt, and expands
//! `/skill:<name>` on demand — no host-side Skill parsing is involved.
//!
//! This module only provides:
//! - disk CRUD (`install_skill` / `uninstall_skill`) for the web/CLI editors,
//! - a listing helper (`list_skills`) that delegates to pi-rs `load_skills`,
//! - a `SkillEntry` projection (pi-rs `Skill` + markdown body) for the editor.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub use pi_coding_agent::core::skills::{load_skills, LoadSkillsOptions, LoadSkillsResult, Skill};

use openatrading_agent::PROJECT_SKILLS_DIR;

/// Directory pi-rs scans for skills: `$PI_RS_HOME/agent/skills`.
pub fn skills_dir() -> PathBuf {
    pi_coding_agent::config::get_agent_dir().join("skills")
}

/// List all skills discovered by pi-rs (user + project scopes).
///
/// Delegates to `pi_coding_agent::core::skills::load_skills` with the default
/// scan paths (`include_defaults: true`), so the result matches what the
/// model actually sees in its system prompt.
pub fn list_skills() -> Vec<Skill> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    load_skills_at(&cwd).skills
}

fn load_skills_at(cwd: &Path) -> LoadSkillsResult {
    let cwd = cwd.to_string_lossy().to_string();
    let agent_dir = pi_coding_agent::config::get_agent_dir()
        .to_string_lossy()
        .to_string();
    load_skills(&LoadSkillsOptions {
        cwd: cwd.clone(),
        agent_dir: Some(agent_dir),
        skill_paths: vec![Path::new(&cwd)
            .join(PROJECT_SKILLS_DIR)
            .to_string_lossy()
            .to_string()],
        include_defaults: true,
    })
}

/// Return the pi-rs `source_info.scope` as a static string (`"user"` /
/// `"project"` / `"temporary"`), so callers need not depend on pi-rs directly.
pub fn skill_scope(skill: &Skill) -> &'static str {
    use pi_coding_agent::core::source_info::SourceScope;
    match skill.source_info.scope {
        SourceScope::User => "user",
        SourceScope::Project => "project",
        SourceScope::Temporary => "temporary",
    }
}

/// Read the markdown body (after YAML frontmatter) of a skill file.
pub fn read_skill_content(file_path: &str) -> Option<String> {
    let raw = std::fs::read_to_string(file_path).ok()?;
    match crate::utils::parse_yaml_frontmatter(&raw) {
        Some((_matter, body)) => Some(body.trim().to_string()),
        None => Some(raw.trim().to_string()),
    }
}

/// Validate a skill name for safe disk use.
///
/// This is a safety check (no path traversal / empty), not the full Agent
/// Skills spec — pi-rs enforces the strict spec at load time and emits
/// diagnostics, while still loading leniently named skills (e.g. non-ASCII).
pub fn validate_skill_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("name must not be empty".into());
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err("name must not contain path separators or '..'".into());
    }
    Ok(())
}

// ── API / CLI projection ────────────────────────────────────────────────

/// A skill as exposed to the web/CLI surface. Mirrors pi-rs `Skill` plus the
/// markdown body (read on demand) so the editor can show/edit the content.
#[derive(Debug, Clone, Serialize)]
pub struct SkillEntry {
    pub name: String,
    pub description: String,
    pub file_path: String,
    pub base_dir: String,
    pub disable_model_invocation: bool,
    /// `"user"` | `"project"` | `"temporary"` — from pi-rs `source_info.scope`.
    pub scope: String,
    /// Markdown body after frontmatter (read from `file_path`).
    pub content: String,
}

impl SkillEntry {
    /// Build from a pi-rs `Skill`, reading the body from disk.
    pub fn from_pi(skill: &Skill) -> Self {
        use pi_coding_agent::core::source_info::SourceScope;
        let scope = match skill.source_info.scope {
            SourceScope::User => "user",
            SourceScope::Project => "project",
            SourceScope::Temporary => "temporary",
        };
        Self {
            name: skill.name.clone(),
            description: skill.description.clone(),
            file_path: skill.file_path.clone(),
            base_dir: skill.base_dir.clone(),
            disable_model_invocation: skill.disable_model_invocation,
            scope: scope.to_string(),
            content: read_skill_content(&skill.file_path).unwrap_or_default(),
        }
    }
}

/// List skills as `SkillEntry` (with content) for the editor surface.
pub fn list_skill_entries() -> Vec<SkillEntry> {
    list_skills().iter().map(SkillEntry::from_pi).collect()
}

/// Request body for creating or updating a skill.
#[derive(Debug, Deserialize)]
pub struct SkillRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub content: String,
    #[serde(default, rename = "disable-model-invocation")]
    pub disable_model_invocation: bool,
}

/// Write a skill's `SKILL.md` to the skills directory.
pub fn install_skill(req: &SkillRequest) -> std::io::Result<PathBuf> {
    validate_skill_name(&req.name)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    let dir = skills_dir().join(&req.name);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("SKILL.md");
    let mut fm = String::from("---\n");
    fm.push_str(&format!("name: {}\n", req.name));
    fm.push_str(&format!("description: {}\n", req.description));
    if req.disable_model_invocation {
        fm.push_str("disable-model-invocation: true\n");
    }
    fm.push_str("---\n\n");
    fm.push_str(&req.content);
    std::fs::write(&path, fm)?;
    Ok(path)
}

/// Delete a skill by name from the skills directory.
pub fn uninstall_skill(name: &str) -> std::io::Result<()> {
    let dir = skills_dir().join(name);
    if !dir.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("skill '{name}' not found"),
        ));
    }
    std::fs::remove_dir_all(&dir)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn delegates_project_agents_skills_to_pi_rs_loader() {
        let temp = tempfile::tempdir().expect("tempdir");
        let skill_dir = temp.path().join(PROJECT_SKILLS_DIR).join("project-skill");
        std::fs::create_dir_all(&skill_dir).expect("create skill dir");
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: project-skill\ndescription: Project skill\n---\n\nBody.\n",
        )
        .expect("write skill");

        let result = load_skills_at(temp.path());
        assert!(
            result
                .skills
                .iter()
                .any(|skill| skill.name == "project-skill"),
            "pi-rs should load the configured `.agents/skills` path"
        );
    }
}
