//! Domain-specific tool registry for the openatrading agent.
//!
//! Holds the MCP + extension tool bridge. Per-role domain tool factories
//! (`create_positions_tool`, `create_historical_context_tool`) were removed:
//! they had no callers after the pi-rs session migration; re-wire from
//! `openatrading_core::llm::tools` if per-role position/history tools are
//! needed again (recoverable from git history).

use std::sync::Arc;

use openatrading_agent::mcp_tools::{McpHandle, McpRegistry, ToolConfig};
use openatrading_agent::pi_ai_types::ContentBlock;
use openatrading_agent::tools::{to_box_err, tool, ExecuteFn};
use openatrading_agent::types::{AgentToolResult, DynTool};
use pi_extension_api::ExtensionRegistry;

// ============================================================================
// Extension → DynTool bridge
// ============================================================================

/// Convert extension `RegisteredTool`s into `DynTool` objects.
///
/// Each extension tool's `execute` callback is wrapped to translate between
/// `ToolCallOutput` (extension result) and `AgentToolResult<Value>` (agent
/// result).
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
// ToolRegistry — MCP + extension tools
// ============================================================================

/// Registry for MCP-discovered and extension-provided tools.
///
/// Market data tools, `submit_trading_decision`, etc. are provided by
/// `MarketDataExtension` via the `ExtensionRegistry`. MCP tools are
/// discovered at runtime.
pub struct ToolRegistry {
    mcp: Vec<Arc<DynTool>>,
    extension: Vec<Arc<DynTool>>,
    mcp_handle: Option<McpHandle>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    pub const fn new() -> Self {
        Self {
            mcp_handle: None,
            mcp: Vec::new(),
            extension: Vec::new(),
        }
    }

    /// Register an MCP tool.
    pub fn add_mcp(&mut self, tool: Arc<DynTool>) {
        self.mcp.push(tool);
    }

    /// Load extension tools from an `ExtensionRegistry`.
    pub fn set_extension_tools(&mut self, ext_registry: &ExtensionRegistry) {
        self.extension = extension_tools_as_dyn(ext_registry);
    }

    /// Keep stdio MCP handle alive so subprocess transports don't close.
    pub fn set_mcp_handle(&mut self, handle: Option<McpHandle>) {
        self.mcp_handle = handle;
    }

    pub fn mcp_tools(&self) -> &[Arc<DynTool>] {
        &self.mcp
    }

    /// Extension-provided tools (market data, decision, etc.).
    pub fn extension_tools(&self) -> &[Arc<DynTool>] {
        &self.extension
    }

    /// All tools (MCP + extension).
    pub fn all_tools(&self) -> Vec<Arc<DynTool>> {
        self.mcp
            .iter()
            .chain(self.extension.iter())
            .cloned()
            .collect()
    }
}

// ============================================================================
// build_tools — create ToolRegistry with MCP tools
// ============================================================================

pub async fn build_tools(config: &ToolConfig) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    let mcp = McpRegistry::from_config(&config.mcp_servers);
    let (tools, handle) = mcp.discover();
    for t in tools {
        registry.add_mcp(t);
    }
    registry.set_mcp_handle(handle);
    registry
}
