//! Persistence for built-in extension toggles.
//!
//! pi-rs already owns the canonical `settings.json` field
//! `extensionsEnabled` (`HashMap<extension_id, bool>`), so this module reuses
//! it instead of introducing a second config file. The agent/session layer
//! defines the built-in metadata and the filter logic; core only handles
//! reading and writing through [`SettingsManager`].

use pi_coding_agent::core::settings_manager::SettingsManager;

pub use openatrading_agent::session::{
    command_base_name, disabled_command_in_prompt, is_command_disabled, BuiltinExtension,
    ExtensionToggles, BUILTIN_EXTENSIONS,
};

/// Open the same global settings file the rest of the app uses.
fn settings_manager() -> SettingsManager {
    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    SettingsManager::create(&cwd, None)
}

/// Load effective built-in extension toggles from pi-rs settings.
///
/// Missing ids default to enabled (see [`ExtensionToggles`]).
#[must_use]
pub fn load_extension_toggles() -> ExtensionToggles {
    let manager = settings_manager();
    ExtensionToggles::from_enabled_map(&manager.get_extensions_enabled())
}

/// Persist the full toggle map to `settings.json` as `extensionsEnabled`.
///
/// Unknown extension ids carried in the map are written back unchanged.
///
/// # Errors
/// The map could not be serialized.
pub fn save_extension_toggles(toggles: &ExtensionToggles) -> Result<(), String> {
    let mut manager = settings_manager();
    let value = serde_json::to_value(toggles.enabled_map())
        .map_err(|e| format!("failed to serialize extension settings: {e}"))?;
    manager.set_global("extensionsEnabled", value);
    Ok(())
}
