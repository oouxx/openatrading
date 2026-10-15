use std::sync::Arc;

use arc_swap::ArcSwap;

use crate::api::Client as ApiClient;
use crate::llm::{create_factory, DynTool, FactoryOptions, SessionPool, ToolRegistry};
use crate::memory::sqlite::MemoryStore;
use crate::social::NotificationSender;
use crate::trade::{ApiQuoteSource, TradeService, TradeStore};

#[derive(Clone)]
pub struct AppContext {
    /// Session pool managing all LLM agent sessions.
    pub pool: Arc<SessionPool>,
    pub api: ApiClient,
    pub tool_registry: Arc<ArcSwap<ToolRegistry>>,
    pub memory: Arc<MemoryStore>,
    /// 模拟盘交易（手动记账，agent 不参与下单）。
    pub trade: Arc<TradeService>,
    pub social: Arc<dyn NotificationSender>,
    /// Directory where session JSONL files are stored.
    pub session_dir: String,
}

/// [`AppContext::build`] 的参数束(参数 >3 个时按规范合并为 struct)。
pub struct ContextParams {
    pub memory: Arc<MemoryStore>,
    /// 交易数据存储。行情来源由 `build` 用内部的 [`ApiClient`] 组装，
    /// 调用方不必关心二者如何拼装。
    pub trade_store: Arc<TradeStore>,
    pub social: Arc<dyn NotificationSender>,
    /// 自动化调度中枢;`Some` 时为每个会话注册 create/list automation 工具。
    pub automations: Option<Arc<crate::automation::AutomationHub>>,
}

impl AppContext {
    /// # Errors
    /// 任何子组件初始化失败时返回错误。
    pub async fn build(params: ContextParams) -> Self {
        let ContextParams {
            memory,
            trade_store,
            social,
            automations,
        } = params;
        Self::build_from(memory, trade_store, social, automations).await
    }

    async fn build_from(
        memory: Arc<MemoryStore>,
        trade_store: Arc<TradeStore>,
        social: Arc<dyn NotificationSender>,
        automations: Option<Arc<crate::automation::AutomationHub>>,
    ) -> Self {
        let agent_dir = pi_coding_agent::config::get_agent_dir()
            .to_string_lossy()
            .to_string();

        let api = ApiClient::new();
        let trade = Arc::new(TradeService::new(
            trade_store,
            Arc::new(ApiQuoteSource::new(api.clone())),
        ));

        // 自动化任务工具:创建/查看定时任务(对话内建任务)。
        let automation_tools = automations
            .as_ref()
            .map(crate::automation::automation_tools);

        let factory = create_factory(FactoryOptions {
            agent_dir,
            model: None,
            tools: None,
            custom_tools: automation_tools,
        });
        let pool = Arc::new(SessionPool::new(factory));
        let mut registry = ToolRegistry::new();
        let ext_registry = openatrading_extension::create_registry();
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
