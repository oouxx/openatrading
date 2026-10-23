//! 行情回放的采集与读取编排。
//!
//! 页面和定时任务共用同一套逻辑：
//! - 盘中：增量 upsert，状态为 [`ReplayDayStatus::Partial`]；
//! - 收盘后：全量重拉并按事件 id 覆盖，校验覆盖到开盘后标记
//!   [`ReplayDayStatus::Complete`]；
//! - 读取时只做过滤，不再访问上游（除非该日尚未采集）。

use anyhow::{anyhow, Result};
use chrono::NaiveTime;

use crate::api::xuangubao::{ReplayEvent, XuanguBaoClient};
use crate::context::AppContext;
use crate::memory::{ReplayDay, ReplayDayStatus};
use crate::utils::time::beijing_now;

/// 落库阈值比页面默认筛选阈值更低，保留细粒度数据供后续分析。
pub const PERSIST_MIN_PCP: f64 = 0.005;

/// 收盘后等待上游索引完成再定稿。
const FINALIZE_BUFFER_MINUTES: u32 = 5;

#[derive(Debug, Clone)]
pub struct CaptureRequest {
    pub date: String,
    pub min_pcp: f64,
    /// 是否尝试把该日标记为完整日（盘中任务传 `false`）。
    pub finalize: bool,
}

#[derive(Debug, Clone)]
pub struct ReplayDayData {
    pub day: ReplayDay,
    pub events: Vec<ReplayEvent>,
    pub index: Option<serde_json::Value>,
}

/// 采集某个交易日的事件并落库。
///
/// # Errors
/// 日期非法、上游请求失败或数据库写入失败时返回错误。空数据不会写入，
/// 避免节假日或上游故障污染回放日列表。
pub async fn capture_day(ctx: &AppContext, request: &CaptureRequest) -> Result<ReplayDay> {
    let batch = ctx
        .api
        .xuangubao
        .fetch_events_for_date(&request.date, request.min_pcp)
        .await?;
    let events: Vec<ReplayEvent> = batch.events.iter().map(ReplayEvent::from).collect();

    let store = ctx.memory.replay();
    let existing = store.day(&request.date)?;

    if events.is_empty() {
        if let Some(day) = existing {
            return Ok(day);
        }
        return Err(anyhow!("{} 没有可用的回放事件", request.date));
    }

    let now = beijing_now();
    let closed = is_market_closed(&request.date, &now);
    let full_day = closed && batch.reached_open;
    let already_complete = existing
        .as_ref()
        .is_some_and(|day| day.status == ReplayDayStatus::Complete);

    // 完整日刷新时如果分页没有覆盖到开盘，宁可保留旧数据，也不能用
    // 残缺结果覆盖并继续标记为 complete。
    if already_complete && !full_day {
        let Some(day) = existing else {
            return Err(anyhow!("{} 回放日状态异常", request.date));
        };
        return Ok(day);
    }

    if request.finalize && full_day {
        // 全量覆盖，清掉上游已修正/撤销的事件。
        store.replace_events(&request.date, &events)?;
    } else {
        store.upsert_events(&request.date, &events)?;
    }

    let complete = already_complete || (request.finalize && full_day);
    let completed_at = existing
        .as_ref()
        .and_then(|day| day.completed_at.clone())
        .or_else(|| complete.then(|| now.format("%Y-%m-%d %H:%M:%S").to_string()));

    let day = ReplayDay {
        date: request.date.clone(),
        status: if complete {
            ReplayDayStatus::Complete
        } else {
            ReplayDayStatus::Partial
        },
        event_count: u32::try_from(events.len()).unwrap_or(u32::MAX),
        first_event_timestamp: events.first().map(|event| event.event_timestamp),
        last_event_timestamp: events.last().map(|event| event.event_timestamp),
        captured_at: now.format("%Y-%m-%d %H:%M:%S").to_string(),
        completed_at,
    };
    store.save_day(&day)?;

    // 指数分时接口只提供“当前交易日”的数据；历史日期在事后补采时拿不到，
    // 因此只在采集当天保存，避免把今天的走势错误地挂到历史日期上。
    let today = beijing_now().format("%Y-%m-%d").to_string();
    if complete || request.date == today {
        persist_index(ctx, &request.date).await;
    }

    Ok(day)
}

/// 读取已落库的某日回放数据。
///
/// # Errors
/// 数据库读取失败。
pub fn load_day(ctx: &AppContext, date: &str) -> Result<Option<ReplayDayData>> {
    let store = ctx.memory.replay();
    let Some(day) = store.day(date)? else {
        return Ok(None);
    };
    let events = store.events(date)?;
    let index = store
        .index_json(date)?
        .and_then(|json| serde_json::from_str(&json).ok());
    Ok(Some(ReplayDayData { day, events, index }))
}

/// 确保某日数据存在；缺数据时补采，盘中数据收盘后自动升级为完整日。
///
/// # Errors
/// 上游请求失败或数据库读写失败时返回错误。
pub async fn ensure_day(ctx: &AppContext, date: &str) -> Result<ReplayDayData> {
    let existing = load_day(ctx, date)?;
    let needs_capture = match existing.as_ref() {
        None => true,
        Some(data) => {
            data.day.status == ReplayDayStatus::Partial && is_market_closed(date, &beijing_now())
        }
    };

    if needs_capture {
        let request = CaptureRequest {
            date: date.to_string(),
            min_pcp: PERSIST_MIN_PCP,
            finalize: true,
        };
        if let Err(error) = capture_day(ctx, &request).await {
            // 收盘对账失败时，已经落库的盘中快照仍然有价值；
            // 不要因为上游临时异常让整天的数据变得不可读。
            if let Some(data) = existing {
                crate::log::warn!("[replay] {date} 定稿失败，返回盘中快照: {error}");
                return Ok(data);
            }
            return Err(error);
        }
    }

    load_day(ctx, date)?.ok_or_else(|| anyhow!("回放数据未生成: {date}"))
}

/// 默认打开最近一个完整交易日；节假日/空数据时向前回退。
///
/// # Errors
/// 所有候选日期都没有数据时返回最后一个错误。
pub async fn ensure_latest_day(ctx: &AppContext) -> Result<ReplayDayData> {
    let latest = XuanguBaoClient::latest_completed_trade_date();
    let mut last_error = None;
    for date in candidate_dates(&latest) {
        match ensure_day(ctx, &date).await {
            Ok(data) if !data.events.is_empty() => return Ok(data),
            Ok(_) => {}
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow!("最近交易日没有回放数据")))
}

/// 最近交易日 + 最多往前 6 个工作日（覆盖节假日）。
fn candidate_dates(latest: &str) -> Vec<String> {
    let mut dates = vec![latest.to_string()];
    dates.extend(
        XuanguBaoClient::last_n_trading_days(7)
            .into_iter()
            .filter(|date| date.as_str() < latest),
    );
    dates.dedup();
    dates
}

fn is_market_closed(date: &str, now: &chrono::DateTime<chrono::FixedOffset>) -> bool {
    let today = now.format("%Y-%m-%d").to_string();
    if date < today.as_str() {
        return true;
    }
    if date > today.as_str() {
        return false;
    }
    let finalize_at = NaiveTime::from_hms_opt(15, FINALIZE_BUFFER_MINUTES, 0)
        .unwrap_or_else(|| panic!("15:{FINALIZE_BUFFER_MINUTES}:00 is a valid time"));
    now.time() >= finalize_at
}

async fn persist_index(ctx: &AppContext, date: &str) {
    let today = beijing_now().format("%Y-%m-%d").to_string();
    if date != today {
        return;
    }
    match crate::api::xuangubao::fetch_index_trends(&ctx.api.xuangubao).await {
        Ok(index) => match serde_json::to_string(&index) {
            Ok(json) => {
                if let Err(error) = ctx.memory.replay().set_index_json(date, &json) {
                    crate::log::warn!("[replay] 保存指数分时失败: {error}");
                }
            }
            Err(error) => crate::log::warn!("[replay] 序列化指数分时失败: {error}"),
        },
        Err(error) => crate::log::warn!("[replay] 拉取指数分时失败: {error}"),
    }
}
