//! pi-coding-agent extensions for openatrading.
//!
//! Provides [`create_registry()`] which returns an [`ExtensionRegistry`]
//! pre-configured with all openatrading extensions and the pi-rs built-in
//! goal extension.
//!
//! Pass the registry to `create_agent_session()` via
//! `CreateAgentSessionOptions.extension_registry` to register extension-provided
//! tool metadata, prompt guidelines, and event hooks.
//!
//! [`create_registry()`] takes an optional [`TradingContext`]: when `Some`,
//! the paper-trading tools (`get_current_positions`, `get_historical_context`,
//! `submit_trading_decision`) are registered in addition to the market-data
//! tools; when `None`, only market-data tools are registered (used for the
//! MCP-serving global registry which must stay market-only).

pub mod state;
pub mod trading;

pub use state::{DecisionSink, MemoryReader, PositionReader, TradingContext};

use pi_extension_api::{create_synthetic_source_info, ExtensionRegistry};

/// Create an `ExtensionRegistry` with all openatrading extensions + pi-rs built-ins.
///
/// Pass `Some(ctx)` to register the paper-trading tools; `None` for a
/// market-data-only registry.
pub fn create_registry(trading: Option<TradingContext>) -> ExtensionRegistry {
    let mut registry = ExtensionRegistry::new();
    registry.register(
        Box::new(trading::MarketDataExtension::new(trading)),
        create_synthetic_source_info("/tmp/test".into(), "market".into(), None, None, None),
    );
    // registry.register(Box::new(pi_extensions::goal::GoalExtension::new()));
    registry
}
