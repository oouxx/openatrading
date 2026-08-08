//! MCP 客户端：把外部 MCP server 的工具接入 pi-rs 扩展体系（rmcp 3.x）。
//!
//! [`McpClient`] 封装 rmcp 的 client 传输（streamable HTTP / stdio 子进程），
//! 负责连接、列举工具、调用工具。[`McpExtension`] 实现 pi-rs 的
//! [`HookHandler`](pi_extension_api::HookHandler)，把发现的远端工具注册进
//! `ExtensionRegistry`，并在 `handle_tool_call` 中把调用路由回对应 MCP server。
//!
//! pi-rs v1.81.0 已落地 `mcp` feature，但仅限 ACP 会话内使用（按 ACP `McpServer`
//! 配置连接），未通过 pi-coding-agent SDK 暴露通用 MCP 客户端能力；本项目需直连
//! 外部 server 并经 HookHandler 接入所有会话类型，故仍自实现 rmcp↔HookHandler 桥接。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use ahash::AHashMap;
use rmcp::model::{CallToolRequestParams, CallToolResult, ContentBlock, Tool};
use rmcp::service::{RoleClient, RunningService, ServiceExt};
use rmcp::transport::{StreamableHttpClientTransport, TokioChildProcess};
use serde_json::Value;
use thiserror::Error;

use async_trait::async_trait;
use pi_extension_api::{
    ExtensionContext, HookHandler, ToolCallOutput, ToolDefinition, ToolRegistry,
};

// ============================================================================
// 超时控制
// ============================================================================

/// MCP 连接握手（rmcp initialize）超时。
///
/// 远端 server 无响应（网络挂起、服务卡死）时，rmcp 的握手会无限等待，
/// 阻塞网关/CLI 启动。健康 server 的握手应在亚秒级完成，5s 足以覆盖慢速冷启动。
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// MCP 工具列举（`list_tools`）超时：连接成功但响应停滞时避免发现流程无限等待。
const LIST_TOOLS_TIMEOUT: Duration = Duration::from_secs(5);

// ============================================================================
// McpError
// ============================================================================

#[derive(Debug, Error)]
pub enum McpError {
    #[error("connect: {0}")]
    Connect(String),
    #[error("list: {0}")]
    List(String),
    #[error("call: {0}")]
    Call(String),
    #[error("mcp: no text content")]
    NoContent,
    #[error("result parse: {0}")]
    Parse(#[from] serde_json::Error),
}

// ============================================================================
// McpClient — rmcp client over streamable HTTP / stdio
// ============================================================================

/// MCP 客户端：持有一个到 MCP server 的活跃连接，用于列举/调用远端工具。
///
/// 通过 `Arc<McpClient>` 在多个会话/服务端之间共享同一条连接。
pub struct McpClient {
    service: RunningService<RoleClient, ()>,
}

impl McpClient {
    /// 通过 streamable HTTP 连接 MCP server。
    ///
    /// # Errors
    /// 连接失败时返回 `McpError::Connect`。
    pub async fn connect(endpoint: impl Into<String>) -> Result<Self, McpError> {
        let transport = StreamableHttpClientTransport::from_uri(endpoint.into());
        let service = tokio::time::timeout(CONNECT_TIMEOUT, ().serve(transport))
            .await
            .map_err(|_| {
                McpError::Connect(format!("handshake timed out after {CONNECT_TIMEOUT:?}"))
            })?
            .map_err(|e| McpError::Connect(e.to_string()))?;
        Ok(Self { service })
    }

    /// 通过 stdio 子进程连接 MCP server。
    ///
    /// # Errors
    /// 子进程启动或握手失败时返回 `McpError::Connect`。
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
        let transport =
            TokioChildProcess::new(cmd).map_err(|e| McpError::Connect(e.to_string()))?;
        let service = tokio::time::timeout(CONNECT_TIMEOUT, ().serve(transport))
            .await
            .map_err(|_| {
                McpError::Connect(format!("handshake timed out after {CONNECT_TIMEOUT:?}"))
            })?
            .map_err(|e| McpError::Connect(e.to_string()))?;
        tracing::info!("[mcp] connect_stdio: handshake complete, service initialized");
        Ok(Self { service })
    }

    /// 列出 server 上的全部工具，序列化为 JSON 数组（兼容 CLI 展示）。
    ///
    /// # Errors
    /// 请求失败时返回 `McpError::List`。
    pub async fn list_tools(&self) -> Result<Value, McpError> {
        let tools = self.list_all_tools_with_timeout().await?;
        tracing::info!("[mcp] list_tools: got {} tools", tools.len());
        Ok(serde_json::to_value(&tools).unwrap_or_default())
    }

    /// 调用工具并把首个文本结果解析为 JSON。
    ///
    /// # Errors
    /// 调用失败、无文本内容或 JSON 解析失败时返回对应 `McpError`。
    pub async fn call(&self, tool_name: &str, arguments: Value) -> Result<Value, McpError> {
        let text = self.call_raw(tool_name, arguments).await?;
        serde_json::from_str(&text).map_err(McpError::Parse)
    }

    /// 调用工具并返回首个文本内容的原始字符串。
    ///
    /// # Errors
    /// 调用失败或无文本内容时返回对应 `McpError`。
    pub async fn call_raw(&self, tool_name: &str, arguments: Value) -> Result<String, McpError> {
        tracing::info!("[mcp] call_raw: {tool_name}");
        let args = arguments
            .as_object()
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        let params = CallToolRequestParams::new(tool_name.to_string()).with_arguments(args);
        let result: CallToolResult = self
            .service
            .peer()
            .call_tool(params)
            .await
            .map_err(|e| McpError::Call(e.to_string()))?;

        let text = result
            .content
            .first()
            .and_then(|c| match c {
                ContentBlock::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .ok_or(McpError::NoContent)?;

        tracing::info!("[mcp] call_raw OK: {tool_name}");
        Ok(text)
    }

    /// 健康检查：列出工具并尝试以空参调用第一个工具。
    ///
    /// # Errors
    /// 列举失败时返回 `McpError`；首个工具调用失败时通过 `Result<()>` 项返回描述。
    pub async fn verify(&self) -> Result<(usize, Result<(), String>), McpError> {
        let tools = self.list_tools().await?;
        let empty: &[Value] = &[];
        let arr = tools.as_array().map_or(empty, Vec::as_slice);
        let count = arr.len();
        if count == 0 {
            return Ok((0, Ok(())));
        }
        let first_name: String = match arr[0]["name"].as_str() {
            Some(n) => n.to_string(),
            None => return Ok((count, Ok(()))),
        };
        match self.call_raw(&first_name, serde_json::json!({})).await {
            Ok(_) => Ok((count, Ok(()))),
            Err(e) => Ok((count, Err(e.to_string()))),
        }
    }

    /// 原始列举工具元数据（供发现使用，不做 JSON 序列化往返）。
    async fn list_specs(&self) -> Result<Vec<RemoteToolSpec>, McpError> {
        let tools = self.list_all_tools_with_timeout().await?;
        Ok(tools
            .into_iter()
            .map(|t| RemoteToolSpec {
                name: t.name.into_owned(),
                description: t.description.map(|c| c.into_owned()).unwrap_or_default(),
                schema: Value::Object((*t.input_schema).clone()),
            })
            .collect())
    }

    /// 带超时地列举 server 全部工具；响应停滞时返回 `McpError::List`。
    async fn list_all_tools_with_timeout(&self) -> Result<Vec<Tool>, McpError> {
        tokio::time::timeout(LIST_TOOLS_TIMEOUT, self.service.peer().list_all_tools())
            .await
            .map_err(|_| McpError::List(format!("timed out after {LIST_TOOLS_TIMEOUT:?}")))?
            .map_err(|e| McpError::List(e.to_string()))
    }
}

// ============================================================================
// Transport / server config
// ============================================================================

/// MCP 连接的传输协议配置。
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
    /// 用于日志展示的端点标识。
    pub fn url(&self) -> &str {
        match self {
            Self::StreamableHttp { url, .. } | Self::Sse { url, .. } => url,
            Self::Stdio { command, .. } => command,
        }
    }

    /// 按配置建立一条 MCP 客户端连接。
    ///
    /// # Errors
    /// 连接失败时返回 `McpError`。
    pub async fn connect(&self) -> Result<McpClient, McpError> {
        match self {
            Self::StreamableHttp { url, .. } | Self::Sse { url, .. } => {
                McpClient::connect(url.to_string()).await
            }
            Self::Stdio {
                command, args, env, ..
            } => McpClient::connect_stdio(command, args, env).await,
        }
    }
}

/// 单个 MCP server 的配置。
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct McpServerConfig {
    pub name: String,
    #[serde(default = "default_transport")]
    pub transport: McpTransportConfig,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

impl McpServerConfig {
    /// 仅供测试/UI 占位：用默认 streamable-http 端点构造。
    #[must_use]
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

// ============================================================================
// Discovery
// ============================================================================

/// 从远端 `list_tools` 解析出的单个工具元数据。
#[derive(Clone)]
pub struct RemoteToolSpec {
    pub name: String,
    pub description: String,
    pub schema: Value,
}

/// 一条已连接 MCP server 的发现结果：客户端连接 + 它暴露的工具元数据。
#[derive(Clone)]
pub struct DiscoveredServer {
    pub name: String,
    pub client: Arc<McpClient>,
    pub specs: Vec<RemoteToolSpec>,
}

/// 连接所有启用中的 MCP server 并发现其工具。
///
/// 单个 server 连接/发现失败不会中断整体：失败的 server 被跳过并记录日志，
/// 以保证其余 server 仍可用。
pub async fn discover_mcp(servers: &[McpServerConfig]) -> Vec<DiscoveredServer> {
    let mut discovered = Vec::new();
    for cfg in servers.iter().filter(|s| s.enabled) {
        match cfg.transport.connect().await {
            Ok(client) => {
                let client = Arc::new(client);
                match client.list_specs().await {
                    Ok(specs) => {
                        tracing::info!(
                            "[mcp] discovered server '{}' -> {} tools",
                            cfg.name,
                            specs.len()
                        );
                        discovered.push(DiscoveredServer {
                            name: cfg.name.clone(),
                            client,
                            specs,
                        });
                    }
                    Err(e) => {
                        tracing::warn!("[mcp] server '{}' list_tools failed: {e}", cfg.name);
                    }
                }
            }
            Err(e) => tracing::warn!("[mcp] server '{}' connect failed: {e}", cfg.name),
        }
    }
    discovered
}

// ============================================================================
// McpExtension — pi-rs HookHandler 桥接 MCP 客户端工具
// ============================================================================

/// 把外部 MCP server 的工具接入 pi-rs 的扩展。
///
/// 实现 [`HookHandler`]：`register_tools` 注册发现的远端工具元数据，
/// `handle_tool_call` 按名路由到对应 `McpClient` 并把 rmcp 结果翻译回
/// pi-rs 的 `ToolCallOutput`。多个会话可通过克隆 `Arc<McpClient>` 共享连接，
/// 但每个 `ExtensionRegistry` 需要自己的 `McpExtension` 实例
/// （`HookHandler` 经 `Box` 注册，不可跨 registry 复用）。
pub struct McpExtension {
    /// 工具名 → 所属 server 的客户端连接。
    routes: HashMap<String, Arc<McpClient>>,
    /// 待注册的远端工具元数据。
    specs: Vec<RemoteToolSpec>,
}

impl McpExtension {
    /// 用发现结果构造。传入空切片等价于不注册任何 MCP 工具。
    #[must_use]
    pub fn new(discovered: Vec<DiscoveredServer>) -> Self {
        let mut routes = HashMap::new();
        let mut specs = Vec::new();
        for srv in discovered {
            for spec in &srv.specs {
                routes.insert(spec.name.clone(), Arc::clone(&srv.client));
            }
            specs.extend(srv.specs);
        }
        Self { routes, specs }
    }

    /// 是否注册了任何 MCP 工具。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.specs.is_empty()
    }
}

#[async_trait]
impl HookHandler for McpExtension {
    fn name(&self) -> &'static str {
        "openatrading-mcp-client"
    }

    fn register_tools(&self, registry: &mut ToolRegistry) {
        for spec in &self.specs {
            let def = ToolDefinition {
                name: spec.name.clone(),
                description: spec.description.clone(),
                parameters: Some(spec.schema.clone()),
                ..Default::default()
            };
            registry.register(&def.name.clone(), def);
        }
    }

    async fn handle_tool_call(
        &self,
        tool_name: &str,
        params: Value,
        _ctx: &ExtensionContext,
    ) -> Option<ToolCallOutput> {
        let client = self.routes.get(tool_name)?;
        match client.call(tool_name, params).await {
            Ok(result) => {
                let text = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".into());
                Some(ToolCallOutput {
                    content: vec![serde_json::json!({ "type": "text", "text": text })],
                    details: None,
                    is_error: false,
                    terminate: None,
                })
            }
            Err(e) => Some(ToolCallOutput {
                content: vec![serde_json::json!({ "type": "text", "text": e.to_string() })],
                details: None,
                is_error: true,
                terminate: None,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 挂起的 MCP server（accept 连接但永不响应 HTTP 请求）不应阻塞发现流程：
    /// `discover_mcp` 必须在超时上界内返回并跳过该 server。
    #[tokio::test]
    async fn discover_mcp_skips_hung_server_without_blocking() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap_or_else(|e| panic!("bind test listener: {e}"));
        let addr = listener
            .local_addr()
            .unwrap_or_else(|e| panic!("local addr: {e}"));
        tokio::spawn(async move {
            loop {
                if listener.accept().await.is_err() {
                    break;
                }
            }
        });

        let cfg = McpServerConfig {
            name: "hung".to_string(),
            transport: McpTransportConfig::StreamableHttp {
                url: format!("http://{addr}/mcp"),
                auth_token: None,
            },
            enabled: true,
        };

        let started = std::time::Instant::now();
        let discovered = discover_mcp(&[cfg]).await;
        let elapsed = started.elapsed();

        assert!(discovered.is_empty(), "hung server must be skipped");
        let upper_bound = CONNECT_TIMEOUT + LIST_TOOLS_TIMEOUT + Duration::from_secs(2);
        assert!(
            elapsed < upper_bound,
            "discovery blocked too long: {elapsed:?} (upper bound {upper_bound:?})"
        );
    }
}
