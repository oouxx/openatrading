use std::sync::Arc;

use crate::api::Client as ApiClient;
use crate::llm::{build_tools, DynTool, LlmClient, McpServerConfig, ToolConfig, ToolRegistry};
use crate::memory::sqlite::MemoryStore;
use crate::social::NotificationSender;
use crate::trade::sqlite::TradeManager;

#[derive(Clone)]
pub struct AppContext {
    pub llm: LlmClient,
    pub api: ApiClient,
    pub tool_registry: Arc<ToolRegistry>,
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
        let api = ApiClient::new();
        let tool_cfg = ToolConfig {
            mcp_servers: mcp_servers.to_vec(),
        };
        let tool_registry = Arc::new(build_tools(&tool_cfg).await);
        Self {
            llm,
            api,
            tool_registry,
            memory,
            trade,
            social,
        }
    }

    /// Get all tools (market + meta + MCP + decision).
    pub fn all_tools(&self) -> Vec<Arc<DynTool>> {
        self.tool_registry.all_tools()
    }

    /// Get only MCP tools.
    pub fn mcp_tools(&self) -> &[Arc<DynTool>] {
        self.tool_registry.mcp_tools()
    }
}
