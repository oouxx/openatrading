//! MCP 服务端：把 pi-rs 扩展注册的工具对外暴露为标准 MCP server（rmcp 3.x）。
//!
//! [`McpServerHandler`] 实现 rmcp 的 [`ServerHandler`](rmcp::handler::server::ServerHandler)，
//! 但不依赖 `DynTool`/`pi-coding-agent`：它持有一份 `Arc<ExtensionRegistry>`，
//! `call_tool` 经 pi-extension-api 的 `HookRunner::dispatch_tool_call` 派发——
//! 与 agent 会话内调用同一套工具的路径完全一致。
//!
//! 这同时修复了旧实现中 `openatrading mcp serve` 因基于空的
//! `ToolRegistry` 而对外暴露 0 个工具的问题。

use std::net::SocketAddr;
use std::sync::Arc;

use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, ContentBlock, ErrorCode, Implementation,
    ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::transport::StreamableHttpServerConfig;
use rmcp::ErrorData;
use tokio::sync::watch;

use pi_extension_api::{
    ExtensionContext, ExtensionRegistry, ExtensionUIContext, RuntimeHandle, ToolInvocation,
};

/// 基于扩展注册表派发的 MCP server 处理器。
///
/// 通过 `Arc<ExtensionRegistry>` 共享工具元数据与 `handle_tool_call` 派发，
/// 因此暴露的工具与 agent 会话内可用的（扩展）工具保持同源。
pub struct McpServerHandler {
    registry: Arc<ExtensionRegistry>,
    ctx: Arc<ExtensionContext>,
}

impl McpServerHandler {
    /// 构造处理器。传入一个已注册好扩展（行情/交易/MCP 客户端等）的注册表。
    #[must_use]
    pub fn new(registry: Arc<ExtensionRegistry>) -> Self {
        let ctx = Arc::new(synthetic_context());
        Self { registry, ctx }
    }

    /// 由注册表中的工具构建 rmcp `Tool` 描述列表。
    fn build_tool_descriptors(&self) -> Vec<Tool> {
        self.registry
            .tools()
            .iter()
            .map(|rt| {
                let schema = rt
                    .definition
                    .parameters
                    .as_ref()
                    .and_then(|p| p.as_object().cloned())
                    .unwrap_or_default();
                Tool::new(
                    rt.definition.name.clone(),
                    rt.definition.description.clone(),
                    Arc::new(schema),
                )
            })
            .collect()
    }

    /// 按名调用工具，经扩展派发并翻译结果为 rmcp 格式。
    async fn call_inherent(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<CallToolResult, ErrorData> {
        let output = self
            .registry
            .hook_runner()
            .dispatch_tool_call(name, arguments, &self.ctx, &ToolInvocation::noop())
            .await
            .ok_or_else(|| {
                ErrorData::new(
                    ErrorCode::METHOD_NOT_FOUND,
                    format!("unknown tool: {name}"),
                    None,
                )
            })?;

        let contents: Vec<ContentBlock> = output
            .content
            .into_iter()
            .map(|v| match v.get("text").and_then(|t| t.as_str()) {
                Some(t) => ContentBlock::text(t.to_string()),
                None => ContentBlock::text(v.to_string()),
            })
            .collect();

        if output.is_error {
            Ok(CallToolResult::error(contents))
        } else {
            Ok(CallToolResult::success(contents))
        }
    }
}

impl ServerHandler for McpServerHandler {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "openatrading".to_string(),
                env!("CARGO_PKG_VERSION").to_string(),
            ))
            .with_instructions("OpenATrading market data & trading tools")
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(
            self.build_tool_descriptors(),
        ))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<rmcp::model::CallToolResponse, ErrorData> {
        let args = request.arguments.map_or_else(
            || serde_json::Value::Object(serde_json::Map::new()),
            serde_json::Value::Object,
        );
        self.call_inherent(&request.name, args)
            .await
            .map(Into::into)
    }
}

/// 构造一个最小化的 `ExtensionContext`，仅供 server 端 `handle_tool_call` 派发使用。
/// 行情/交易扩展的 `handle_tool_call` 不读取该上下文。
fn synthetic_context() -> ExtensionContext {
    let mut ui = ExtensionUIContext::noop();
    ui.notify = Arc::new(|msg, _level| eprintln!("[mcp-server] {msg}"));
    ExtensionContext::new("mcp-server".to_string(), false, ui, RuntimeHandle::noop())
}

// ============================================================================
// Transports
// ============================================================================

/// 通过 stdio 暴露 MCP server。
///
/// # Errors
/// 启动或服务失败时返回错误。
pub async fn serve_stdio(
    registry: Arc<ExtensionRegistry>,
    mut shutdown: watch::Receiver<bool>,
) -> anyhow::Result<()> {
    use rmcp::ServiceExt;
    let server = McpServerHandler::new(registry);
    let (stdin, stdout) = rmcp::transport::io::stdio();
    let service = server
        .serve((stdin, stdout))
        .await
        .map_err(|e| anyhow::anyhow!("mcp stdio serve: {e}"))?;

    tokio::select! {
        res = service.waiting() => {
            res.map_err(|e| anyhow::anyhow!("mcp stdio waiting: {e}"))?;
        }
        _ = shutdown.changed() => {
            tracing::info!("[mcp] stdio shutdown signal received");
        }
    }
    Ok(())
}

/// 通过 HTTP（`/mcp`）暴露 MCP server。
///
/// # Errors
/// 绑定或服务失败时返回错误。
pub async fn serve_http(
    registry: Arc<ExtensionRegistry>,
    bind: SocketAddr,
    mut shutdown: watch::Receiver<bool>,
) -> anyhow::Result<()> {
    use rmcp::transport::streamable_http_server::{
        session::local::LocalSessionManager, StreamableHttpService,
    };

    let service_factory = {
        let registry = Arc::clone(&registry);
        move || -> Result<McpServerHandler, std::io::Error> {
            Ok(McpServerHandler::new(Arc::clone(&registry)))
        }
    };

    let service = StreamableHttpService::new(
        service_factory,
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig::default(),
    );

    let router = axum::Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .map_err(|e| anyhow::anyhow!("bind {bind}: {e}"))?;

    tracing::info!("[mcp] http listening on http://{bind}/mcp");

    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            let _ = shutdown.changed().await;
            tracing::info!("[mcp] http shutdown signal received");
        })
        .await
        .map_err(|e| anyhow::anyhow!("mcp http serve: {e}"))?;
    Ok(())
}
