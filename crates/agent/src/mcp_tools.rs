use std::sync::Arc;

use ahash::AHashMap;
use rmcp::model::{CallToolRequestParams, RawContent};
use rmcp::service::RunningService;
use rmcp::transport::child_process::TokioChildProcess;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::{RoleClient, ServiceExt};
use thiserror::Error;

use pi_coding_agent::pi_agent_core::types::AgentToolResult;
use pi_coding_agent::pi_ai_types::ContentBlock;

use crate::tools::{to_box_err, tool};
use pi_coding_agent::pi_agent_core::types::DynTool;

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

/// Specification for creating an MCP-backed `DynTool`.
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
    /// Connect to an MCP server via URL.
    ///
    /// # Errors
    /// Returns an error if the connection fails.
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
    /// Connect to an MCP server via stdio.
    ///
    /// # Errors
    /// Returns an error if the connection fails.
    pub async fn connect_stdio(
        command: &str,
        args: &[String],
        env: &AHashMap<String, String>,
    ) -> Result<Self, McpError> {
        let mut cmd = tokio::process::Command::new(command);
        cmd.args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit());
        for (k, v) in env {
            cmd.env(k, v);
        }
        tracing::info!(
            "[mcp] connect_stdio: cmd={} args={:?} env={:?} cwd={:?}",
            command,
            args,
            env,
            std::env::current_dir().ok()
        );
        let transport = TokioChildProcess::new(cmd).map_err(|e| McpError::Connect(e.into()))?;
        tracing::info!("[mcp] connect_stdio: child process spawned, serving...");
        let service: RunningService<RoleClient, ()> = ()
            .serve(transport)
            .await
            .map_err(|e| McpError::Connect(e.into()))?;
        let peer = service.peer().clone();
        tracing::info!("[mcp] connect_stdio: handshake complete, service initialized");
        Ok(Self {
            peer,
            _service: service,
        })
    }

    /// List all tools from the server.
    /// List available tools from the MCP server.
    ///
    /// # Errors
    /// Returns an error if the request fails.
    pub async fn list_tools(&self) -> Result<serde_json::Value, McpError> {
        tracing::info!("[mcp] list_tools: calling list_all_tools");
        let tools = self
            .peer
            .list_all_tools()
            .await
            .map_err(|e| McpError::List(e.into()))?;
        tracing::info!("[mcp] list_tools: got {} tools", tools.len());
        Ok(serde_json::to_value(tools).unwrap_or_default())
    }

    /// Call a tool.
    /// Call a tool on the MCP server.
    ///
    /// # Errors
    /// Returns an error if the call fails.
    pub async fn call(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, McpError> {
        tracing::info!("[mcp] call: {tool_name}");
        let args = arguments
            .as_object()
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        let params = CallToolRequestParams::new(tool_name.to_string()).with_arguments(args);
        let start = std::time::Instant::now();
        let result = self.peer.call_tool(params).await.map_err(|e| {
            tracing::error!("[mcp] call call_tool ERROR: {tool_name}: {e}");
            McpError::Call(e.into())
        })?;
        tracing::info!(
            "[mcp] call call_tool OK: {tool_name} took {:?}",
            start.elapsed()
        );

        let text = result
            .content
            .first()
            .and_then(|c| match &c.raw {
                RawContent::Text(t) => Some(t.text.as_str()),
                _ => None,
            })
            .ok_or(McpError::NoContent)?;

        tracing::info!("[mcp] do_call: {tool_name} returned:\n{}", text);
        serde_json::from_str(text).map_err(McpError::Parse)
    }

    /// Call a tool and return the raw text content without JSON parsing.
    /// Call a tool on the MCP server with raw arguments.
    ///
    /// # Errors
    /// Returns an error if the call fails.
    pub async fn call_raw(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<String, McpError> {
        tracing::info!("[mcp] call_raw: {tool_name}");
        tracing::info!(
            "[mcp] call_raw args: {}",
            serde_json::to_string(&arguments).unwrap_or_default()
        );
        let args = arguments
            .as_object()
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        let params = CallToolRequestParams::new(tool_name.to_string()).with_arguments(args);
        tracing::info!("[mcp] call_raw: sending to peer");
        let result = self.peer.call_tool(params).await.map_err(|e| {
            tracing::error!("[mcp] call_raw call_tool ERROR: {tool_name}: {e}");
            McpError::Call(e.into())
        })?;

        let text = result
            .content
            .first()
            .and_then(|c| match &c.raw {
                RawContent::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .ok_or(McpError::NoContent)?;

        tracing::info!("[mcp] call_raw OK: {tool_name}");
        tracing::info!("[mcp] call_raw: {tool_name} returned:\n{}", text);
        Ok(text)
    }

    /// Verify MCP server is functional: list tools and call the first one.
    ///
    /// Returns `(tool_count, call_result)` where `call_result` is `Ok(())` if
    /// the first tool was called successfully, or an error description.
    /// Verify the MCP server connection by listing tools.
    ///
    /// # Errors
    /// Returns an error if the connection fails.
    pub async fn verify(&self) -> Result<(usize, Result<(), String>), McpError> {
        tracing::info!("[mcp] verify: listing tools");
        let tools = self.list_tools().await?;
        let empty: &[serde_json::Value] = &[];
        let arr = tools.as_array().map_or(empty, |v| v.as_slice());
        let count = arr.len();
        tracing::info!("[mcp] verify: got {} tools", count);

        if count == 0 {
            return Ok((0, Ok(())));
        }

        // Try to call the first tool with empty args
        let first_name: String = match arr[0]["name"].as_str() {
            Some(n) => n.to_string(),
            None => return Ok((count, Ok(()))),
        };

        match self.call_raw(&first_name, serde_json::json!({})).await {
            Ok(_) => Ok((count, Ok(()))),
            Err(e) => Ok((count, Err(e.to_string()))),
        }
    }

    /// Wrap this client into a `DynTool` factory.
    pub fn create_tool(&self, spec: McpToolSpec) -> Arc<DynTool> {
        let peer = self.peer.clone();
        let McpToolSpec {
            name,
            description,
            schema,
            method,
        } = spec;
        let execute: crate::tools::ExecuteFn = Arc::new(move |_id, args, _signal, _progress| {
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

        tool(&name, &description, schema, execute)
    }

    async fn do_call(
        peer: &rmcp::service::Peer<RoleClient>,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, McpError> {
        tracing::info!("[mcp] do_call: {tool_name}");
        tracing::info!(
            "[mcp] do_call args: {}",
            serde_json::to_string(&arguments).unwrap_or_default()
        );
        let args = arguments
            .as_object()
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        let params = CallToolRequestParams::new(tool_name.to_string()).with_arguments(args);
        tracing::info!("[mcp] do_call: sending to peer");
        let result = peer.call_tool(params).await.map_err(|e| {
            tracing::error!("[mcp] do_call: call_tool ERROR: {tool_name}: {e}");
            McpError::Call(e.into())
        })?;

        let text = result
            .content
            .first()
            .and_then(|c| match &c.raw {
                RawContent::Text(t) => Some(t.text.as_str()),
                _ => None,
            })
            .ok_or(McpError::NoContent)?;

        tracing::info!("[mcp] do_call: {tool_name} returned:\n{}", text);
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
        env: AHashMap<String, String>,
    },
}

impl McpTransportConfig {
    pub fn url(&self) -> &str {
        match self {
            Self::StreamableHttp { url, .. } | Self::Sse { url, .. } => url,
            Self::Stdio { command, .. } => command,
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

impl McpServerConfig {
    /// Create a new config with just a name (for testing/UI stubs).
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            transport: McpTransportConfig::StreamableHttp {
                url: "http://localhost:8081/mcp".to_string(),
                auth_token: None,
            },
            enabled: true,
        }
    }
}

fn default_transport() -> McpTransportConfig {
    McpTransportConfig::StreamableHttp {
        url: "http://localhost:8081/mcp".to_string(),
        auth_token: None,
    }
}
const fn default_enabled() -> bool {
    true
}

/// Registry of MCP server connections.
pub struct McpRegistry;

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
        Self
    }

    pub fn single(_endpoint: &str) -> Self {
        Self
    }

    pub fn discover(&self) -> (Vec<Arc<DynTool>>, Option<McpHandle>) {
        tracing::warn!("[mcp] discovery requires pi-agent-core's `mcp` feature — stubbed");
        (Vec::new(), None)
    }
}

/// Stub: pi-agent-core's `McpHandle` requires its `mcp` feature.
/// Re-enable when pi-coding-agent propagates the feature.
#[derive(Clone)]
pub struct McpHandle;

// ============================================================================
// ToolConfig
// ============================================================================

#[derive(Default)]
pub struct ToolConfig {
    pub mcp_servers: Vec<McpServerConfig>,
}
