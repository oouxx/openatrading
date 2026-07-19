use std::sync::Arc;

use openatrading_agent::mcp_tools::{McpHandle, McpRegistry, ToolConfig};
use openatrading_agent::pi_ai_types::ContentBlock;
use openatrading_agent::tools::{empty_schema, text_result, to_box_err, tool};
use openatrading_agent::types::{AgentToolResult, DynTool};
use pi_extension_api::{ExtensionRegistry, ToolCallOutput, ToolDefinition, ToolExecuteFn};

use crate::memory::sqlite::MemoryStore;
use crate::trade::sqlite::TradeManager;

// ─── 持仓 & 记忆（per-role 注入）─────────────────────────

/// Create a tool that queries current positions for a given role.
pub fn create_positions_tool(trade: Arc<TradeManager>, role_name: String) -> Arc<DynTool> {
    tool(
        "get_current_positions",
        "查询当前角色的持仓列表。返回股票代码、名称、成本价、持仓数量、\
        当前价格、浮动盈亏、买入理由、持仓占比等。\
        用法：做决策前先查持仓，避免重复买入已持仓股票，\
        根据持仓盈亏决定是否止盈/止损。",
        empty_schema(),
        Arc::new(move |_id, _args, _signal, _progress| {
            let trade = trade.clone();
            let role_name = role_name.clone();
            Box::pin(async move {
                let positions = trade.format_positions(&role_name).map_err(to_box_err)?;
                Ok(text_result(&positions))
            })
        }),
    )
}

/// Create a tool that queries historical context data from memory.
pub fn create_historical_context_tool(memory: Arc<MemoryStore>) -> Arc<DynTool> {
    tool(
        "get_historical_context",
        "查询近N日市场历史快照数据（默认5天）。包含每日市场概览、\
        涨停数据、板块异动等历史记录。\
        用法：对比历史同期数据判断趋势变化——\
        今日温度 vs 近5日均温 = 情绪方向；连续缩量涨停 = 加速赶顶。",
        serde_json::json!({"type":"object","properties":{"days":{"type":"integer","description":"查询天数，默认5","default":5}},"required":[]}),
        Arc::new(move |_id, args, _signal, _progress| {
            let memory = memory.clone();
            Box::pin(async move {
                let days: u32 = args.get("days").and_then(|v| v.as_u64()).unwrap_or(5) as u32;
                let ctx = memory
                    .get_historical_context(days)
                    .unwrap_or_else(|_| "暂无历史数据".to_string());
                Ok(text_result(&ctx))
            })
        }),
    )
}

// ─── ToolDefinition variants (for custom_tools in invoke_agent) ──────────

/// Create a `ToolDefinition` for querying current positions.
pub fn create_positions_tool_def(trade: Arc<TradeManager>, role_name: String) -> ToolDefinition {
    let execute: ToolExecuteFn = Arc::new(move |_id, _args, _signal| {
        let trade = trade.clone();
        let role_name = role_name.clone();
        Box::pin(async move {
            let positions = trade
                .format_positions(&role_name)
                .map_err(|e| e.to_string())?;
            Ok(ToolCallOutput {
                content: vec![serde_json::Value::String(positions)],
                details: None,
                is_error: false,
                terminate: None,
            })
        })
    });

    ToolDefinition {
        name: "get_current_positions".to_string(),
        label: None,
        description: "查询当前角色的持仓列表。返回股票代码、名称、成本价、持仓数量、当前价格、浮动盈亏、买入理由、持仓占比等。用法：做决策前先查持仓，避免重复买入已持仓股票，根据持仓盈亏决定是否止盈/止损。".to_string(),
        prompt_snippet: None,
        prompt_guidelines: None,
        parameters: Some(empty_schema()),
        render_shell: None,
        execution_mode: None,
        execute: Some(execute),
    }
}

/// Create a `ToolDefinition` for querying historical context.
pub fn create_historical_context_tool_def(memory: Arc<MemoryStore>) -> ToolDefinition {
    let execute: ToolExecuteFn = Arc::new(move |_id, args, _signal| {
        let memory = memory.clone();
        Box::pin(async move {
            let days: u32 = args.get("days").and_then(|v| v.as_u64()).unwrap_or(5) as u32;
            let ctx = memory
                .get_historical_context(days)
                .unwrap_or_else(|_| "暂无历史数据".to_string());
            Ok(ToolCallOutput {
                content: vec![serde_json::Value::String(ctx)],
                details: None,
                is_error: false,
                terminate: None,
            })
        })
    });

    ToolDefinition {
        name: "get_historical_context".to_string(),
        label: None,
        description: "查询近N日市场历史快照数据（默认5天）。包含每日市场概览、涨停数据、板块异动等历史记录。用法：对比历史同期数据判断趋势变化——今日温度 vs 近5日均温 = 情绪方向；连续缩量涨停 = 加速赶顶。".to_string(),
        prompt_snippet: None,
        prompt_guidelines: None,
        parameters: Some(serde_json::json!({"type":"object","properties":{"days":{"type":"integer","description":"查询天数，默认5","default":5}},"required":[]})),
        render_shell: None,
        execution_mode: None,
        execute: Some(execute),
    }
}

// ============================================================================
// Extension → DynTool bridge
// ============================================================================

/// Convert extension `RegisteredTool`s into `DynTool` objects.
///
/// Each extension tool's `execute` callback is wrapped to translate between
/// `ToolCallOutput` (extension result) and `AgentToolResult<Value>` (agent result).
pub fn extension_tools_as_dyn(registry: &ExtensionRegistry) -> Vec<Arc<DynTool>> {
    registry
        .collect_tools_from_ref()
        .into_iter()
        .filter_map(|rt| {
            let execute = rt.definition.execute?;
            let name = rt.definition.name.clone();
            let name_for_err = name.clone();
            let description = rt.definition.description.clone();
            let parameters_schema = rt.definition.parameters.clone().unwrap_or_default();

            let execute_fn: openatrading_agent::tools::ExecuteFn =
                Arc::new(move |id, args, signal, _progress| {
                    let execute = execute.clone();
                    let name_for_err = name_for_err.clone();
                    Box::pin(async move {
                        let output = execute(id, args, signal).await.map_err(|e| {
                            to_box_err(format!("extension tool '{name_for_err}': {e}"))
                        })?;

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
    _mcp_handle: Option<McpHandle>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            _mcp_handle: None,
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
        self._mcp_handle = handle;
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
    let (tools, handle) = mcp.discover().await;
    for t in tools {
        registry.add_mcp(t);
    }
    registry.set_mcp_handle(handle);
    registry
}
