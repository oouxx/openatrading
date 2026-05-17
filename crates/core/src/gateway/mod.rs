use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::api::Client as ApiClient;
use crate::cron::{self, CronConfig};
use crate::llm::openrouter::LlmClient;
use crate::memory::sqlite::SqliteMemoryStore;
use crate::social::NotificationSender;
use crate::trade::sqlite::SqliteTradeManager;
use crate::web::{self, AppState, RoleEntry};

pub struct Gateway {
    pub memory: Arc<SqliteMemoryStore>,
    pub trade: Arc<SqliteTradeManager>,
}

pub struct GatewayConfig {
    pub cron: Option<(CronConfig, Arc<dyn NotificationSender>)>,
    pub web_port: Option<u16>,
    pub strategies_dir: PathBuf,
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

    /// Start configured services, block until Ctrl+C
    pub async fn start(self, config: GatewayConfig) -> Result<()> {
        let mut handles = Vec::new();

        // ── Cron ──────────────────────────────
        if let Some((cron_config, social)) = config.cron {
            eprintln!(
                "[gateway] cron: enabled, collection={}, decision={}",
                cron_config.collection_schedule, cron_config.decision_schedule
            );
            let _scheduler = cron::start_scheduler(
                &cron_config,
                self.memory.clone(),
                self.trade.clone(),
                social,
            )
            .await?;
        } else {
            eprintln!("[gateway] cron: disabled");
        }

        // ── Web ───────────────────────────────
        if let Some(port) = config.web_port {
            // Load built-in + user roles from filesystem
            let mut roles: Vec<RoleEntry> = crate::roles::BUILTIN_STRATEGIES
                .iter()
                .map(|(name, display_name, strategy)| RoleEntry {
                    name: name.to_string(),
                    display_name: display_name.to_string(),
                    strategy: strategy.to_string(),
                })
                .collect();

            for (name, display_name, strategy) in
                crate::roles::list_user_roles(&config.strategies_dir)
            {
                // Skip if already exists (user override of built-in)
                if !roles.iter().any(|r| r.name == name) {
                    roles.push(RoleEntry {
                        name,
                        display_name,
                        strategy,
                    });
                }
            }

            let state = Arc::new(AppState {
                llm: LlmClient::from_env(),
                api: ApiClient::new(),
                memory: self.memory.clone(),
                trade: self.trade.clone(),
                roles,
                strategies_dir: config.strategies_dir.clone(),
            });

            let router = web::build_router(state);
            let addr = format!("0.0.0.0:{}", port);
            let listener = tokio::net::TcpListener::bind(&addr).await?;
            println!("🌐 Web API: http://0.0.0.0:{}", port);

            handles.push(tokio::spawn(async move {
                if let Err(e) = axum::serve(listener, router).await {
                    eprintln!("[gateway] web server error: {e}");
                }
            }));
        }

        if handles.is_empty() {
            eprintln!("[gateway] nothing to run (no cron, no web)");
            return Ok(());
        }

        eprintln!("[gateway] running (Ctrl+C to stop)");

        // Wait for Ctrl+C
        tokio::signal::ctrl_c().await?;
        eprintln!("\n[gateway] shutting down...");

        for h in handles {
            h.abort();
        }

        Ok(())
    }
}
