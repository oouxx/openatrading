//! pi-goal extension — /goal command, session-aware goal tracking.
//!
//! Wraps `pi_extensions::goal::GoalExtension` from pi-rs.

use async_trait::async_trait;
use pi_extension_api::{EventResult, ExtensionAPI, ExtensionContext, ExtensionEvent};

/// Goal tracking extension. Delegates to pi-rs's `GoalExtension` internally.
pub struct GoalExt(pub pi_extensions::goal::GoalExtension);

impl GoalExt {
    pub fn new() -> Self {
        Self(pi_extensions::goal::GoalExtension::new())
    }
}

impl Default for GoalExt {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExtensionAPI for GoalExt {
    fn name(&self) -> &'static str {
        "goal"
    }

    fn register_tools(&self, registry: &mut pi_extension_api::ToolRegistry) {
        self.0.register_tools(registry);
    }

    fn register_commands(&self, registry: &mut pi_extension_api::CommandRegistry) {
        self.0.register_commands(registry);
    }

    fn register_shortcuts(&self, registry: &mut pi_extension_api::ShortcutRegistry) {
        self.0.register_shortcuts(registry);
    }

    fn register_flags(&self, registry: &mut pi_extension_api::FlagRegistry) {
        self.0.register_flags(registry);
    }

    async fn on_event(
        &self,
        event: &ExtensionEvent,
        ctx: &ExtensionContext,
    ) -> Option<EventResult> {
        self.0.on_event(event, ctx).await
    }
}
