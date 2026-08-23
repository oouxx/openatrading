//! pi-coding-agent extensions for openatrading.
//!
//! Provides [`create_registry()`] which returns an [`ExtensionRegistry`]
//! pre-configured with all openatrading extensions (market data / paper trading /
//! MCP 客户端) plus the pi-rs built-in Rust extensions (`pi-extensions`:
//! goal / subagent / web_search).
//!
//! Pass the extension registry to `create_agent_session()` via
//! `CreateAgentSessionOptions.extension_registry` to register extension-provided
//! tool metadata, prompt guidelines, and lifecycle hooks.
//!
//! MCP 客户端工具（外部 MCP server）通过 [`mcp::McpExtension`] 接入；
//! MCP 服务端（对外暴露扩展工具）见 [`mcp::server`]。两者均基于 pi-rs 的
//! `HookHandler` 接口，不依赖 `pi-coding-agent`/`DynTool`。

pub mod mcp;
pub mod state;
pub mod trading;

pub use mcp::{DiscoveredServer, McpServerConfig, McpTransportConfig};
pub use state::{DecisionSink, MemoryReader, PositionReader, TradingContext};

use mcp::McpExtension;
use pi_extension_api::{
    create_builtin_source_info, create_synthetic_source_info, ExtensionRegistry,
};

/// 构造扩展注册表的入参。
///
/// 把可选的 `trading` 与 `mcp` 状态打包，保持 `create_registry` 单参签名
/// （遵循项目 ≤3 参数规则），同时支持"行情 only / 行情+交易 / +MCP 客户端"组合。
#[derive(Default)]
pub struct RegistryOptions {
    /// 交易态：`Some` 注册 paper-trading 工具（持仓/决策），`None` 仅行情。
    pub trading: Option<TradingContext>,
    /// 已发现的 MCP server（含活跃连接 + 工具元数据）。
    /// `None` 或空表示不接入任何外部 MCP 工具。
    pub mcp: Option<Vec<DiscoveredServer>>,
}

impl RegistryOptions {
    /// 仅行情工具（无交易、无 MCP），用于 MCP 服务端等 market-only 场景。
    #[must_use]
    pub fn market_only() -> Self {
        Self {
            trading: None,
            mcp: None,
        }
    }
}

/// Create an `ExtensionRegistry` with all openatrading extensions + pi-rs built-ins.
///
/// 传入 [`RegistryOptions`] 控制是否启用 paper-trading 与 MCP 客户端工具。
#[must_use]
pub fn create_registry(options: RegistryOptions) -> ExtensionRegistry {
    let mut registry = ExtensionRegistry::new();
    registry.register(
        Box::new(trading::MarketDataExtension::new(options.trading)),
        create_synthetic_source_info("/tmp/test".into(), "market".into(), None, None, None),
    );
    if let Some(discovered) = options.mcp {
        let ext = McpExtension::new(discovered);
        if !ext.is_empty() {
            registry.register(
                Box::new(ext),
                create_synthetic_source_info("/tmp/test".into(), "mcp".into(), None, None, None),
            );
        }
    }
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
