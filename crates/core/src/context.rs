use std::sync::Arc;

use arc_swap::ArcSwap;

use crate::api::Client as ApiClient;
use crate::llm::{
    create_factory, discover_mcp, DynTool, FactoryOptions, McpServerConfig, SessionPool,
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

        let discovered = discover_mcp(mcp_servers).await;
        let mcp_names: Vec<String> = discovered
            .iter()
            .flat_map(|s| s.specs.iter().map(|t| t.name.clone()))
            .collect();
        let factory = create_factory(FactoryOptions {
            agent_dir,
            model: None,
            tools: None,
            custom_tools: None,
            trading: Some(trading),
            mcp: Some(discovered),
        });
        let pool = Arc::new(SessionPool::new(factory));
        let mut registry = ToolRegistry::new();
        let ext_registry = openatrading_extension::create_registry(
            openatrading_extension::RegistryOptions::market_only(),
        );
        registry.set_extension_tools(&ext_registry);
        registry.set_mcp_names(mcp_names);
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

    /// Get all extension tools.
    pub fn all_tools(&self) -> Vec<Arc<DynTool>> {
        self.tool_registry.load().all_tools()
    }

    /// Get discovered MCP tool names.
    pub fn mcp_tools(&self) -> Vec<String> {
        self.tool_registry.load().mcp_tools()
    }
}
