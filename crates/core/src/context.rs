use std::sync::Arc;

use arc_swap::ArcSwap;

use crate::api::Client as ApiClient;
use crate::llm::{create_factory, DynTool, FactoryOptions, SessionPool, ToolRegistry};
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

/// [`AppContext::build`] 的参数束(参数 >3 个时按规范合并为 struct)。
pub struct ContextParams {
    pub memory: Arc<MemoryStore>,
    pub trade: Arc<TradeManager>,
    pub social: Arc<dyn NotificationSender>,
    /// 自动化调度中枢;`Some` 时为每个会话注册 create/list automation 工具。
    pub automations: Option<Arc<crate::automation::AutomationHub>>,
}

impl AppContext {
    pub async fn build(params: ContextParams) -> Self {
        let ContextParams {
            memory,
            trade,
            social,
            automations,
        } = params;
        Self::build_from(memory, trade, social, automations).await
    }

    async fn build_from(
        memory: Arc<MemoryStore>,
        trade: Arc<TradeManager>,
        social: Arc<dyn NotificationSender>,
        automations: Option<Arc<crate::automation::AutomationHub>>,
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

        // 自动化任务工具:创建/查看定时任务(对话内建任务)。
        let automation_tools = automations
            .as_ref()
            .map(crate::automation::automation_tools);

        let factory = create_factory(FactoryOptions {
            agent_dir,
            model: None,
            tools: None,
            custom_tools: automation_tools,
            trading: Some(trading),
        });
        let pool = Arc::new(SessionPool::new(factory));
        let mut registry = ToolRegistry::new();
        let ext_registry = openatrading_extension::create_registry(
            openatrading_extension::RegistryOptions::market_only(),
        );
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

    /// Get all extension tools.
    pub fn all_tools(&self) -> Vec<Arc<DynTool>> {
        self.tool_registry.load().all_tools()
    }
}
