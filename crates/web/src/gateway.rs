use anyhow::Result;
use std::path::Path;
use std::sync::Arc;

use openatrading_core::context::AppContext;
use openatrading_core::memory::sqlite::MemoryStore;
use openatrading_core::trade::sqlite::TradeManager;

use crate::cache;
use crate::{self as web, AppState, RoleEntry};

pub struct Gateway {
    pub memory: Arc<MemoryStore>,
    pub trade: Arc<TradeManager>,
}

pub struct GatewayConfig {
    pub web_port: u16,
    pub roles: Vec<RoleEntry>,
    pub mcp_servers: Vec<openatrading_core::llm::McpServerConfig>,
}

impl Gateway {
    pub fn init(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let memory = Arc::new(MemoryStore::open(db_path)?);
        let trade = Arc::new(TradeManager::open(db_path)?);
        Ok(Gateway { memory, trade })
    }

    pub async fn start(self, config: GatewayConfig, ctx: AppContext) -> Result<()> {
        let state = Arc::new(AppState {
            ctx,
            roles: Arc::new(tokio::sync::RwLock::new(Vec::new())),
            mcp_servers: config.mcp_servers.clone(),
            cache: cache::new_cache(),
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
        let addr = format!("0.0.0.0:{}", config.web_port);
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        println!("🌐 Web API: http://0.0.0.0:{}", config.web_port);

        axum::serve(listener, router).await?;
        Ok(())
    }
}
