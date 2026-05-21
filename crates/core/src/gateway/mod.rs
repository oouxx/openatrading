use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::api::Client as ApiClient;
use crate::cron::{self, CronConfig};
use crate::llm::openrouter::LlmClient;
use crate::memory::sqlite::SqliteMemoryStore;
use crate::social::telegram_bot::{TelegramBot, TelegramBotConfig};
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
    pub roles: Vec<RoleEntry>,
    pub strategies_dir: PathBuf,
    pub telegram_bot: Option<TelegramBotConfig>,
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
        let mut handles = Vec::new();

        let roles = config.roles;

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
                LlmClient::from_env(),
                ApiClient::new(),
                roles.clone(),
            )
            .await?;
        } else {
            eprintln!("[gateway] cron: disabled");
        }

        // ── Web ───────────────────────────────
        if let Some(port) = config.web_port {
            let state = Arc::new(AppState {
                llm: LlmClient::from_env(),
                api: ApiClient::new(),
                memory: self.memory.clone(),
                trade: self.trade.clone(),
                roles: Arc::new(tokio::sync::RwLock::new(roles.clone())),
                strategies_dir: config.strategies_dir,
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

        // ── Telegram Bot ──────────────────────
        if let Some(bot_config) = config.telegram_bot {
            let bot = TelegramBot::new(bot_config, roles, self.memory.clone(), self.trade.clone());
            handles.push(tokio::spawn(async move {
                if let Err(e) = bot.start_polling().await {
                    eprintln!("[gateway] telegram bot error: {e}");
                }
            }));
            eprintln!("[gateway] telegram bot: enabled");
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