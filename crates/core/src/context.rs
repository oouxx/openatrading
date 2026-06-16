use std::sync::Arc;

use crate::api::Client as ApiClient;
use crate::llm::{build_tools, DynTool, LlmClient, McpServerConfig, ToolConfig};
use crate::memory::sqlite::MemoryStore;
use crate::social::NotificationSender;
use crate::trade::sqlite::TradeManager;

#[derive(Clone)]
pub struct AppContext {
    pub llm: LlmClient,
    pub api: ApiClient,
    pub tools: Vec<Arc<DynTool>>,
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
        let tools = build_tools(&tool_cfg).await.all_tools().to_vec();
        Self {
            llm,
            api,
            tools,
            memory,
            trade,
            social,
        }
    }
}
