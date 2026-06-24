pub mod agents;
pub mod api;
pub mod chat;
pub mod commands;
pub mod context;
pub mod cron;
pub mod llm;
pub mod memory;
pub mod pipelines;
pub mod prompts;
pub mod roles;
pub mod skills;
pub mod social;
pub use openatrading_channels as channels;
pub mod trade;
pub mod types;
pub mod utils;

use std::path::PathBuf;

/// Check whether aggressive trading mode is enabled via `OPENATRADING_AGGRESSIVE`.
///
/// When enabled: daily full-position requirement, every decision must produce
/// buy/sell/hold instructions (no "wait and see").
pub fn is_aggressive_mode() -> bool {
    std::env::var("OPENATRADING_AGGRESSIVE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Resolve the `.atrading` data directory.
///
/// Priority:
/// 1. `$OPENATRADING_HOME` — replaces the home base entirely
/// 2. `$HOME` / `$USERPROFILE` — standard user home directory
pub fn atrading_home() -> PathBuf {
    if let Ok(base) = std::env::var("OPENATRADING_HOME") {
        PathBuf::from(base).join(".atrading")
    } else {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".atrading")
    }
}

// Re-export RoleEntry since it's used across web, cli, and social modules
#[derive(Debug, Clone, serde::Serialize)]
pub struct RoleEntry {
    pub name: String,
    pub display_name: String,
    pub strategy: String,
    /// true = defined in config.toml, cannot be edited/deleted via API
    #[serde(default)]
    pub readonly: bool,
}

use crate::skills::Skill;

impl RoleEntry {
    /// Convert this role into a pi-rs `Skill` for use with skill formatters.
    pub fn to_skill(&self) -> Skill {
        Skill {
            name: self.name.clone(),
            description: self.display_name.clone(),
            content: self.strategy.clone(),
            file_path: format!("role://{}", self.name),
            disable_model_invocation: false,
        }
    }
}
