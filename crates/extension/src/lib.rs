//! pi-coding-agent extensions for openatrading.
//!
//! Provides [`create_registry()`] which returns an [`ExtensionRegistry`]
//! pre-configured with openatrading's market-data extension plus the pi-rs
//! built-in Rust extensions (`pi-extensions`: subagent / web_search).
//!
//! Pass the extension registry to `create_agent_session()` via
//! `CreateAgentSessionOptions.extension_registry` to register extension-provided
//! tool metadata, prompt guidelines, and lifecycle hooks.
//!
//! MCP 服务端（对外暴露扩展工具）见 [`mcp::server`]，基于 pi-rs 的 `HookHandler`
//! 接口，不依赖 `pi-coding-agent`/`DynTool`。MCP **客户端**由 pi-rs SDK 提供，
//! 本项目经 `openatrading_core::llm::McpToolSet` 复用，不在本 crate 内。
//!
//! 交易不在扩展里：模拟盘改为用户手动记账（`openatrading_core::trade`），
//! agent 只拿行情/资讯工具，没有任何下单或读持仓能力；韭研专题分析由项目 Skill 负责。

pub mod mcp;
pub mod trading;

use pi_extension_api::{
    create_builtin_source_info, create_synthetic_source_info, ExtensionRegistry,
};

/// Create an `ExtensionRegistry` with all openatrading extensions + pi-rs built-ins.
#[must_use]
pub fn create_registry() -> ExtensionRegistry {
    let mut registry = ExtensionRegistry::new();
    registry.register(
        Box::new(trading::MarketDataExtension::new()),
        create_synthetic_source_info("/tmp/test".into(), "market".into(), None, None, None),
    );
    // pi-rs built-in Rust extensions (pi-extensions): subagent / web_search.
    registry.register(
        Box::new(pi_extensions::subagent::SubagentExtension::new()),
        create_builtin_source_info("subagent"),
    );
    registry.register(
        Box::new(pi_extensions::web_search::WebSearchExtension::new()),
        create_builtin_source_info("web_search"),
    );
    registry
}
