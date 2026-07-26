//! pi-coding-agent extensions for openatrading.
//!
//! Provides `create_registry()` which returns an `ExtensionRegistry` pre-configured
//! with all openatrading extensions and the pi-rs built-in goal extension.
//!
//! Pass the registry to `create_agent_session()` via
//! `CreateAgentSessionOptions.extension_registry` to register extension-provided
//! tool metadata, prompt guidelines, and event hooks.

pub mod trading;

use pi_extension_api::ExtensionRegistry;

/// Create an `ExtensionRegistry` with all openatrading extensions + pi-rs built-ins.
pub fn create_registry() -> ExtensionRegistry {
    let mut registry = ExtensionRegistry::new();
    registry.register(Box::new(trading::MarketDataExtension));
    // registry.register(Box::new(pi_extensions::goal::GoalExtension::new()));
    registry
}
