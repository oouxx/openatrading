use std::sync::Arc;

use arc_swap::ArcSwap;

use crate::api::Client as ApiClient;
use crate::llm::{
    build_tools, AgentRuntime, DynTool, LlmClient, McpServerConfig, ToolConfig, ToolRegistry,
};
use crate::memory::sqlite::MemoryStore;
use crate::social::NotificationSender;
use crate::trade::sqlite::TradeManager;

#[derive(Clone)]
pub struct AppContext {
    /// Unified runtime for all LLM operations (streaming agent sessions).
    pub runtime: AgentRuntime,
    /// LlmClient kept for one-shot doctor/extractor calls that don't need tools
    /// or streaming. Created on demand via [`AppContext::llm_client`].
    llm: LlmClient,
    pub api: ApiClient,
    pub tool_registry: Arc<ArcSwap<ToolRegistry>>,
    pub memory: Arc<MemoryStore>,
    pub trade: Arc<TradeManager>,
    pub social: Arc<dyn NotificationSender>,
}

impl AppContext {
    pub async fn build(
        provider: Option<&str>,
        model: Option<&str>,
        mcp_servers: &[McpServerConfig],
        memory: Arc<MemoryStore>,
        trade: Arc<TradeManager>,
        social: Arc<dyn NotificationSender>,
    ) -> Self {
        let llm = LlmClient::from_config(provider, model);
        let runtime = AgentRuntime::new();
        let api = ApiClient::new();
        let tool_cfg = ToolConfig {
            mcp_servers: mcp_servers.to_vec(),
        };
        let mut registry = build_tools(&tool_cfg).await;
        let ext_registry = openatrading_extension::create_registry();
        registry.set_extension_tools(&ext_registry);
        let tool_registry = Arc::new(ArcSwap::from_pointee(registry));

        Self {
            runtime,
            llm,
            api,
            tool_registry,
            memory,
            trade,
            social,
        }
    }

    /// Create an [`LlmClient`] on demand for one-shot doctor/extractor calls
    /// that don't need tools or streaming.
    pub fn llm_client(&self) -> LlmClient {
        self.llm.clone()
    }

    /// Get all tools (extension + MCP).
    pub fn all_tools(&self) -> Vec<Arc<DynTool>> {
        self.tool_registry.load().all_tools()
    }

    /// Get only MCP tools.
    pub fn mcp_tools(&self) -> Vec<Arc<DynTool>> {
        self.tool_registry.load().mcp_tools().to_vec()
    }
}
