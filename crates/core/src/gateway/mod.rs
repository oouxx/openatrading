use anyhow::Result;
use std::path::Path;
use std::sync::Arc;

use crate::cron::{self, CronConfig};
use crate::memory::sqlite::SqliteMemoryStore;
use crate::social::NotificationSender;
use crate::trade::sqlite::SqliteTradeManager;

pub struct Gateway {
    pub memory: Arc<SqliteMemoryStore>,
    pub trade: Arc<SqliteTradeManager>,
}

impl Gateway {
    /// 初始化 Gateway，打开 SQLite 数据库
    pub fn init(db_path: &Path) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let memory = Arc::new(SqliteMemoryStore::open(db_path)?);
        let trade = Arc::new(SqliteTradeManager::open(db_path)?);
        Ok(Gateway { memory, trade })
    }

    /// 启动 cron scheduler，阻塞至 Ctrl+C
    pub async fn start(
        &self,
        cron_config: &CronConfig,
        social: Arc<dyn NotificationSender>,
    ) -> Result<()> {
        let _scheduler =
            cron::start_scheduler(cron_config, self.memory.clone(), self.trade.clone(), social)
                .await?;

        eprintln!("[gateway] 已启动 (Ctrl+C 停止)");

        // Block until Ctrl+C
        tokio::signal::ctrl_c().await?;
        eprintln!("\n[gateway] 正在停止...");
        Ok(())
    }
}
