pub mod agents;
pub mod api;
pub mod cron;
pub mod llm;
pub mod memory;
pub mod roles;
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
