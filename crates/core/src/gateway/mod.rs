use anyhow::Result;
use std::path::Path;
use std::sync::Arc;

use crate::api::Client as ApiClient;
use crate::cron::{self, CronConfig};
use crate::llm::openrouter::LlmClient;
use crate::memory::sqlite::SqliteMemoryStore;
use crate::social::NotificationSender;
use crate::trade::sqlite::SqliteTradeManager;
use crate::web::{self, AppState};

pub struct Gateway {
    pub memory: Arc<SqliteMemoryStore>,
    pub trade: Arc<SqliteTradeManager>,
}

pub struct GatewayConfig {
    pub cron: Option<(CronConfig, Arc<dyn NotificationSender>)>,
    pub web_port: Option<u16>,
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
            let scheduler = cron::start_scheduler(
                &cron_config,
                self.memory.clone(),
                self.trade.clone(),
                social,
            )
            .await?;
            handles.push(tokio::spawn(async move {
                let _ = scheduler.start().await;
            }));
            eprintln!("[gateway] cron scheduler started");
        }

        // ── Web ───────────────────────────────
        if let Some(port) = config.web_port {
            let state = Arc::new(AppState {
                llm: LlmClient::from_env(),
                api: ApiClient::new(),
                memory: self.memory.clone(),
                trade: self.trade.clone(),
            });

            let router = web::build_router(state);
            let addr = format!("0.0.0.0:{}", port);
            let listener = tokio::net::TcpListener::bind(&addr).await?;
            println!("🌐 Web API: http://0.0.0.0:{}", port);

            handles.push(tokio::spawn(async move {
                axum::serve(listener, router).await.unwrap();
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
