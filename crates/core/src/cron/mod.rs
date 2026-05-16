use anyhow::Result;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobScheduler};

use crate::memory::MemoryStore;
use crate::social::NotificationSender;
use crate::trade::TradeManager;

#[derive(Debug, Clone, Deserialize)]
pub struct CronConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_collection_schedule")]
    pub collection_schedule: String,
    #[serde(default = "default_decision_schedule")]
    pub decision_schedule: String,
}

fn default_true() -> bool {
    true
}
fn default_collection_schedule() -> String {
    "*/30 9-15 * * 1-5".to_string()
}
fn default_decision_schedule() -> String {
    "0 0 * * 1-5".to_string()
}

#[derive(Debug, Clone)]
pub struct CollectSummary {
    pub timestamp: NaiveDateTime,
    pub sources: Vec<String>,
    pub snapshot_count: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct TradeSuggestion {
    pub symbol: String,
    pub action: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct Decision {
    pub role_name: String,
    pub summary: String,
    pub trade_suggestions: Vec<TradeSuggestion>,
}

/// 启动 cron scheduler，注册采集和决策两个 Job
pub async fn start_scheduler(
    config: &CronConfig,
    memory: Arc<dyn MemoryStore>,
    trade: Arc<dyn TradeManager>,
    social: Arc<dyn NotificationSender>,
) -> Result<JobScheduler> {
    let scheduler = JobScheduler::new().await?;

    if !config.enabled {
        return Ok(scheduler);
    }

    // ── 采集 Job ──────────────────────────────────────
    let mem_c = memory.clone();
    let soc_c = social.clone();
    let schedule = config.collection_schedule.clone();
    scheduler
        .add(Job::new_async(schedule.as_str(), move |_uuid, _lock| {
            let mem = mem_c.clone();
            let soc = soc_c.clone();
            Box::pin(async move {
                if let Err(e) = mem.save_snapshot("cron_tick", "{}") {
                    eprintln!("[cron] save_snapshot error: {e}");
                }
                let summary = CollectSummary {
                    timestamp: chrono::Local::now().naive_local(),
                    sources: vec!["cron_tick".into()],
                    snapshot_count: 1,
                };
                let _ = soc.notify_collect(&summary).await;
            })
        })?)
        .await?;

    // ── 决策 Job ───────────────────────────────────────
    let mem_d = memory.clone();
    let trd = trade.clone();
    let soc_d = social.clone();
    let schedule_d = config.decision_schedule.clone();
    scheduler
        .add(Job::new_async(schedule_d.as_str(), move |_uuid, _lock| {
            let mem = mem_d.clone();
            let trd = trd.clone();
            let soc = soc_d.clone();
            Box::pin(async move {
                if let Err(e) = run_decision(&*mem, &*trd, &*soc).await {
                    eprintln!("[cron] decision error: {e}");
                }
            })
        })?)
        .await?;

    scheduler.start().await?;
    Ok(scheduler)
}

async fn run_decision(
    memory: &dyn MemoryStore,
    trade: &dyn TradeManager,
    social: &dyn NotificationSender,
) -> Result<()> {
    let roles = ["zarili", "trader", "quant", "risk_officer"];
    for role_name in &roles {
        if let Ok(context) = trade.format_positions(role_name) {
            if context != "暂无持仓" {
                let historical = memory.get_historical_context(5).unwrap_or_default();
                let decision = Decision {
                    role_name: role_name.to_string(),
                    summary: format!(
                        "角色【{}】\n持仓:\n{}\n近5日数据: {}条",
                        role_name,
                        context,
                        historical.lines().count()
                    ),
                    trade_suggestions: vec![],
                };
                let _ = social.notify_decision(&decision).await;
            }
        }
    }
    Ok(())
}
