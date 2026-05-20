use anyhow::Result;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobScheduler};

use crate::api::Client as ApiClient;
use crate::llm::openrouter::LlmClient;
use crate::memory::MemoryStore;
use crate::social::NotificationSender;
use crate::trade::TradeManager;
use crate::web::RoleEntry;

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
    "0 */30 9-15 * * 1-5 *".to_string()
}
fn default_decision_schedule() -> String {
    "0 0 0 * * 1-5 *".to_string()
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

pub async fn start_scheduler(
    config: &CronConfig,
    memory: Arc<dyn MemoryStore>,
    trade: Arc<dyn TradeManager>,
    social: Arc<dyn NotificationSender>,
    llm: LlmClient,
    api: ApiClient,
    roles: Vec<RoleEntry>,
) -> Result<JobScheduler> {
    let scheduler = JobScheduler::new().await?;

    if !config.enabled {
        return Ok(scheduler);
    }

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

    let mem_d = memory.clone();
    let trd = trade.clone();
    let soc_d = social.clone();
    let schedule_d = config.decision_schedule.clone();
    scheduler
        .add(Job::new_async(schedule_d.as_str(), move |_uuid, _lock| {
            let mem = mem_d.clone();
            let trd = trd.clone();
            let soc = soc_d.clone();
            let llm = llm.clone();
            let api = api.clone();
            let roles = roles.clone();
            Box::pin(async move {
                if let Err(e) = run_decision(&*mem, &*trd, &*soc, &llm, &api, &roles).await {
                    eprintln!("[cron] decision error: {e}");
                }
            })
        })?)
        .await?;

    scheduler.start().await?;
    Ok(scheduler)
}

pub async fn run_decision(
    memory: &dyn MemoryStore,
    trade: &dyn TradeManager,
    social: &dyn NotificationSender,
    llm: &LlmClient,
    api: &ApiClient,
    roles: &[RoleEntry],
) -> Result<()> {
    let market_context = match api.xuangubao.fetch_market_overview().await {
        Ok(o) => format!(
            "## 当前市场数据\n\
             - 日期：{}\n\
             - 上涨家数：{}\n\
             - 下跌家数：{}\n\
             - 涨停家数：{}\n\
             - 跌停家数：{}\n\
             - 炸板率：{:.1}%\n\
             - 市场温度：{:.1}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M"),
            o.rise_count,
            o.fall_count,
            o.limit_up_count,
            o.limit_down_count,
            o.bomb_rate,
            o.market_temperature,
        ),
        Err(e) => {
            tracing::warn!("[cron] fetch market overview failed: {e}");
            "## 市场数据获取失败\n".to_string()
        }
    };

    let historical = memory.get_historical_context(5).unwrap_or_default();

    for role in roles {
        let positions = trade.format_positions(&role.name)?;
        let has_positions = positions != "暂无持仓";

        let mut messages = vec![
            serde_json::json!({"role": "system", "content": role.strategy}),
            serde_json::json!({"role": "user", "content": market_context.clone()}),
            serde_json::json!({"role": "assistant", "content": "已收到市场数据，请提问。"}),
        ];

        let role_context = if has_positions {
            format!(
                "## 当前持仓\n{}\n\n## 近5日历史数据\n{}",
                positions, historical
            )
        } else {
            format!("## 当前无持仓\n\n## 近5日历史数据\n{}", historical)
        };

        messages.push(serde_json::json!({
            "role": "user",
            "content": format!(
                "{}\n\n请根据以上市场数据和{}，给出今日复盘和明日操作建议。要求简洁，不超过300字。",
                role_context,
                role.display_name,
            ),
        }));

        let messages_json = serde_json::json!(messages).to_string();

        let reply = match llm.chat_with_messages(&messages_json).await {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("[cron] LLM decision failed for role '{}': {e}", role.name);
                format!("❌ LLM 调用失败: {e}")
            }
        };

        let decision = Decision {
            role_name: role.display_name.clone(),
            summary: format!("*{} 决策*\n{}", role.display_name, reply),
            trade_suggestions: vec![],
        };

        if let Err(e) = social.notify_decision(&decision).await {
            tracing::error!(
                "[cron] notify decision failed for role '{}': {e}",
                role.name
            );
        }
    }

    Ok(())
}
