pub mod api;
pub mod automation;
pub mod commands;
pub mod config;
pub mod context;
pub mod init;
pub mod llm;
pub mod log;
pub mod memory;
// pub mod platform;
pub mod prompts;
pub mod roles;
pub mod skills;
pub mod social;
pub use openatrading_channels as channels;
pub use openatrading_extension::create_registry as create_extension_registry;
pub use openatrading_extension::RegistryOptions;
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
    std::env::var("OPENATRADING_HOME").map_or_else(
        |_| {
            let home = std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join(".atrading")
        },
        |base| PathBuf::from(base).join(".atrading"),
    )
}

// Re-export RoleEntry since it's used across web, cli, and social modules
#[derive(Debug, Clone, serde::Serialize)]
pub struct RoleEntry {
    pub name: String,
    pub display_name: String,
    /// true = defined in config.toml, cannot be edited/deleted via API
    #[serde(default)]
    pub readonly: bool,
}
