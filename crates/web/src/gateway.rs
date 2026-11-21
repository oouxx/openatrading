use anyhow::Result;
use std::net::SocketAddr;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use openatrading_core::context::AppContext;
use openatrading_core::memory::sqlite::MemoryStore;
use openatrading_core::trade::TradeStore;

use crate::cache;
use crate::market_stream;
use crate::{self as web, AppState, RoleEntry};

pub struct Gateway {
    pub memory: Arc<MemoryStore>,
    pub trade: Arc<TradeStore>,
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
        let trade = Arc::new(TradeStore::open(db_path)?);
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
            in_flight: cache::InFlightRequests::new(),
            market_hub: market_stream::MarketStreamHub::new(),
            forward_tasks: std::sync::Mutex::new(std::collections::HashMap::new()),
            automation: config.automation,
        });

        // Seed config roles first, then discover disk roles (config wins).
        //
        // 这里**不能**擅自改 `readonly`：`config.roles` 是调用方合并后的列表，
        // 里面既有 config.toml 声明的角色、也有磁盘角色。过去在这里统一置
        // `readonly = true`，导致 gateway 一重启所有角色都变成不可删/不可改，
        // 磁盘角色也被误伤。readonly 的语义由构建列表的一方决定
        // （见 `build_roles`：config 声明的为只读，磁盘角色可编辑）。
        {
            let mut roles = state.roles.write().await;
            let disk_roles = openatrading_core::roles::list_roles_from_disk();
            for role in config.roles {
                if !roles.iter().any(|r| r.name == role.name) {
                    roles.push(role);
                }
            }
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
