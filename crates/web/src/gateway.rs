use anyhow::Result;
use std::net::SocketAddr;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use openatrading_core::context::AppContext;
use openatrading_core::memory::sqlite::MemoryStore;
use openatrading_core::trade::sqlite::TradeManager;

use crate::cache;
use crate::market_stream;
use crate::{self as web, AppState, RoleEntry};

pub struct Gateway {
    pub memory: Arc<MemoryStore>,
    pub trade: Arc<TradeManager>,
    /// 自动化任务存储(与 memory/trade 同一数据库)。
    pub automation: Arc<openatrading_core::automation::AutomationStore>,
}

pub struct GatewayConfig {
    /// Full bind address (`host:port`) for the web API, e.g. `0.0.0.0:8080`.
    pub web_bind: String,
    pub roles: Vec<RoleEntry>,
    /// 外部 MCP server 配置读写入口（读写 pi-rs 的 `mcp.json`）。
    pub mcp: openatrading_core::llm::McpConfigStore,
    /// 自动化调度中枢(由调用方 attach + start)。
    pub automation: Arc<openatrading_core::automation::AutomationHub>,
}

impl Gateway {
    /// Initialize the gateway with a database path.
    ///
    /// # Errors
    /// Returns an error if the database cannot be opened or the parent directory cannot be created.
    pub fn init(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let memory = Arc::new(MemoryStore::open(db_path)?);
        let trade = Arc::new(TradeManager::open(db_path)?);
        let automation = Arc::new(openatrading_core::automation::AutomationStore::open(
            db_path,
        )?);
        Ok(Self {
            memory,
            trade,
            automation,
        })
    }

    /// Start the web API server.
    ///
    /// # Errors
    /// Returns an error if the server cannot bind to the address or fails to serve.
    pub async fn start(self, config: GatewayConfig, ctx: AppContext) -> Result<()> {
        let state = Arc::new(AppState {
            ctx,
            roles: Arc::new(tokio::sync::RwLock::new(Vec::new())),
            mcp: config.mcp,
            cache: cache::new_cache(),
            market_hub: market_stream::MarketStreamHub::new(),
            forward_tasks: std::sync::Mutex::new(std::collections::HashMap::new()),
            automation: config.automation,
        });

        // Seed config roles first, then discover disk roles (config wins)
        {
            let mut roles = state.roles.write().await;
            let disk_roles = openatrading_core::roles::list_roles_from_disk();
            // config.toml roles have higher priority, marked readonly
            for role in config.roles {
                if !roles.iter().any(|r| r.name == role.name) {
                    let mut r = role;
                    r.readonly = true;
                    roles.push(r);
                }
            }
            // disk roles are editable
            for disk_role in disk_roles {
                if !roles.iter().any(|r| r.name == disk_role.name) {
                    roles.push(disk_role);
                }
            }
        }

        let router = web::build_router(state);
        let addr = SocketAddr::from_str(&config.web_bind)
            .map_err(|e| anyhow::anyhow!("invalid web_bind {}: {e}", config.web_bind))?;
        let listener = tokio::net::TcpListener::bind(addr).await?;
        openatrading_core::log::info!("🌐 Web API: http://{addr}");

        axum::serve(listener, router).await?;
        Ok(())
    }
}
