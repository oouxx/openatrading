//! pi-coding-agent extensions for openatrading.
//!
//! Provides [`create_registry()`] which returns an [`ExtensionRegistry`]
//! pre-configured with all openatrading extensions (market data / paper trading)
//! plus the pi-rs built-in Rust extensions (`pi-extensions`:
//! goal / subagent / web_search).
//!
//! Pass the extension registry to `create_agent_session()` via
//! `CreateAgentSessionOptions.extension_registry` to register extension-provided
//! tool metadata, prompt guidelines, and lifecycle hooks.
//!
//! MCP 服务端（对外暴露扩展工具）见 [`mcp::server`]，基于 pi-rs 的 `HookHandler`
//! 接口，不依赖 `pi-coding-agent`/`DynTool`。MCP **客户端**由 pi-rs SDK 提供，
//! 本项目经 `openatrading_core::llm::McpToolSet` 复用，不在本 crate 内。

pub mod mcp;
pub mod state;
pub mod trading;

pub use state::{DecisionSink, MemoryReader, PositionReader, TradingContext};

use pi_extension_api::{
    create_builtin_source_info, create_synthetic_source_info, ExtensionRegistry,
};

/// 构造扩展注册表的入参。
///
/// 打包可选状态，保持 `create_registry` 单参签名（遵循项目 ≤3 参数规则），
/// 同时支持"行情 only / 行情+交易"组合。
#[derive(Default)]
pub struct RegistryOptions {
    /// 交易态：`Some` 注册 paper-trading 工具（持仓/决策），`None` 仅行情。
    pub trading: Option<TradingContext>,
}

impl RegistryOptions {
    /// 仅行情工具（无交易），用于 MCP 服务端等 market-only 场景。
    #[must_use]
    pub fn market_only() -> Self {
        Self { trading: None }
    }
}

/// Create an `ExtensionRegistry` with all openatrading extensions + pi-rs built-ins.
///
/// 传入 [`RegistryOptions`] 控制是否启用 paper-trading 工具。
#[must_use]
pub fn create_registry(options: RegistryOptions) -> ExtensionRegistry {
    let mut registry = ExtensionRegistry::new();
    registry.register(
        Box::new(trading::MarketDataExtension::new(options.trading)),
        create_synthetic_source_info("/tmp/test".into(), "market".into(), None, None, None),
    );
    // pi-rs built-in Rust extensions (pi-extensions): goal / subagent / web_search.
    // Registered unconditionally — these are general-purpose agent capabilities
    // available to every session regardless of trading/mcp state.
    registry.register(
        Box::new(pi_extensions::goal::GoalExtension::new()),
        create_builtin_source_info("goal"),
    );
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
