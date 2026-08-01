//! AppContext 级工具列表，供社交适配器 `/tools` 与 Web `/api/tools` 展示。
//!
//! 注意：agent 会话内真正的工具派发走的是 per-session 的 `ExtensionRegistry`
//! （见 `openatrading_agent::session::factory`）。本模块只维护一个轻量列表
//! `ToolRegistry`：扩展工具（行情/交易）的 DynTool 视图 + MCP 客户端工具名。
//!
//! 市场扩展工具用 `handle_tool_call` 而非 `ToolDefinition::execute`，故
//! `extension_tools_as_dyn` 对它们产出为空（与旧实现行为一致）；MCP 客户端
//! 工具名由 `AppContext` 经 `discover_mcp` 发现后注入，用于 `/mcp` 展示。

use std::sync::Arc;

use openatrading_agent::pi_ai_types::ContentBlock;
use openatrading_agent::tools::{to_box_err, tool, ExecuteFn};
use openatrading_agent::types::{AgentToolResult, DynTool};
use pi_extension_api::ExtensionRegistry;

// ============================================================================
// Extension → DynTool bridge
// ============================================================================

/// 将扩展 `RegisteredTool` 中带 `execute` 回调的工具转为 `DynTool`。
///
/// 用 `handle_tool_call` 派发的扩展工具（如行情工具）没有 `execute` 回调，
/// 不会被此处转换——这是已知行为，与旧实现一致。
fn extension_tools_as_dyn(registry: &ExtensionRegistry) -> Vec<Arc<DynTool>> {
    registry
        .tools()
        .iter()
        .filter_map(|rt| {
            let execute = rt.definition.execute.clone()?;
            let name = rt.definition.name.clone();
            let name_for_err = name.clone();
            let description = rt.definition.description.clone();
            let parameters_schema = rt.definition.parameters.clone().unwrap_or_default();

            let execute_fn: ExecuteFn = Arc::new(move |id, args, signal, _progress| {
                let execute = execute.clone();
                let name_for_err = name_for_err.clone();
                Box::pin(async move {
                    let output = execute(id, args, signal)
                        .await
                        .map_err(|e| to_box_err(format!("extension tool '{name_for_err}': {e}")))?;

                    let content: Vec<ContentBlock> = output
                        .content
                        .into_iter()
                        .filter_map(|v| {
                            v.as_str()
                                .map(|s| ContentBlock::text(s.to_string()))
                                .or_else(|| Some(ContentBlock::text(v.to_string())))
                        })
                        .collect();

                    Ok(AgentToolResult {
                        content,
                        details: serde_json::Value::Null,
                        terminate: output.terminate,
                    })
                })
            });

            Some(tool(&name, &description, parameters_schema, execute_fn))
        })
        .collect()
}

// ============================================================================
// ToolRegistry — extension tool list + MCP tool names
// ============================================================================

/// AppContext 级工具列表。
///
/// `extension` 为扩展工具的 DynTool 视图（行情/交易），`mcp_names` 为已发现
/// 外部 MCP server 暴露的工具名，用于 `/mcp` 展示。
pub struct ToolRegistry {
    extension: Vec<Arc<DynTool>>,
    mcp_names: Vec<String>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    pub const fn new() -> Self {
        Self {
            extension: Vec::new(),
            mcp_names: Vec::new(),
        }
    }

    /// 从 `ExtensionRegistry` 载入扩展工具（DynTool 视图）。
    pub fn set_extension_tools(&mut self, ext_registry: &ExtensionRegistry) {
        self.extension = extension_tools_as_dyn(ext_registry);
    }

    /// 设置已发现的 MCP 客户端工具名。
    pub fn set_mcp_names(&mut self, names: Vec<String>) {
        self.mcp_names = names;
    }

    /// 扩展工具（行情/交易等）。
    pub fn extension_tools(&self) -> &[Arc<DynTool>] {
        &self.extension
    }

    /// 所有工具（当前即扩展工具）。
    pub fn all_tools(&self) -> Vec<Arc<DynTool>> {
        self.extension.to_vec()
    }

    /// 已发现的 MCP 客户端工具名。
    pub fn mcp_tools(&self) -> Vec<String> {
        self.mcp_names.clone()
    }
}
