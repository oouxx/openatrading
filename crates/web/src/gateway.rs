use anyhow::Result;
use std::path::Path;
use std::sync::Arc;

use trading_agent_core::api::Client as ApiClient;
use trading_agent_core::llm::LlmClient;
use trading_agent_core::memory::sqlite::SqliteMemoryStore;
use trading_agent_core::trade::sqlite::SqliteTradeManager;

use crate::cache;
use crate::{self as web, AppState, RoleEntry};

pub struct Gateway {
    pub memory: Arc<SqliteMemoryStore>,
    pub trade: Arc<SqliteTradeManager>,
}

pub struct GatewayConfig {
    pub web_port: u16,
    pub roles: Vec<RoleEntry>,
    pub strategies_dir: String,
}

impl Gateway {
    pub fn init(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let memory = Arc::new(SqliteMemoryStore::open(db_path)?);
        let trade = Arc::new(SqliteTradeManager::open(db_path)?);
        Ok(Gateway { memory, trade })
    }

    pub async fn start(self, config: GatewayConfig) -> Result<()> {
        let state = Arc::new(AppState {
            llm: LlmClient::from_env(),
            api: ApiClient::new(),
            memory: self.memory.clone(),
            trade: self.trade.clone(),
            roles: Arc::new(tokio::sync::RwLock::new(config.roles.clone())),
            strategies_dir: config.strategies_dir.into(),
            cache: cache::new_cache(),
        });

        let router = web::build_router(state);
        let addr = format!("0.0.0.0:{}", config.web_port);
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        println!("🌐 Web API: http://0.0.0.0:{}", config.web_port);

        axum::serve(listener, router).await?;
        Ok(())
    }
}
