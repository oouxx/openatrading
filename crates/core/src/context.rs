use std::sync::Arc;

use arc_swap::ArcSwap;

use pi_coding_agent::core::model_registry::ModelRegistry;
use pi_coding_agent::core::session_manager::SessionManager;

use crate::api::Client as ApiClient;
use crate::llm::agent_runtime::AgentRuntime;
use crate::llm::{build_tools, DynTool, LlmClient, McpServerConfig, ToolConfig, ToolRegistry};
use crate::memory::sqlite::MemoryStore;
use crate::social::NotificationSender;
use crate::trade::sqlite::TradeManager;

#[derive(Clone)]
pub struct AppContext {
    pub llm: LlmClient,
    pub api: ApiClient,
    pub tool_registry: Arc<ArcSwap<ToolRegistry>>,
    pub memory: Arc<MemoryStore>,
    pub trade: Arc<TradeManager>,
    pub social: Arc<dyn NotificationSender>,
    pub agent_runtime: Arc<AgentRuntime>,
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
        let tool_registry = Arc::new(ArcSwap::from_pointee(build_tools(&tool_cfg).await));

        let cwd = std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let session_manager =
            SessionManager::new(&cwd, &format!("{cwd}/.pi-rs/sessions"), None, false, None);
        let model_registry = ModelRegistry::new(ModelRegistry::builtin_models_list());
        let agent_runtime = Arc::new(AgentRuntime::new(session_manager, model_registry));

        Self {
            llm,
            api,
            tool_registry,
            memory,
            trade,
            social,
            agent_runtime,
        }
    }

    /// Get all tools (market + meta + MCP + decision).
    pub fn all_tools(&self) -> Vec<Arc<DynTool>> {
        self.tool_registry.load().all_tools()
    }

    /// Get only MCP tools.
    pub fn mcp_tools(&self) -> Vec<Arc<DynTool>> {
        self.tool_registry.load().mcp_tools().to_vec()
    }
}
