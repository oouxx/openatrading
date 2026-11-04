//! AppContext 级工具列表，供 Web `/api/tools` 展示。
//!
//! 工具元数据直接取 pi-rs [`ExtensionRegistry`] 的注册结果——会话内
//! `ToolDefinition → DynTool` 的转换与派发已由 pi-rs 在装配/调用时完成，这里不再
//! 维护一份平行的 `DynTool` 视图。外部 MCP 工具也不在此列出：它们由 pi-rs 的 MCP
//! 客户端在会话装配时注入（见 `llm::mcp`）。

use pi_extension_api::ExtensionRegistry;

/// 工具列表项（展示用）。
#[derive(Debug, Clone)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
}

/// AppContext 级工具列表：扩展注册表里的工具元数据。
#[derive(Debug, Default)]
pub struct ToolRegistry {
    tools: Vec<ToolInfo>,
}

impl ToolRegistry {
    #[must_use]
    pub const fn new() -> Self {
        Self { tools: Vec::new() }
    }

    /// 从 pi-rs `ExtensionRegistry` 载入工具元数据。
    pub fn set_extension_tools(&mut self, ext_registry: &ExtensionRegistry) {
        self.tools = ext_registry
            .tools()
            .iter()
            .map(|rt| ToolInfo {
                name: rt.definition.name.clone(),
                description: rt.definition.description.clone(),
            })
            .collect();
    }

    /// 所有已注册扩展工具。
    #[must_use]
    pub fn all_tools(&self) -> &[ToolInfo] {
        &self.tools
    }
}
