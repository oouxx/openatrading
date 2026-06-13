pub use pi_agent_core::harness::skills::format_skill_invocation;
pub use pi_agent_core::harness::skills::format_skills_for_system_prompt;
pub use pi_agent_core::harness::types::Skill;

use crate::RoleEntry;

/// Registry of trading skills backed by pi-rs `Skill` type.
///
/// Skills are loaded from `~/.atrading/skills/`. Each subdirectory contains a
/// `SKILL.md` file whose raw content becomes the skill content. No frontmatter
/// or format is required — the directory name is used as the skill name.
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

    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    pub fn skills(&self) -> &[Skill] {
        &self.skills
    }

    /// Build the full system prompt for a role (decision path).
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
    pub fn build_chat_prompt(strategy_text: &str) -> String {
        let fallback = Skill {
            name: "default".into(),
            description: String::new(),
            content: strategy_text.to_string(),
            file_path: String::new(),
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

    /// Write a skill to disk. The raw content is written verbatim as `SKILL.md`.
    pub fn install_skill(skill: &Skill) -> std::io::Result<std::path::PathBuf> {
        let dir = Self::install_dir().join(&skill.name);
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("SKILL.md");
        std::fs::write(&path, &skill.content)?;
        Ok(path)
    }

    /// Load all skills from `~/.atrading/skills/`.
    /// Returns an empty registry if no skills are installed.
    pub fn from_disk() -> Self {
        let dir = Self::install_dir();
        if !dir.exists() {
            return Self::new();
        }

        let mut reg = Self::new();
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => return Self::new(),
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
            let content = match std::fs::read_to_string(&skill_path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            reg.register(Skill {
                name,
                description: String::new(),
                content,
                file_path: skill_path.to_string_lossy().to_string(),
                disable_model_invocation: false,
            });
        }

        reg
    }

    /// Parse a raw string into a Skill. Accepts any non-empty content.
    /// Returns `None` if the content is empty or whitespace.
    pub fn parse_skill_from_md(raw: &str) -> Option<Skill> {
        let content = raw.trim().to_string();
        if content.is_empty() {
            return None;
        }
        Some(Skill {
            name: "unnamed".into(),
            description: String::new(),
            content,
            file_path: String::new(),
            disable_model_invocation: false,
        })
    }

    /// Parse a raw string and install it to disk in one step.
    pub fn install_from_raw_md(raw: &str) -> std::io::Result<std::path::PathBuf> {
        let skill = Self::parse_skill_from_md(raw).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "empty skill body")
        })?;
        Self::install_skill(&skill)
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}
