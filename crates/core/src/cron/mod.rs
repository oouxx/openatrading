use anyhow::Result;
use chrono::NaiveDateTime;
use chrono_tz::Asia::Shanghai;
use serde::Deserialize;
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobScheduler};

use crate::api::Client as ApiClient;
use crate::context::AppContext;
use crate::social::NotificationSender;

#[derive(Debug, Clone, Deserialize)]
pub struct CronConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_collection_schedule")]
    pub collection_schedule: String,
    #[serde(default = "default_news_schedule")]
    pub news_schedule: String,
}

const fn default_true() -> bool {
    true
}
fn default_collection_schedule() -> String {
    "0 0 18 * * 1-5".into()
}
fn default_news_schedule() -> String {
    "0 */5 9-15 * * 1-5".into()
}

#[derive(Debug, Clone)]
pub struct CollectSummary {
    pub timestamp: NaiveDateTime,
    pub sources: Vec<String>,
    pub snapshot_count: u32,
}

macro_rules! add_job {
    ($scheduler:expr, $schedule:expr, $name:expr, $task:expr) => {
        match Job::new_async_tz($schedule.as_str(), Shanghai, $task) {
            Ok(j) => {
                $scheduler.add(j).await?;
            }
            Err(e) => {
                crate::log::error!("[cron] {}: invalid schedule '{}': {e}", $name, $schedule);
            }
        }
    };
}

pub async fn start_scheduler(ctx: Arc<AppContext>, config: CronConfig) -> Result<()> {
    if !config.enabled {
        return Ok(());
    }

    let scheduler = JobScheduler::new().await?;

    // ── Collection ──
    {
        let ctx = ctx.clone();
        add_job!(
            scheduler,
            config.collection_schedule,
            "collection",
            move |_uuid, _lock| {
                let ctx = ctx.clone();
                Box::pin(async move {
                    if let Err(e) = run_collection(&ctx).await {
                        crate::log::error!("[cron] collection error: {e}");
                    }
                })
            }
        );
    }

    // ── News push (trading hours) ──
    {
        let ctx = ctx.clone();
        add_job!(
            scheduler,
            config.news_schedule,
            "news_push",
            move |_uuid, _lock| {
                let ctx = ctx.clone();
                Box::pin(async move {
                    if let Err(e) = run_news_push(&ctx.api, &*ctx.social).await {
                        crate::log::error!("[cron] news_push error: {e}");
                    }
                })
            }
        );
    }

    // ── News push (off-hours, 24/7) ──
    {
        let ctx = ctx.clone();
        let schedule = String::from("0 */30 0-8,16-23 * * *");
        add_job!(
            scheduler,
            schedule,
            "news_push_offhours",
            move |_uuid, _lock| {
                let ctx = ctx.clone();
                Box::pin(async move {
                    if let Err(e) = run_news_push(&ctx.api, &*ctx.social).await {
                        crate::log::error!("[cron] news_push error: {e}");
                    }
                })
            }
        );
    }

    scheduler.start().await?;
    crate::log::info!("[cron] scheduler started (tz=Asia/Shanghai)");

    tokio::signal::ctrl_c().await?;
    crate::log::info!("[cron] shutting down...");

    Ok(())
}

async fn run_collection(ctx: &AppContext) -> Result<()> {
    let mut sources = Vec::new();
    let mut snapshot_count = 0u32;

    match ctx.api.xuangubao.fetch_market_overview().await {
        Ok(overview) => {
            let json = serde_json::json!({
                "rise_count": overview.rise_count,
                "fall_count": overview.fall_count,
                "limit_up_count": overview.limit_up_count,
                "limit_down_count": overview.limit_down_count,
                "bomb_rate": overview.bomb_rate,
                "market_temperature": overview.market_temperature,
            });
            ctx.memory
                .save_snapshot("xuangubao_overview", &json.to_string())?;
            sources.push("xuangubao_overview".into());
            snapshot_count += 1;
        }
        Err(e) => {
            crate::log::warn!("[cron] collection: fetch overview failed: {e}");
        }
    }

    match ctx.api.xuangubao.fetch_limit_up().await {
        Ok(stocks) => {
            let json = serde_json::to_string(&stocks).unwrap_or_else(|_| "[]".into());
            ctx.memory.save_snapshot("xuangubao_limit_up", &json)?;
            sources.push("xuangubao_limit_up".into());
            snapshot_count += 1;
        }
        Err(e) => {
            crate::log::warn!("[cron] collection: fetch limit_up failed: {e}");
        }
    }

    match ctx.api.xuangubao.fetch_super_stock().await {
        Ok(stocks) => {
            let json = serde_json::to_string(&stocks).unwrap_or_else(|_| "[]".into());
            ctx.memory.save_snapshot("xuangubao_super_stock", &json)?;
            sources.push("xuangubao_super_stock".into());
            snapshot_count += 1;
        }
        Err(e) => {
            crate::log::warn!("[cron] collection: fetch super_stock failed: {e}");
        }
    }

    {
        let date = crate::utils::time::beijing_now()
            .format("%Y-%m-%d")
            .to_string();
        match ctx.api.jiuyan.fetch_field_str(&date, "1").await {
            Ok(items) => {
                let json = serde_json::to_string(&items).unwrap_or_else(|_| "{}".into());
                ctx.memory.save_snapshot("jiuyan_field_items", &json)?;
                sources.push("jiuyan_field_items".into());
                snapshot_count += 1;
            }
            Err(e) => {
                crate::log::warn!("[cron] collection: fetch jiuyan field failed: {e}");
            }
        }
    }

    let summary = CollectSummary {
        timestamp: crate::utils::time::beijing_now().naive_local(),
        sources,
        snapshot_count,
    };
    let text = format!(
        "📊 采集完成\n时间: {}\n数据源: {}\n快照: {} 条",
        summary.timestamp.format("%H:%M:%S"),
        summary.sources.join(", "),
        summary.snapshot_count,
    );
    let _ = ctx.social.notify_collect(&text).await;

    Ok(())
}
static LAST_PUSHED_NEWS_TIME: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);
static NEWS_TIME_INITIALIZED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub async fn run_news_push(api: &ApiClient, social: &dyn NotificationSender) -> Result<()> {
    let items = api.cls.fetch_telegraph("", None, 50).await?;
    if items.is_empty() {
        return Ok(());
    }

    if !NEWS_TIME_INITIALIZED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        let now = chrono::Utc::now().timestamp();
        LAST_PUSHED_NEWS_TIME.store(now, std::sync::atomic::Ordering::Relaxed);
        crate::log::info!("[cron] news_push: initialized, skipping existing items");
        return Ok(());
    }

    let last_time = LAST_PUSHED_NEWS_TIME.load(std::sync::atomic::Ordering::Relaxed);

    let mut new_items: Vec<&crate::api::cls::TelegramInfo> = items
        .iter()
        .filter(|item| {
            let ts = item.ctime.unwrap_or(0);
            ts > last_time
        })
        .collect();

    if new_items.is_empty() {
        return Ok(());
    }

    new_items.sort_by_key(|item| item.ctime.unwrap_or(0));

    let max_time = new_items
        .iter()
        .map(|item| item.ctime.unwrap_or(0))
        .max()
        .unwrap_or(last_time);
    LAST_PUSHED_NEWS_TIME.store(max_time, std::sync::atomic::Ordering::Relaxed);

    let important: Vec<&crate::api::cls::TelegramInfo> = new_items
        .iter()
        .filter(|item| {
            let level = item.level.as_deref().unwrap_or("");
            level == "A" || level == "B"
        })
        .copied()
        .collect();

    if important.is_empty() {
        return Ok(());
    }

    let beijing_tz =
        chrono::FixedOffset::east_opt(8 * 3600).unwrap_or_else(|| panic!("invalid UTC+8 offset"));
    for item in &important {
        let title = item.title.as_deref().unwrap_or("");
        let brief = item.brief.as_deref().unwrap_or("");
        let content = item.content.as_deref().unwrap_or("");
        let text = if !content.is_empty() {
            content
        } else if !brief.is_empty() {
            brief
        } else {
            title
        };
        let level_tag = match item.level.as_deref().unwrap_or("") {
            "A" => "🔴",
            "B" => "⚡",
            _ => "📰",
        };
        let time_str = item
            .ctime
            .map(|t| {
                chrono::DateTime::from_timestamp(t, 0)
                    .map(|dt| dt.with_timezone(&beijing_tz).format("%H:%M").to_string())
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        let msg = format!("{level_tag} {time_str} {text}");
        if let Err(e) = social.notify_news(&msg).await {
            crate::log::warn!("[cron] news_push: send failed: {e}");
        }
    }

    crate::log::info!(
        "[cron] news_push: pushed {} important items (A/B level)",
        important.len()
    );
    Ok(())
}
