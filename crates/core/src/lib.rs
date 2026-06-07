pub mod agents;
pub mod api;
pub mod cron;
pub mod llm;
pub mod memory;
pub mod roles;
pub mod skills;
pub mod social;
pub mod trade;
pub mod utils;

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
