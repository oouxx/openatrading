//! Market data extension — A-share trading tools for the agent.
//!
//! Registered tools appear in the agent's tool list via prompt guidelines.
//! Actual tool execution is handled by `DynTool` instances passed through
//! `CreateAgentSessionOptions.custom_tools` with `execute` callbacks.

use async_trait::async_trait;
use pi_extension_api::{EventResult, ExtensionAPI, ExtensionContext, ExtensionEvent, ToolRegistry};

/// Market data extension providing A-share trading tools.
pub struct MarketDataExtension;

impl MarketDataExtension {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MarketDataExtension {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExtensionAPI for MarketDataExtension {
    fn name(&self) -> &'static str {
        "openatrading-market"
    }

    fn register_tools(&self, _registry: &mut ToolRegistry) {
        // Tools are provided via CreateAgentSessionOptions.custom_tools with
        // execute callbacks. Extension tool registration would duplicate names.
    }

    async fn on_event(
        &self,
        _event: &ExtensionEvent,
        _ctx: &ExtensionContext,
    ) -> Option<EventResult> {
        None
    }
}
