mod builtin;

pub use pi_agent_core::harness::skills::format_skill_invocation;
pub use pi_agent_core::harness::skills::format_skills_for_system_prompt;
pub use pi_agent_core::harness::types::Skill;

use crate::RoleEntry;

/// Registry of trading skills backed by pi-rs `Skill` type.
///
/// Skills use the same `Skill` struct and formatters from pi-rs's harness
/// module. Built-in skills are defined programmatically and can be
/// supplemented with file-based skills loaded from `~/.atrading/skills/`.
pub struct SkillRegistry {
    skills: Vec<Skill>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self { skills: Vec::new() }
    }

    pub fn register(&mut self, skill: Skill) {
        self.skills.push(skill);
    }

    /// Create a registry pre-populated with all built-in trading skills.
    pub fn with_builtin() -> Self {
        let mut reg = Self::new();
        for skill in builtin::builtin_skills() {
            reg.register(skill);
        }
        reg
    }

    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    pub fn skills(&self) -> &[Skill] {
        &self.skills
    }

    /// Build the full system prompt for a role (decision path).
    ///
    /// Uses pi-rs `format_skill_invocation` for the role's strategy content
    /// and `format_skills_for_system_prompt` for the skill index (from disk
    /// or built-in fallback).
    pub fn build_role_prompt(role: &RoleEntry) -> String {
        let role_skill = role.to_skill();
        let skills = Self::from_disk();
        let mut parts = Vec::new();
        parts.push(format_skill_invocation(&role_skill, None));
        let idx = format_skills_for_system_prompt(skills.skills());
        if !idx.is_empty() {
            parts.push(idx);
        }
        parts.join("\n\n")
    }

    /// Build system prompt for chat paths (web, Tauri, Telegram ask).
    /// Creates a transient skill from the raw strategy text.
    pub fn build_chat_prompt(strategy_text: &str) -> String {
        let fallback = Skill {
            name: "default".into(),
            description: "当前策略".into(),
            content: strategy_text.to_string(),
            file_path: "builtin://default".into(),
            disable_model_invocation: false,
        };
        let skills = Self::from_disk();
        let mut parts = Vec::new();
        parts.push(format_skill_invocation(&fallback, None));
        let idx = format_skills_for_system_prompt(skills.skills());
        if !idx.is_empty() {
            parts.push(idx);
        }
        parts.join("\n\n")
    }

    /// Path to the installed skills directory (`{atrading_home}/skills/`).
    pub fn install_dir() -> std::path::PathBuf {
        crate::atrading_home().join("skills")
    }

    /// Write a skill to disk as a `SKILL.md` file in the install directory.
    pub fn install_skill(skill: &Skill) -> std::io::Result<std::path::PathBuf> {
        let dir = Self::install_dir().join(&skill.name);
        std::fs::create_dir_all(&dir)?;
        let content = format!(
            "---\nname: {}\ndescription: {}\n---\n\n{}",
            skill.name, skill.description, skill.content
        );
        let path = dir.join("SKILL.md");
        std::fs::write(&path, &content)?;
        Ok(path)
    }

    /// Load all skills from `~/.atrading/skills/`.
    /// Falls back to compiled built-in skills if directory is empty or missing.
    pub fn from_disk() -> Self {
        let dir = Self::install_dir();
        if !dir.exists() {
            return Self::with_builtin();
        }

        let mut reg = Self::new();
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => return Self::with_builtin(),
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let skill_path = path.join("SKILL.md");
            if !skill_path.exists() {
                continue;
            }
            let raw = match std::fs::read_to_string(&skill_path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let (fm, body) = crate::roles::parse_frontmatter(&raw);
            if body.is_empty() {
                continue;
            }
            let name = fm.get("name").cloned().unwrap_or_else(|| {
                path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            });
            let description = fm.get("description").cloned().unwrap_or_default();
            reg.register(Skill {
                name,
                description,
                content: body.to_string(),
                file_path: skill_path.to_string_lossy().to_string(),
                disable_model_invocation: false,
            });
        }

        if reg.is_empty() {
            Self::with_builtin()
        } else {
            reg
        }
    }

    /// List built-in skills available for installation.
    pub fn builtin_skills() -> Vec<Skill> {
        builtin::builtin_skills()
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}
