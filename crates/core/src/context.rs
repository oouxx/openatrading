use std::sync::Arc;

use arc_swap::ArcSwap;

use crate::api::Client as ApiClient;
use crate::llm::{
    build_tools, create_factory, DynTool, FactoryOptions, McpServerConfig, SessionPool, ToolConfig,
    ToolRegistry,
};
use crate::memory::sqlite::MemoryStore;
use crate::social::NotificationSender;
use crate::trade::sqlite::TradeManager;
use crate::trade::PaperExecutor;
use openatrading_extension::{MemoryReader, PositionReader, TradingContext};

#[derive(Clone)]
pub struct AppContext {
    /// Session pool managing all LLM agent sessions.
    pub pool: Arc<SessionPool>,
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
        mcp_servers: &[McpServerConfig],
        memory: Arc<MemoryStore>,
        trade: Arc<TradeManager>,
        social: Arc<dyn NotificationSender>,
    ) -> Self {
        let agent_dir = pi_coding_agent::config::get_agent_dir()
            .to_string_lossy()
            .to_string();

        let api = ApiClient::new();

        // Paper-account trading state shared by every session created from
        // this factory. A single `paper` account is the only decision sink;
        // role CRUD is retained for future strategy↔account isolation.
        let trading = TradingContext {
            role: "paper".to_string(),
            positions: trade.clone() as Arc<dyn PositionReader>,
            memory: memory.clone() as Arc<dyn MemoryReader>,
            sink: Arc::new(PaperExecutor::new(trade.clone(), api.clone())),
        };

        let factory = create_factory(FactoryOptions {
            agent_dir,
            model: None,
            tools: None,
            custom_tools: None,
            trading: Some(trading),
        });
        let pool = Arc::new(SessionPool::new(factory));
        let tool_cfg = ToolConfig {
            mcp_servers: mcp_servers.to_vec(),
        };
        let mut registry = build_tools(&tool_cfg).await;
        let ext_registry = openatrading_extension::create_registry(None);
        registry.set_extension_tools(&ext_registry);
        let tool_registry = Arc::new(ArcSwap::from_pointee(registry));
        let session_dir = pi_coding_agent::config::get_sessions_dir()
            .to_string_lossy()
            .to_string();
        Self {
            pool,
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
