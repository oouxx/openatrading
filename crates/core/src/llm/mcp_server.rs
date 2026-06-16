use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use openatrading_agent::types::DynTool;
use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, ErrorCode, Implementation, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer};
use tokio::sync::watch;

use super::tools;

/// 收集所有要对外暴露的行情类 DynTool。
pub fn build_market_tools() -> Vec<Arc<DynTool>> {
    vec![
        tools::get_limit_up(),
        tools::get_limit_down(),
        tools::get_super_stock(),
        tools::get_market_overview(),
        tools::get_yesterday_limit_up(),
        tools::get_dapan_abnormal(),
        tools::get_realtime_quote(),
        tools::get_plates_by_stock(),
        tools::get_market_trend(),
        tools::get_plate_abnormal(),
        tools::get_kline(),
        tools::get_telegraph(),
        tools::get_telegraph_allday(),
        tools::get_field_items(),
        tools::get_lhb_detail(),
        tools::get_lhb_combined(),
        tools::get_lhb_active_yyb(),
        tools::get_all_day_events(),
    ]
}

/// 持有工具集合，用作 rmcp ServerHandler 状态。
#[derive(Clone)]
pub struct MarketToolsServer {
    tools: Arc<Vec<Arc<DynTool>>>,
}

impl MarketToolsServer {
    pub fn new(tools: Vec<Arc<DynTool>>) -> Self {
        Self {
            tools: Arc::new(tools),
        }
    }

    pub fn tools(&self) -> &[Arc<DynTool>] {
        &self.tools
    }

    /// 将内部 DynTool 转换为 rmcp Tool 描述列表。
    #[doc(hidden)]
    pub fn build_tool_descriptors(&self) -> Vec<Tool> {
        self.tools
            .iter()
            .map(|t| {
                let schema_obj = t.parameters_schema.as_object().cloned().unwrap_or_default();
                Tool::new(t.name.clone(), t.description.clone(), Arc::new(schema_obj))
            })
            .collect()
    }

    /// 路由工具调用到 DynTool.execute 并转换结果为 rmcp 格式。
    pub async fn call_tool_inherent(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<CallToolResult, ErrorData> {
        let tool = self.tools.iter().find(|t| t.name == name).ok_or_else(|| {
            ErrorData::new(
                ErrorCode::METHOD_NOT_FOUND,
                format!("unknown tool: {name}"),
                None,
            )
        })?;

        let result = (tool.execute)("mcp".into(), arguments, None, None)
            .await
            .map_err(|e| ErrorData::internal_error(format!("{e}"), None))?;

        let contents: Vec<Content> = result
            .content
            .into_iter()
            .map(|cb| match cb {
                openatrading_agent::pi_ai_types::ContentBlock::Text { text, .. } => {
                    Content::text(text)
                },
                openatrading_agent::pi_ai_types::ContentBlock::Image {
                    data, mime_type, ..
                } => Content::image(data, mime_type),
                other => Content::text(format!("{other:?}")),
            })
            .collect();

        let mut call_result = CallToolResult::default();
        call_result.content = contents;
        Ok(call_result)
    }
}

impl ServerHandler for MarketToolsServer {
    fn get_info(&self) -> ServerInfo {
        let mut impl_info = Implementation::default();
        impl_info.name = "openatrading-market".into();
        impl_info.version = env!("CARGO_PKG_VERSION").into();
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(impl_info)
            .with_instructions("China A-share market data tools")
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
    ) -> Result<CallToolResult, ErrorData> {
        let args = request
            .arguments
            .map(serde_json::Value::Object)
            .unwrap_or(serde_json::Value::Object(Default::default()));
        self.call_tool_inherent(&request.name, args).await
    }
}

pub async fn serve_stdio(
    tools: Vec<Arc<DynTool>>,
    mut shutdown: watch::Receiver<bool>,
) -> Result<()> {
    let server = MarketToolsServer::new(tools);
    let (stdin, stdout) = rmcp::transport::io::stdio();
    let service = rmcp::serve_server(server, (stdin, stdout))
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

pub async fn serve_http(
    tools: Vec<Arc<DynTool>>,
    bind: SocketAddr,
    mut shutdown: watch::Receiver<bool>,
) -> Result<()> {
    use rmcp::transport::streamable_http_server::{
        session::local::LocalSessionManager, tower::StreamableHttpService,
    };

    let service_factory = {
        let tools = Arc::new(tools);
        move || -> Result<MarketToolsServer, std::io::Error> {
            Ok(MarketToolsServer::new((*tools).clone()))
        }
    };

    let service = StreamableHttpService::new(
        service_factory,
        Arc::new(LocalSessionManager::default()),
        Default::default(),
    );

    let router = axum::Router::new().route_service("/mcp", service);
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
