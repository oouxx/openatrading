use std::sync::Arc;

use super::tools::{empty_schema, to_box_err};
use crate::llm::DynTool;
use openatrading_agent::pi_ai_types::ContentBlock;
use openatrading_agent::types::AgentToolResult;

// ============================================================================
// McpClient — standard MCP JSON-RPC 2.0 client over HTTP
// ============================================================================

/// Lightweight MCP client speaking JSON-RPC 2.0 over HTTP.
/// Sends POST requests to `{endpoint}/mcp` and parses MCP responses.
#[derive(Clone)]
pub struct McpClient {
    endpoint: String,
    client: reqwest::Client,
}

impl McpClient {
    /// Create client with explicit endpoint (e.g. "http://localhost:8081").
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            client: reqwest::Client::new(),
        }
    }

    /// Read endpoint from `MCP_ENDPOINT` env var, or use default.
    pub fn from_env() -> Self {
        let ep =
            std::env::var("MCP_ENDPOINT").unwrap_or_else(|_| "http://localhost:8081".to_string());
        Self::new(ep)
    }

    /// Call an MCP tool via JSON-RPC 2.0 `tools/call`.
    pub async fn call(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let url = format!("{}/mcp", self.endpoint);
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": arguments,
            },
        });
        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("mcp connection refused: {e}"))?;

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("mcp parse failed: {e}"))?;

        // JSON-RPC error
        if let Some(err) = json.get("error") {
            let msg = err["message"].as_str().unwrap_or("unknown mcp error");
            return Err(msg.to_string());
        }

        let result = json.get("result").ok_or("mcp: no result")?;

        // MCP tool-level error
        if result
            .get("isError")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            let msg = result["content"]
                .as_array()
                .and_then(|arr| arr.first())
                .and_then(|c| c.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown mcp tool error");
            return Err(msg.to_string());
        }

        // Extract text from first content block
        let text = result["content"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|c| c.get("text"))
            .and_then(|v| v.as_str())
            .ok_or("mcp: no text content")?;

        // The text field is a JSON string (dataframe serialized by the server)
        serde_json::from_str(text).map_err(|e| format!("mcp result parse: {e}"))
    }

    /// Wrap this client into a tool factory for the given akshare method.
    /// The tool will call `.call(method, args)` when executed.
    pub fn create_tool(
        &self,
        name: &str,
        description: &str,
        schema: serde_json::Value,
        method: &str,
    ) -> Arc<DynTool> {
        let client = self.clone();
        let method = method.to_string();
        let execute: super::tools::ExecuteFn = Arc::new(move |_id, args, _signal, _progress| {
            let client = client.clone();
            let method = method.clone();
            Box::pin(async move {
                match client.call(&method, args).await {
                    Ok(result) => Ok(AgentToolResult {
                        content: vec![ContentBlock::text(
                            serde_json::to_string_pretty(&result)
                                .unwrap_or_else(|_| "{}".to_string()),
                        )],
                        details: serde_json::Value::Null,
                        terminate: None,
                    }),
                    Err(e) => Err(to_box_err(e)),
                }
            })
        });

        super::tools::tool(name, description, schema, execute)
    }
}

// ============================================================================
// Default MCP tool registration
// ============================================================================

/// Register akshare-one-mcp tools into the given ToolRegistry.
pub fn register_mcp_tools(registry: &mut super::tools::ToolRegistry, client: &McpClient) {
    // ── 历史行情（含技术指标）────────────────────
    registry.add_mcp(client.create_tool(
        "get_hist_data",
        "获取个股历史K线行情数据，支持日/周/月/分钟线，可选30+种技术指标(MA,MACD,RSI,BOLL等)。参数：symbol(股票代码), interval(周期:minute/hour/day/week/month/year), start_date(YYYY-MM-DD), end_date(YYYY-MM-DD), adjust(复权:none/qfq/hfq), source(数据源:eastmoney/eastmoney_direct/sina), indicators_list(技术指标列表), recent_n(返回最近N条)",
        serde_json::json!({
            "type": "object",
            "properties": {
                "symbol": {"type": "string", "description": "股票代码，如 000001"},
                "interval": {"type": "string", "description": "K线周期: minute/day/week/month/year", "default": "day"},
                "start_date": {"type": "string", "description": "开始日期 YYYY-MM-DD", "default": "1970-01-01"},
                "end_date": {"type": "string", "description": "结束日期 YYYY-MM-DD", "default": "2030-12-31"},
                "adjust": {"type": "string", "description": "复权方式: none/qfq(前复权)/hfq(后复权)", "default": "none"},
                "source": {"type": "string", "description": "数据源: eastmoney/eastmoney_direct/sina", "default": "eastmoney"},
                "recent_n": {"type": "integer", "description": "返回最近N条记录，默认100", "default": 100}
            },
            "required": ["symbol"],
        }),
        "get_hist_data",
    ));

    // ── 实时行情 ────────────────────────────────
    registry.add_mcp(client.create_tool(
        "get_realtime_data",
        "获取个股实时行情数据，包含最新价、涨跌幅、成交量等。留空symbol返回全市场行情。source可选: xueqiu/eastmoney/eastmoney_direct",
        serde_json::json!({
            "type": "object",
            "properties": {
                "symbol": {"type": "string", "description": "股票代码，留空返回全市场", "default": ""},
                "source": {"type": "string", "description": "数据源: xueqiu/eastmoney/eastmoney_direct", "default": "eastmoney_direct"},
            },
            "required": [],
        }),
        "get_realtime_data",
    ));

    // ── 个股新闻 ────────────────────────────────
    registry.add_mcp(client.create_tool(
        "get_news_data",
        "获取个股相关新闻资讯",
        serde_json::json!({
            "type": "object",
            "properties": {
                "symbol": {"type": "string", "description": "股票代码，如 000001"},
                "recent_n": {"type": "integer", "description": "返回最近N条，默认10", "default": 10},
            },
            "required": ["symbol"],
        }),
        "get_news_data",
    ));

    // ── 资产负债表 ──────────────────────────────
    registry.add_mcp(client.create_tool(
        "get_balance_sheet",
        "获取公司资产负债表数据，用于财务分析",
        serde_json::json!({
            "type": "object",
            "properties": {
                "symbol": {"type": "string", "description": "股票代码，如 000001"},
                "recent_n": {"type": "integer", "description": "返回最近N期，默认10", "default": 10},
            },
            "required": ["symbol"],
        }),
        "get_balance_sheet",
    ));

    // ── 利润表 ──────────────────────────────────
    registry.add_mcp(client.create_tool(
        "get_income_statement",
        "获取公司利润表/损益表数据",
        serde_json::json!({
            "type": "object",
            "properties": {
                "symbol": {"type": "string", "description": "股票代码，如 000001"},
                "recent_n": {"type": "integer", "description": "返回最近N期，默认10", "default": 10},
            },
            "required": ["symbol"],
        }),
        "get_income_statement",
    ));

    // ── 现金流量表 ──────────────────────────────
    registry.add_mcp(client.create_tool(
        "get_cash_flow",
        "获取公司现金流量表数据",
        serde_json::json!({
            "type": "object",
            "properties": {
                "symbol": {"type": "string", "description": "股票代码，如 000001"},
                "source": {"type": "string", "description": "数据源: sina", "default": "sina"},
                "recent_n": {"type": "integer", "description": "返回最近N期，默认10", "default": 10},
            },
            "required": ["symbol"],
        }),
        "get_cash_flow",
    ));

    // ── 内部交易 ────────────────────────────────
    registry.add_mcp(client.create_tool(
        "get_inner_trade_data",
        "获取公司内部人交易/增减持数据",
        serde_json::json!({
            "type": "object",
            "properties": {
                "symbol": {"type": "string", "description": "股票代码，如 000001"},
            },
            "required": ["symbol"],
        }),
        "get_inner_trade_data",
    ));

    // ── 财务指标 ────────────────────────────────
    registry.add_mcp(client.create_tool(
        "get_financial_metrics",
        "获取公司关键财务指标（三大报表核心数据）",
        serde_json::json!({
            "type": "object",
            "properties": {
                "symbol": {"type": "string", "description": "股票代码，如 000001"},
                "recent_n": {"type": "integer", "description": "返回最近N期，默认10", "default": 10},
            },
            "required": ["symbol"],
        }),
        "get_financial_metrics",
    ));

    // ── 时间/交易日历 ──────────────────────────
    registry.add_mcp(client.create_tool(
        "get_time_info",
        "获取当前时间（ISO格式+时间戳）和最近交易日信息",
        empty_schema(),
        "get_time_info",
    ));
}
