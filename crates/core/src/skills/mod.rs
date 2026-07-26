use ahash::AHashMap as HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub use openatrading_agent::harness::types::Skill;

use crate::RoleEntry;

// ── agentskills.io-compatible skill entry ────────────────────────────────

/// A skill entry that conforms to the [Agent Skills](https://agentskills.io) specification.
///
/// Each skill is a directory containing `SKILL.md` with YAML frontmatter.
/// Optional subdirectories: `scripts/`, `references/`, `assets/`.
#[derive(Debug, Clone, Serialize)]
pub struct SkillEntry {
    // ── Required (agentskills.io) ──
    pub name: String,
    pub description: String,

    // ── Optional (agentskills.io) ──
    pub license: Option<String>,
    pub compatibility: Option<String>,
    pub metadata: HashMap<String, String>,
    /// Space-separated pre-approved tools (experimental).
    pub allowed_tools: Option<String>,

    // ── Internal ──
    /// Full path to the `SKILL.md` file.
    pub file_path: String,
    /// Markdown body after frontmatter.
    pub content: String,
    /// Base directory of the skill (parent of SKILL.md).
    pub base_dir: String,
    /// Whether the model should be prevented from invoking this skill.
    pub disable_model_invocation: bool,
    /// Scope where this skill was discovered.
    pub scope: SkillScope,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkillScope {
    /// Project-level: `<project>/.agents/skills/` or `<project>/.atrading/skills/`
    Project,
    /// User-level: `~/.agents/skills/` or `~/.atrading/skills/`
    User,
}

impl SkillEntry {
    /// Convert to the pi-rs `Skill` type for backward compatibility.
    #[must_use]
    pub fn to_pi_skill(&self) -> Skill {
        Skill {
            name: self.name.clone(),
            description: self.description.clone(),
            content: self.content.clone(),
            file_path: self.file_path.clone(),
            disable_model_invocation: self.disable_model_invocation,
        }
    }

    /// List bundled resources (scripts, references, assets).
    #[must_use]
    pub fn list_resources(&self) -> Vec<String> {
        let base = Path::new(&self.base_dir);
        let mut resources = Vec::new();
        for sub in &["scripts", "references", "assets"] {
            let dir = base.join(sub);
            if dir.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        if entry.path().is_file() {
                            if let Ok(rel) = entry.path().strip_prefix(base) {
                                resources.push(rel.to_string_lossy().to_string());
                            }
                        }
                    }
                }
            }
        }
        resources.sort();
        resources
    }
}

// ── Frontmatter parsing ─────────────────────────────────────────────────

/// Raw frontmatter fields from `SKILL.md`.

#[derive(Debug, Deserialize, Serialize)]
struct SkillFrontmatter {
    name: Option<String>,
    description: Option<String>,
    license: Option<String>,
    compatibility: Option<String>,
    #[serde(default)]
    metadata: Option<HashMap<String, String>>,
    #[serde(rename = "allowed-tools")]
    allowed_tools: Option<String>,
}

/// Parse YAML frontmatter from `---\n...\n---` delimited content.
/// Returns `(frontmatter, body)` where body is the content after the closing `---`.
fn parse_skill_frontmatter(raw: &str) -> (Option<SkillFrontmatter>, String) {
    match crate::utils::parse_yaml_frontmatter(raw) {
        Some((matter, body)) => {
            let fm = serde_yaml::from_str::<SkillFrontmatter>(&matter).ok();
            (fm, body.trim().to_string())
        }
        None => (None, raw.trim().to_string()),
    }
}

// ── Name validation ─────────────────────────────────────────────────────

/// Validate a skill name.
///
/// Rules:
/// - 1-64 characters
/// - Hyphen must not start, end, or appear consecutively
/// - Must contain at least one letter or digit
pub fn validate_skill_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("name must not be empty".into());
    }
    if name.len() > 64 {
        return Err(format!("name exceeds 64 characters: {name}"));
    }
    if name.starts_with('-') {
        return Err(format!("name must not start with hyphen: {name}"));
    }
    if name.ends_with('-') {
        return Err(format!("name must not end with hyphen: {name}"));
    }
    if name.contains("--") {
        return Err(format!("name must not contain consecutive hyphens: {name}"));
    }
    let has_alnum = name.chars().any(char::is_alphanumeric);
    if !has_alnum {
        return Err("name must contain at least one letter or digit".into());
    }
    Ok(())
}

// ── Scanning directories ────────────────────────────────────────────────

/// Scan a single skills directory for subdirectories containing `SKILL.md`.
/// Returns a list of `SkillEntry` values, skipping invalid ones with warnings.
fn scan_skill_dir(dir: &Path, scope: SkillScope) -> Vec<SkillEntry> {
    if !dir.is_dir() {
        return Vec::new();
    }
    let mut entries = Vec::new();
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = match path.file_name() {
            Some(n) => n.to_string_lossy().to_string(),
            None => continue,
        };
        // Skip common non-skill directories
        if dir_name.starts_with('.') || dir_name == "node_modules" {
            continue;
        }
        let skill_path = path.join("SKILL.md");
        if !skill_path.is_file() {
            continue;
        }
        let Ok(raw) = std::fs::read_to_string(&skill_path) else {
            continue;
        };
        let (fm, body) = parse_skill_frontmatter(&raw);
        if body.is_empty() {
            continue;
        }
        let (name, description) = fm.as_ref().map_or_else(
            || (dir_name.clone(), String::new()),
            |f| {
                let name = f.name.clone().unwrap_or_else(|| dir_name.clone());
                let desc = f.description.clone().unwrap_or_default();
                (name, desc)
            },
        );
        // Warn on name mismatch but still load
        if name != dir_name {
            crate::log::warn!(
                "skill name mismatch: frontmatter name '{name}' != directory name '{dir_name}'"
            );
        }
        // Warn on validation issues but still load (lenient)
        if let Err(e) = validate_skill_name(&name) {
            crate::log::warn!("skill name validation: {e}");
        }
        let (license, compatibility, metadata, allowed_tools) = fm.as_ref().map_or_else(
            || (None, None, HashMap::new(), None),
            |f| {
                (
                    f.license.clone(),
                    f.compatibility.clone(),
                    f.metadata.clone().unwrap_or_default(),
                    f.allowed_tools.clone(),
                )
            },
        );
        entries.push(SkillEntry {
            name,
            description,
            license,
            compatibility,
            metadata,
            allowed_tools,
            file_path: skill_path.to_string_lossy().to_string(),
            content: body,
            base_dir: path.to_string_lossy().to_string(),
            disable_model_invocation: false,
            scope: scope.clone(),
        });
    }
    entries
}

/// Resolve collision: project-level skills override user-level skills.
fn merge_skills(project: Vec<SkillEntry>, user: Vec<SkillEntry>) -> Vec<SkillEntry> {
    let mut map: HashMap<String, SkillEntry> = HashMap::new();
    // User-level first (lower priority)
    for s in user {
        map.entry(s.name.clone()).or_insert(s);
    }
    // Project-level overrides
    for s in project {
        if let Some(_existing) = map.get(&s.name) {
            crate::log::warn!(
                "skill collision: project-level '{name}' overrides user-level scope",
                name = s.name
            );
        }
        map.insert(s.name.clone(), s);
    }
    let mut skills: Vec<SkillEntry> = map.into_values().collect();
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    skills
}

/// A request body for creating or updating a skill.
#[derive(Debug, Deserialize)]
pub struct SkillRequest {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub compatibility: Option<String>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
    #[serde(default, rename = "allowed-tools")]
    pub allowed_tools: Option<String>,
}

impl SkillRequest {
    #[must_use]
    pub fn into_skill_entry(self) -> SkillEntry {
        SkillEntry {
            name: self.name,
            description: self.description,
            license: self.license,
            compatibility: self.compatibility,
            metadata: self.metadata,
            allowed_tools: self.allowed_tools,
            content: self.content,
            file_path: String::new(),
            base_dir: String::new(),
            disable_model_invocation: false,
            scope: SkillScope::User,
        }
    }
}

// ── SkillRegistry ───────────────────────────────────────────────────────

/// Registry of skills conforming to the agentskills.io specification.
///
/// Skills are discovered from multiple directories:
/// - Project-level: `<cwd>/.agents/skills/`, `<cwd>/.atrading/skills/`
/// - User-level: `~/.agents/skills/`, `~/.atrading/skills/`
///
/// Project-level skills take precedence over user-level skills.
pub struct SkillRegistry {
    skills: Vec<SkillEntry>,
}

impl SkillRegistry {
    #[must_use]
    pub const fn new() -> Self {
        Self { skills: Vec::new() }
    }

    pub fn register(&mut self, skill: SkillEntry) {
        self.skills.push(skill);
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    #[must_use]
    pub fn skills(&self) -> &[SkillEntry] {
        &self.skills
    }

    /// Build the full system prompt for a role (decision path).
    #[must_use]
    pub fn build_role_prompt(role: &RoleEntry) -> String {
        let role_skill = role.to_skill();
        let skills = Self::from_disk();
        let mut parts = Vec::new();
        parts.push(format_skill_invocation_entry(&role_skill, None));
        let idx = format_skills_catalog(skills.skills());
        if !idx.is_empty() {
            parts.push(idx);
        }
        parts.join("\n\n")
    }

    /// Build system prompt for chat paths (web, Tauri, Telegram ask).
    #[must_use]
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
        parts.push(format_skill_invocation_entry(&fallback, None));
        let idx = format_skills_catalog(skills.skills());
        if !idx.is_empty() {
            parts.push(idx);
        }
        parts.join("\n\n")
    }

    /// User-level skills directory (`~/.atrading/skills/`).
    #[must_use]
    pub fn install_dir() -> PathBuf {
        crate::atrading_home().join("skills")
    }

    /// User-level cross-client skills directory (`~/.agents/skills/`).
    #[must_use]
    pub fn agents_skills_dir() -> PathBuf {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".agents").join("skills")
    }

    /// Project-level cross-client skills directory (`<cwd>/.agents/skills/`).
    #[must_use]
    pub fn project_agents_skills_dir() -> PathBuf {
        std::env::current_dir().map_or_else(
            |_| PathBuf::from(".agents").join("skills"),
            |cwd| cwd.join(".agents").join("skills"),
        )
    }

    /// Project-level skills directory (`<cwd>/.atrading/skills/`).
    #[must_use]
    pub fn project_skills_dir() -> PathBuf {
        std::env::current_dir().map_or_else(
            |_| PathBuf::from(".atrading").join("skills"),
            |cwd| cwd.join(".atrading").join("skills"),
        )
    }

    /// Write a skill to disk. The raw content is written verbatim as `SKILL.md`.
    pub fn install_skill(skill: &SkillEntry) -> std::io::Result<PathBuf> {
        let dir = Self::install_dir().join(&skill.name);
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("SKILL.md");
        // Write with frontmatter
        let frontmatter = serde_yaml::to_string(&SkillFrontmatter {
            name: Some(skill.name.clone()),
            description: Some(skill.description.clone()),
            license: skill.license.clone(),
            compatibility: skill.compatibility.clone(),
            metadata: if skill.metadata.is_empty() {
                None
            } else {
                Some(skill.metadata.clone())
            },
            allowed_tools: skill.allowed_tools.clone(),
        })
        .unwrap_or_default();
        let full = format!("---\n{}---\n\n{}", frontmatter, skill.content);
        std::fs::write(&path, &full)?;
        Ok(path)
    }

    /// Delete a skill from disk by name.
    pub fn uninstall_skill(name: &str) -> std::io::Result<()> {
        let dir = Self::install_dir().join(name);
        if !dir.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("skill '{name}' not found"),
            ));
        }
        std::fs::remove_dir_all(&dir)
    }

    /// Load all skills from all configured directories.
    /// Returns a merged registry with project-level skills overriding user-level.
    #[must_use]
    pub fn from_disk() -> Self {
        let user_native = scan_skill_dir(&Self::install_dir(), SkillScope::User);
        // let user_agents = scan_skill_dir(&Self::agents_skills_dir(), SkillScope::User);
        let project_native = scan_skill_dir(&Self::project_skills_dir(), SkillScope::Project);
        // let project_agents =
        // scan_skill_dir(&Self::project_agents_skills_dir(), SkillScope::Project);

        let user = user_native.into_iter().chain(vec![]).collect();
        let project = project_native.into_iter().chain(vec![]).collect();

        let merged = merge_skills(project, user);
        Self { skills: merged }
    }

    /// Parse a raw string into a `SkillEntry`. Accepts any non-empty content.
    /// Returns `None` if the content is empty or whitespace.
    #[must_use]
    pub fn parse_skill_from_md(raw: &str) -> Option<SkillEntry> {
        let content = raw.trim().to_string();
        if content.is_empty() {
            return None;
        }
        let (fm, body) = parse_skill_frontmatter(&content);
        let name = fm
            .as_ref()
            .and_then(|f| f.name.clone())
            .unwrap_or_else(|| "unnamed".into());
        let description = fm
            .as_ref()
            .and_then(|f| f.description.clone())
            .unwrap_or_default();
        let (license, compatibility, metadata, allowed_tools) = fm.as_ref().map_or_else(
            || (None, None, HashMap::new(), None),
            |f| {
                (
                    f.license.clone(),
                    f.compatibility.clone(),
                    f.metadata.clone().unwrap_or_default(),
                    f.allowed_tools.clone(),
                )
            },
        );
        Some(SkillEntry {
            name,
            description,
            license,
            compatibility,
            metadata,
            allowed_tools,
            file_path: String::new(),
            content: body,
            base_dir: String::new(),
            disable_model_invocation: false,
            scope: SkillScope::User,
        })
    }

    /// Parse a raw string and install it to disk in one step.
    pub fn install_from_raw_md(raw: &str) -> std::io::Result<PathBuf> {
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

// ── Catalog & invocation formatting (agentskills.io progressive disclosure) ──

/// Format the skill catalog for system prompt injection (tier 1 of progressive disclosure).
///
/// Includes `name`, `description`, and `location` for each skill.
/// Skills with `disable_model_invocation: true` are excluded.
#[must_use]
pub fn format_skills_catalog(skills: &[SkillEntry]) -> String {
    let visible: Vec<&SkillEntry> = skills
        .iter()
        .filter(|s| !s.disable_model_invocation)
        .collect();

    if visible.is_empty() {
        return String::new();
    }

    let mut lines = vec![
        "The following skills provide specialized instructions for specific tasks.".to_string(),
        "When a task matches a skill's description, use the activate_skill tool".to_string(),
        "with the skill's name to load its full instructions.".to_string(),
        "When a skill references relative paths, resolve them against the skill's".to_string(),
        "directory (the parent of SKILL.md) and use absolute paths in tool calls.".to_string(),
        String::new(),
        "<available_skills>".to_string(),
    ];

    for skill in visible {
        lines.push("  <skill>".to_string());
        lines.push(format!("    <name>{}</name>", xml_escape(&skill.name)));
        lines.push(format!(
            "    <description>{}</description>",
            xml_escape(&skill.description)
        ));
        if !skill.file_path.is_empty() {
            lines.push(format!(
                "    <location>{}</location>",
                xml_escape(&skill.file_path)
            ));
        }
        if let Some(license) = &skill.license {
            lines.push(format!("    <license>{}</license>", xml_escape(license)));
        }
        if let Some(compat) = &skill.compatibility {
            lines.push(format!(
                "    <compatibility>{}</compatibility>",
                xml_escape(compat)
            ));
        }
        if let Some(tools) = &skill.allowed_tools {
            lines.push(format!(
                "    <allowed_tools>{}</allowed_tools>",
                xml_escape(tools)
            ));
        }
        lines.push("  </skill>".to_string());
    }

    lines.push("</available_skills>".to_string());
    lines.join("\n")
}

/// Format a skill as an XML block for in-prompt invocation (tier 2 of progressive disclosure).
///
/// Includes the skill name, file location, and full content.
/// Optionally appends additional instructions after the skill block.
#[must_use]
pub fn format_skill_invocation_entry(
    skill: &Skill,
    additional_instructions: Option<&str>,
) -> String {
    let skill_dir = std::path::Path::new(&skill.file_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let skill_block = format!(
        "<skill name=\"{}\" location=\"{}\">\nReferences are relative to {}.\n\n{}\n</skill>",
        xml_escape(&skill.name),
        xml_escape(&skill.file_path),
        skill_dir,
        skill.content
    );

    if let Some(instructions) = additional_instructions {
        format!("{skill_block}\n\n{instructions}")
    } else {
        skill_block
    }
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
