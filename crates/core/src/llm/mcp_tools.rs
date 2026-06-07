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

    /// Call `tools/list` on the MCP server and return the registered tools.
    pub async fn list_tools(&self) -> Result<serde_json::Value, String> {
        let url = format!("{}/mcp", self.endpoint);
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {},
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

        if let Some(err) = json.get("error") {
            let msg = err["message"].as_str().unwrap_or("unknown mcp error");
            return Err(msg.to_string());
        }

        Ok(json["result"]["tools"].clone())
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
// McpRegistry — manages multiple MCP server connections with dynamic discovery
// ============================================================================

/// Configuration for a single MCP server endpoint.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub endpoint: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

/// Registry of MCP server connections.
/// Discovers tools dynamically via `tools/list` on each enabled server.
pub struct McpRegistry {
    clients: Vec<McpClient>,
}

impl McpRegistry {
    /// Build from config list, falling back to `MCP_ENDPOINT` env var if empty.
    pub fn from_config(servers: &[McpServerConfig]) -> Self {
        let mut clients: Vec<McpClient> = servers
            .iter()
            .filter(|s| s.enabled)
            .map(|s| McpClient::new(&s.endpoint))
            .collect();

        if clients.is_empty() {
            // Fallback: single server from env var
            clients.push(McpClient::from_env());
        }

        Self { clients }
    }

    /// Create from a single endpoint (for CLI commands).
    pub fn single(endpoint: &str) -> Self {
        Self {
            clients: vec![McpClient::new(endpoint)],
        }
    }

    /// Discover tools from all enabled MCP servers.
    /// Each discovered tool is wrapped as an `Arc<DynTool>` that calls
    /// the originating server via JSON-RPC `tools/call`.
    pub async fn discover(&self) -> Vec<Arc<DynTool>> {
        let mut all_tools = Vec::new();

        for client in &self.clients {
            match client.list_tools().await {
                Ok(tools) => {
                    if let Some(arr) = tools.as_array() {
                        for tool_data in arr {
                            let name = match tool_data["name"].as_str() {
                                Some(n) => n,
                                None => continue,
                            };
                            let desc = tool_data["description"].as_str().unwrap_or("");
                            let schema = tool_data["inputSchema"].clone();
                            if schema.is_null() {
                                all_tools.push(client.create_tool(
                                    name,
                                    desc,
                                    empty_schema(),
                                    name,
                                ));
                            } else {
                                all_tools.push(client.create_tool(name, desc, schema, name));
                            }
                        }
                    }
                },
                Err(e) => {
                    tracing::warn!("[mcp] discover failed for {}: {e}", client.endpoint);
                },
            }
        }

        all_tools
    }

    /// Return the first client (for CLI mcp command, or when only one server).
    pub fn first(&self) -> Option<&McpClient> {
        self.clients.first()
    }
}

// ============================================================================
// ToolConfig + shared build_tools — one call to assemble all tools
// ============================================================================

/// Configuration passed to [`build_tools`] for tool assembly.
#[derive(Default)]
pub struct ToolConfig {
    pub mcp_servers: Vec<McpServerConfig>,
}


/// One-shot assembly: native tools + MCP server discovery → `ToolRegistry`.
///
/// Call this at entry points instead of repeating the
/// `ToolRegistry::new()` + `register_mcp_tools()` pattern.
pub async fn build_tools(config: &ToolConfig) -> super::tools::ToolRegistry {
    let mut registry = super::tools::ToolRegistry::new();

    let mcp = McpRegistry::from_config(&config.mcp_servers);
    for t in mcp.discover().await {
        registry.add_mcp(t);
    }

    registry
}
