//! 内置定时任务(自 `cron` 模块合并):行情采集 + 财经快讯推送。
//!
//! 由 [`AutomationHub::start`] 按 `[cron]` 配置注册进同一个调度器;
//! 用户自定义自动化的增删改不影响这些固定任务。

use anyhow::Result;
use chrono::NaiveDateTime;
use chrono_tz::Asia::Shanghai;
use serde::Deserialize;
use tokio_cron_scheduler::Job;

use crate::api::Client as ApiClient;
use crate::context::AppContext;
use crate::social::NotificationSender;

use super::runner::{add_job, AutomationHub};

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

/// 把内置任务注册进调度器(只在 gateway 进程执行一次)。
///
/// # Errors
/// 调度器句柄不可用或添加 job 失败;单个表达式非法仅记日志跳过。
pub(super) async fn register_builtin(hub: &AutomationHub, config: CronConfig) -> Result<()> {
    let ctx = hub.ctx()?;
    let scheduler = hub.scheduler().await?;

    // ── Collection ──
    {
        let ctx = ctx.clone();
        add_job!(
            scheduler,
            config.collection_schedule,
            "builtin_collection",
            move |_uuid, _lock| {
                let ctx = ctx.clone();
                Box::pin(async move {
                    if let Err(e) = run_collection(&ctx).await {
                        crate::log::error!("[automation] builtin collection error: {e}");
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
            "builtin_news_push",
            move |_uuid, _lock| {
                let ctx = ctx.clone();
                Box::pin(async move {
                    if let Err(e) = run_news_push(&ctx.api, &*ctx.social).await {
                        crate::log::error!("[automation] builtin news_push error: {e}");
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
            "builtin_news_push_offhours",
            move |_uuid, _lock| {
                let ctx = ctx.clone();
                Box::pin(async move {
                    if let Err(e) = run_news_push(&ctx.api, &*ctx.social).await {
                        crate::log::error!("[automation] builtin news_push error: {e}");
                    }
                })
            }
        );
    }

    // ── Replay events: intraday incremental capture ──
    {
        let ctx = ctx.clone();
        let schedule = String::from("0 */10 9-15 * * 1-5");
        add_job!(
            scheduler,
            schedule,
            "builtin_replay_intraday",
            move |_uuid, _lock| {
                let ctx = ctx.clone();
                Box::pin(async move {
                    if let Err(e) = run_replay_intraday(&ctx).await {
                        crate::log::warn!("[automation] builtin replay intraday error: {e}");
                    }
                })
            }
        );
    }

    // ── Replay events: post-close reconcile (with one retry) ──
    {
        let ctx = ctx.clone();
        let schedule = String::from("0 20,40 15 * * 1-5");
        add_job!(
            scheduler,
            schedule,
            "builtin_replay_close",
            move |_uuid, _lock| {
                let ctx = ctx.clone();
                Box::pin(async move {
                    if let Err(e) = run_replay_close(&ctx).await {
                        crate::log::warn!("[automation] builtin replay close error: {e}");
                    }
                })
            }
        );
    }

    crate::log::info!(
        "[automation] builtin jobs registered (collection + news push + replay capture)"
    );
    Ok(())
}

/// 盘中每 10 分钟落一次当天事件，防止收盘任务失败丢数据。
async fn run_replay_intraday(ctx: &AppContext) -> Result<()> {
    let now = crate::utils::time::beijing_now();
    let open = chrono::NaiveTime::from_hms_opt(9, 30, 0)
        .unwrap_or_else(|| panic!("09:30:00 is a valid time"));
    let finalize_at = chrono::NaiveTime::from_hms_opt(15, 5, 0)
        .unwrap_or_else(|| panic!("15:05:00 is a valid time"));
    if now.time() < open || now.time() >= finalize_at {
        return Ok(());
    }
    let request = crate::replay::CaptureRequest {
        date: now.format("%Y-%m-%d").to_string(),
        min_pcp: crate::replay::PERSIST_MIN_PCP,
        finalize: false,
    };
    match crate::replay::capture_day(ctx, &request).await {
        Ok(day) => crate::log::debug!(
            "[automation] replay intraday captured {} ({} events)",
            day.date,
            day.event_count
        ),
        // 开盘初期/节假日没有事件是正常情况，不按错误处理。
        Err(error) => crate::log::debug!("[automation] replay intraday skipped: {error}"),
    }
    Ok(())
}

/// 收盘后全量对账，把当日标记为完整日。
async fn run_replay_close(ctx: &AppContext) -> Result<()> {
    let date = crate::api::xuangubao::XuanguBaoClient::latest_completed_trade_date();
    let request = crate::replay::CaptureRequest {
        date,
        min_pcp: crate::replay::PERSIST_MIN_PCP,
        finalize: true,
    };
    let day = crate::replay::capture_day(ctx, &request).await?;
    crate::log::info!(
        "[automation] replay close finalized {} ({} events, {:?})",
        day.date,
        day.event_count,
        day.status
    );
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
            crate::log::warn!("[automation] builtin collection: fetch overview failed: {e}");
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
            crate::log::warn!("[automation] builtin collection: fetch limit_up failed: {e}");
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
            crate::log::warn!("[automation] builtin collection: fetch super_stock failed: {e}");
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
        crate::log::info!("[automation] builtin news_push: initialized, skipping existing items");
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
            crate::log::warn!("[automation] builtin news_push: send failed: {e}");
        }
    }

    crate::log::info!(
        "[automation] builtin news_push: pushed {} important items (A/B level)",
        important.len()
    );
    Ok(())
}
