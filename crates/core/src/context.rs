use std::sync::Arc;

use arc_swap::ArcSwap;

use crate::api::Client as ApiClient;
use crate::llm::{build_tools, AgentRuntime, DynTool, McpServerConfig, ToolConfig, ToolRegistry};
use crate::memory::sqlite::MemoryStore;
use crate::social::NotificationSender;
use crate::trade::sqlite::TradeManager;

#[derive(Clone)]
pub struct AppContext {
    /// Unified runtime for all LLM operations (streaming agent sessions).
    /// Arc'd because AgentRuntime is shared across concurrent sessions
    pub runtime: Arc<AgentRuntime>,
    pub api: ApiClient,
    pub tool_registry: Arc<ArcSwap<ToolRegistry>>,
    pub memory: Arc<MemoryStore>,
    pub trade: Arc<TradeManager>,
    pub social: Arc<dyn NotificationSender>,
    /// Directory where session JSONL files are stored.
    pub session_dir: String,
}

impl AppContext {
    pub async fn build(
        _provider: Option<&str>,
        _model: Option<&str>,
        mcp_servers: &[McpServerConfig],
        memory: Arc<MemoryStore>,
        trade: Arc<TradeManager>,
        social: Arc<dyn NotificationSender>,
    ) -> Self {
        let runtime = Arc::new(AgentRuntime::new());
        let api = ApiClient::new();
        let tool_cfg = ToolConfig {
            mcp_servers: mcp_servers.to_vec(),
        };
        let mut registry = build_tools(&tool_cfg).await;
        let ext_registry = openatrading_extension::create_registry();
        registry.set_extension_tools(&ext_registry);
        let tool_registry = Arc::new(ArcSwap::from_pointee(registry));
        let cwd = std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let session_dir = format!("{cwd}/.pi-rs/agent/sessions");
        Self {
            runtime,
            api,
            tool_registry,
            memory,
            trade,
            social,
            session_dir,
        }
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
