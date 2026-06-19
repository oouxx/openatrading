use std::sync::Arc;

use rmcp::model::{CallToolRequestParams, RawContent};
use rmcp::service::RunningService;
use rmcp::transport::child_process::TokioChildProcess;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::{RoleClient, ServiceExt};
use std::collections::HashMap;
use thiserror::Error;

use super::tools::{empty_schema, to_box_err};
use crate::llm::DynTool;
use openatrading_agent::pi_ai_types::ContentBlock;
use openatrading_agent::types::AgentToolResult;

#[derive(Debug, Error)]
pub enum McpError {
    #[error("connect: {0}")]
    Connect(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("list: {0}")]
    List(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("call: {0}")]
    Call(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("mcp: no text content")]
    NoContent,
    #[error("result parse: {0}")]
    Parse(#[from] serde_json::Error),
}

/// Specification for creating an MCP-backed DynTool.
pub struct McpToolSpec {
    pub name: String,
    pub description: String,
    pub schema: serde_json::Value,
    pub method: String,
}

// ============================================================================
// McpClient — full-stack rmcp client over streamable HTTP
// ============================================================================

/// MCP client using rmcp's full transport stack (SSE + HTTP POST).
///
/// Connects on construction, holds the peer for subsequent tool calls.
pub struct McpClient {
    peer: rmcp::service::Peer<RoleClient>,
    _service: RunningService<RoleClient, ()>,
}

impl McpClient {
    /// Connect to an MCP server via streamable HTTP transport.
    pub async fn connect(endpoint: impl Into<String>) -> Result<Self, McpError> {
        let config = StreamableHttpClientTransportConfig::with_uri(endpoint.into());
        let transport =
            rmcp::transport::streamable_http_client::StreamableHttpClientTransport::from_config(
                config,
            );
        let service: RunningService<RoleClient, ()> = ()
            .serve(transport)
            .await
            .map_err(|e| McpError::Connect(e.into()))?;
        let peer = service.peer().clone();
        Ok(Self {
            peer,
            _service: service,
        })
    }

    /// Connect to an MCP server via stdio subprocess.
    pub async fn connect_stdio(
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
    ) -> Result<Self, McpError> {
        let mut cmd = tokio::process::Command::new(command);
        cmd.args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit());
        for (k, v) in env {
            cmd.env(k, v);
        }
        let transport = TokioChildProcess::new(cmd).map_err(|e| McpError::Connect(e.into()))?;
        let service: RunningService<RoleClient, ()> = ()
            .serve(transport)
            .await
            .map_err(|e| McpError::Connect(e.into()))?;
        let peer = service.peer().clone();
        Ok(Self {
            peer,
            _service: service,
        })
    }

    /// List all tools from the server.
    pub async fn list_tools(&self) -> Result<serde_json::Value, McpError> {
        let tools = self
            .peer
            .list_all_tools()
            .await
            .map_err(|e| McpError::List(e.into()))?;
        Ok(serde_json::to_value(tools).unwrap_or_default())
    }

    /// Call a tool.
    pub async fn call(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, McpError> {
        let args = arguments
            .as_object()
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        let params = CallToolRequestParams::new(tool_name.to_string()).with_arguments(args);
        let result = self
            .peer
            .call_tool(params)
            .await
            .map_err(|e| McpError::Call(e.into()))?;

        let text = result
            .content
            .first()
            .and_then(|c| match &c.raw {
                RawContent::Text(t) => Some(t.text.as_str()),
                _ => None,
            })
            .ok_or(McpError::NoContent)?;

        serde_json::from_str(text).map_err(McpError::Parse)
    }

    /// Wrap this client into a DynTool factory.
    pub fn create_tool(&self, spec: McpToolSpec) -> Arc<DynTool> {
        let peer = self.peer.clone();
        let McpToolSpec {
            name,
            description,
            schema,
            method,
        } = spec;
        let execute: super::tools::ExecuteFn = Arc::new(move |_id, args, _signal, _progress| {
            let peer = peer.clone();
            let method = method.clone();
            Box::pin(async move {
                match Self::do_call(&peer, &method, args).await {
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

        super::tools::tool(&name, &description, schema, execute)
    }

    async fn do_call(
        peer: &rmcp::service::Peer<RoleClient>,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, McpError> {
        let args = arguments
            .as_object()
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        let params = CallToolRequestParams::new(tool_name.to_string()).with_arguments(args);
        let result = peer
            .call_tool(params)
            .await
            .map_err(|e| McpError::Call(e.into()))?;

        let text = result
            .content
            .first()
            .and_then(|c| match &c.raw {
                RawContent::Text(t) => Some(t.text.as_str()),
                _ => None,
            })
            .ok_or(McpError::NoContent)?;

        serde_json::from_str(text).map_err(McpError::Parse)
    }
}

// ============================================================================
// McpRegistry — manages multiple MCP server connections with dynamic discovery
// ============================================================================

/// Transport protocol for an MCP connection.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(tag = "type")]
pub enum McpTransportConfig {
    #[serde(rename = "streamable-http")]
    StreamableHttp {
        url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        auth_token: Option<String>,
    },
    #[serde(rename = "sse")]
    Sse {
        url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        post_url: Option<String>,
    },
    #[serde(rename = "stdio")]
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
    },
}

impl McpTransportConfig {
    pub fn url(&self) -> &str {
        match self {
            McpTransportConfig::StreamableHttp { url, .. } => url,
            McpTransportConfig::Sse { url, .. } => url,
            McpTransportConfig::Stdio { command, .. } => command,
        }
    }
}

/// Configuration for a single MCP server.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct McpServerConfig {
    pub name: String,
    #[serde(default = "default_transport")]
    pub transport: McpTransportConfig,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_transport() -> McpTransportConfig {
    McpTransportConfig::StreamableHttp {
        url: "http://localhost:8081/mcp".to_string(),
        auth_token: None,
    }
}
fn default_enabled() -> bool {
    true
}

/// Registry of MCP server connections.
pub struct McpRegistry {
    servers: Vec<McpServerConfig>,
}

impl McpRegistry {
    pub fn from_config(servers: &[McpServerConfig]) -> Self {
        let enabled: Vec<&McpServerConfig> = servers.iter().filter(|s| s.enabled).collect();
        if enabled.is_empty() {
            tracing::info!("[mcp] no servers configured");
        } else {
            for s in &enabled {
                tracing::info!("[mcp] register server: {} ({})", s.name, s.transport.url());
            }
        }
        Self {
            servers: servers.iter().filter(|s| s.enabled).cloned().collect(),
        }
    }

    pub fn single(endpoint: &str) -> Self {
        Self {
            servers: vec![McpServerConfig {
                name: "cli".into(),
                transport: McpTransportConfig::StreamableHttp {
                    url: endpoint.to_string(),
                    auth_token: None,
                },
                enabled: true,
            }],
        }
    }

    pub async fn discover(&self) -> Vec<Arc<DynTool>> {
        let mut all_tools = Vec::new();
        for server in &self.servers {
            let client = match &server.transport {
                McpTransportConfig::StreamableHttp { url, .. } => McpClient::connect(url).await,
                McpTransportConfig::Sse { url, .. } => McpClient::connect(url).await,
                McpTransportConfig::Stdio { command, args, env } => {
                    McpClient::connect_stdio(command, args, env).await
                },
            };
            let client = match client {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!("[mcp] {}: connect failed: {e}", server.name);
                    continue;
                },
            };
            match client.list_tools().await {
                Ok(tools) => {
                    let count = tools.as_array().map(|a| a.len()).unwrap_or(0);
                    if let Some(arr) = tools.as_array() {
                        for tool_data in arr {
                            let name = match tool_data["name"].as_str() {
                                Some(n) => n,
                                None => continue,
                            };
                            let desc = tool_data["description"].as_str().unwrap_or("");
                            let schema = tool_data["inputSchema"].clone();
                            all_tools.push(client.create_tool(McpToolSpec {
                                name: name.to_string(),
                                description: desc.to_string(),
                                schema: if schema.is_null() {
                                    empty_schema()
                                } else {
                                    schema
                                },
                                method: name.to_string(),
                            }));
                        }
                    }
                    tracing::info!("[mcp] {}: discovered {} tools", server.name, count);
                },
                Err(e) => tracing::warn!("[mcp] {}: discover failed: {e}", server.name),
            }
        }
        if all_tools.is_empty() {
            tracing::info!("[mcp] no tools discovered");
        }
        all_tools
    }
}

// ============================================================================
// ToolConfig + shared build_tools
// ============================================================================

#[derive(Default)]
pub struct ToolConfig {
    pub mcp_servers: Vec<McpServerConfig>,
}

pub async fn build_tools(config: &ToolConfig) -> super::tools::ToolRegistry {
    let mut registry = super::tools::ToolRegistry::new();
    let mcp = McpRegistry::from_config(&config.mcp_servers);
    for t in mcp.discover().await {
        registry.add_mcp(t);
    }
    registry
}
