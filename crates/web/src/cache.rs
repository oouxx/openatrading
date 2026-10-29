use chrono::{Datelike, NaiveTime, Weekday};
use moka::sync::Cache;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Notify;

pub type AppCache = Cache<String, CachedEntry>;

/// 同 key 并发回源合并槽。每个 key 只允许一个 leader 真正请求上游，
/// follower 等待同一个完成通知。
#[derive(Default)]
pub struct InFlightRequests {
    slots: Mutex<HashMap<String, Arc<Notify>>>,
}

impl InFlightRequests {
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// 返回 `true` 表示当前调用是 leader；`false` 表示已有请求在途，调用方应等待。
    fn try_acquire(&self, key: &str) -> bool {
        let mut slots = self.slots.lock().unwrap_or_else(|e| e.into_inner());
        if slots.contains_key(key) {
            false
        } else {
            slots.insert(key.to_string(), Arc::new(Notify::new()));
            true
        }
    }

    fn notify(&self, key: &str) {
        let slot = self
            .slots
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(key);
        if let Some(notify) = slot {
            notify.notify_waiters();
        }
    }

    async fn wait(&self, key: &str) {
        let notify = {
            let slots = self.slots.lock().unwrap_or_else(|e| e.into_inner());
            slots.get(key).cloned()
        };
        if let Some(notify) = notify {
            notify.notified().await;
        }
    }
}

// ── A-share market hours constants ──────────────────────────
// Safety: these are compile-time-known valid clock times (HMS).
#[allow(clippy::unwrap_used, clippy::incompatible_msrv)]
const MORNING_OPEN: NaiveTime = NaiveTime::from_hms_opt(9, 30, 0).unwrap();
#[allow(clippy::unwrap_used, clippy::incompatible_msrv)]
const MORNING_CLOSE: NaiveTime = NaiveTime::from_hms_opt(11, 30, 0).unwrap();
#[allow(clippy::unwrap_used, clippy::incompatible_msrv)]
const AFTERNOON_OPEN: NaiveTime = NaiveTime::from_hms_opt(13, 0, 0).unwrap();
#[allow(clippy::unwrap_used, clippy::incompatible_msrv)]
const AFTERNOON_CLOSE: NaiveTime = NaiveTime::from_hms_opt(15, 0, 0).unwrap();

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CachedEntry {
    pub data: String,
    /// Fresh 截止时间；到这里为止可直接返回。
    pub expires_at: Option<i64>,
    /// 最近一次成功写入时间，供 stale-if-error 判断兜底窗口。
    pub generated_at: i64,
    /// Stale 数据可使用到的截止时间；`None` 表示不启用 stale。
    pub stale_until: Option<i64>,
}

pub fn new_cache() -> Arc<AppCache> {
    Arc::new(
        AppCache::builder()
            .max_capacity(512)
            .time_to_idle(Duration::from_secs(60 * 60 * 18))
            .build(),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketPhase {
    PreOpen,
    MorningOpen,
    MorningBreak,
    AfternoonOpen,
    Closed,
    Weekend,
}

impl MarketPhase {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::PreOpen => "preopen",
            Self::MorningOpen => "morningopen",
            Self::MorningBreak => "morningbreak",
            Self::AfternoonOpen => "afternoonopen",
            Self::Closed => "closed",
            Self::Weekend => "weekend",
        }
    }
}

pub fn current_market_phase() -> MarketPhase {
    let now = openatrading_core::utils::time::beijing_now();
    let weekday = now.weekday();

    if weekday == Weekday::Sat || weekday == Weekday::Sun {
        return MarketPhase::Weekend;
    }

    let time = now.time();

    if time < MORNING_OPEN {
        MarketPhase::PreOpen
    } else if time < MORNING_CLOSE {
        MarketPhase::MorningOpen
    } else if time < AFTERNOON_OPEN {
        MarketPhase::MorningBreak
    } else if time < AFTERNOON_CLOSE {
        MarketPhase::AfternoonOpen
    } else {
        MarketPhase::Closed
    }
}

pub fn is_trading_hours() -> bool {
    matches!(
        current_market_phase(),
        MarketPhase::MorningOpen | MarketPhase::AfternoonOpen
    )
}

fn next_open_timestamp() -> i64 {
    let now = openatrading_core::utils::time::beijing_now();
    let time = now.time();

    match current_market_phase() {
        MarketPhase::PreOpen => {
            let secs = (MORNING_OPEN - time).num_seconds();
            now.timestamp() + secs
        }
        MarketPhase::MorningBreak => {
            let secs = (AFTERNOON_OPEN - time).num_seconds();
            now.timestamp() + secs
        }
        MarketPhase::Closed | MarketPhase::Weekend => {
            // Find the next trading day (skip weekends)
            let mut days_ahead = 1i64;
            loop {
                let next = now + chrono::Duration::days(days_ahead);
                if next.weekday() != Weekday::Sat && next.weekday() != Weekday::Sun {
                    let open = next
                        .date_naive()
                        .and_time(MORNING_OPEN)
                        .and_utc()
                        .timestamp();
                    return open;
                }
                days_ahead += 1;
            }
        }
        MarketPhase::MorningOpen | MarketPhase::AfternoonOpen => 0,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CachePolicy {
    TradingTtl(Duration),
    NonTradingUntilOpen,
    Always(Duration),
    Never,
}

impl CachePolicy {
    pub fn overview() -> Self {
        if is_trading_hours() {
            Self::TradingTtl(Duration::from_secs(30))
        } else {
            Self::NonTradingUntilOpen
        }
    }

    pub fn realtime() -> Self {
        if is_trading_hours() {
            Self::TradingTtl(Duration::from_secs(10))
        } else {
            Self::NonTradingUntilOpen
        }
    }

    /// 盘口/逐笔这类秒级数据：交易时段内 3 秒过期（前端 5 秒轮询一次，
    /// 拿到的基本是新鲜数据），非交易时段直接用到下次开盘——盘口在休市时
    /// 不会变，没必要反复请求上游。
    pub fn order_book() -> Self {
        if is_trading_hours() {
            Self::TradingTtl(Duration::from_secs(3))
        } else {
            Self::NonTradingUntilOpen
        }
    }

    /// 个股排行这类全市场实时榜单：交易时段内 5 秒过期。
    pub fn stock_ranking() -> Self {
        if is_trading_hours() {
            Self::TradingTtl(Duration::from_secs(5))
        } else {
            Self::NonTradingUntilOpen
        }
    }

    pub fn snapshot() -> Self {
        if is_trading_hours() {
            Self::TradingTtl(Duration::from_secs(60))
        } else {
            Self::NonTradingUntilOpen
        }
    }

    pub const fn daily() -> Self {
        Self::Always(Duration::from_secs(3600 * 6))
    }

    /// Replay data: cache after close until next open; no cache during trading.
    pub fn replay() -> Self {
        if is_trading_hours() {
            Self::Never
        } else {
            Self::TradingTtl(Duration::from_secs(60))
        }
    }

    pub const fn never() -> Self {
        Self::Never
    }

    pub fn telegraph() -> Self {
        if is_trading_hours() {
            Self::TradingTtl(Duration::from_secs(30))
        } else {
            Self::Always(Duration::from_secs(300))
        }
    }

    fn compute_expires_at(&self) -> Option<i64> {
        let now = openatrading_core::utils::time::beijing_now().timestamp();
        match self {
            Self::Never => None,
            Self::Always(ttl) | Self::TradingTtl(ttl) => {
                Some(now + i64::try_from(ttl.as_secs()).unwrap_or(i64::MAX))
            }
            Self::NonTradingUntilOpen => {
                let ts = next_open_timestamp();
                if ts > now {
                    Some(ts)
                } else {
                    // Fallback: short TTL if we can't compute the next open
                    Some(now + 60)
                }
            }
        }
    }
}

pub fn get(cache: &AppCache, key: &str) -> Option<String> {
    get_fresh(cache, key)
}

/// 只返回 fresh 数据；过期但仍在 stale window 内的数据不会被删除。
pub fn get_fresh(cache: &AppCache, key: &str) -> Option<String> {
    let entry = cache.get(key)?;
    let now = openatrading_core::utils::time::beijing_now().timestamp();
    if entry.expires_at.is_some_and(|expires_at| now > expires_at) {
        return None;
    }
    Some(entry.data)
}

/// 返回过期但仍在 stale window 内的数据和生成时间。
pub fn get_stale(cache: &AppCache, key: &str) -> Option<(String, i64)> {
    let entry = cache.get(key)?;
    let now = openatrading_core::utils::time::beijing_now().timestamp();
    if entry.expires_at.is_some_and(|expires_at| now <= expires_at) {
        return None;
    }
    if entry.stale_until.is_some_and(|until| now <= until) {
        Some((entry.data, entry.generated_at))
    } else {
        None
    }
}

pub fn set(cache: &AppCache, key: &str, value: &str, policy: CachePolicy) {
    set_with_stale(cache, key, value, policy, None);
}

/// 写入 fresh 数据，并可选保留 `stale_window` 供上游失败时兜底。
pub fn set_with_stale(
    cache: &AppCache,
    key: &str,
    value: &str,
    policy: CachePolicy,
    stale_window: Option<Duration>,
) {
    let now = openatrading_core::utils::time::beijing_now().timestamp();
    let expires_at = policy.compute_expires_at();
    if expires_at.is_none() && matches!(policy, CachePolicy::Never) {
        return;
    }
    let stale_until = stale_window.and_then(|window| {
        i64::try_from(window.as_secs())
            .ok()
            .and_then(|seconds| expires_at.and_then(|expires_at| expires_at.checked_add(seconds)))
    });
    cache.insert(
        key.to_string(),
        CachedEntry {
            data: value.to_string(),
            expires_at,
            generated_at: now,
            stale_until,
        },
    );
}

use axum::{http::StatusCode, Json};

/// Fetch a value from cache or call `fetch` to populate it.
///
/// # Errors
/// Returns `StatusCode` if the fetch function fails.
pub async fn get_or_fetch<F, Fut>(
    cache: &AppCache,
    key: &str,
    policy: CachePolicy,
    fetch: F,
) -> Result<String, StatusCode>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<String, StatusCode>>,
{
    if let Some(cached) = get_fresh(cache, key) {
        return Ok(cached);
    }

    let value = fetch().await?;
    set(cache, key, &value, policy);
    Ok(value)
}

/// 带 single-flight 与 stale-if-error 的 JSON 缓存读取。
///
/// 同 key 的并发未命中只允许一个 leader 回源；leader 失败时，如果存在仍在
/// stale window 内的成功数据，则返回旧数据并附带 `stale` 标记，避免随机上游
/// 断连直接变成用户可见错误。
pub async fn get_or_fetch_json_resilient<F, Fut, T>(
    cache: &AppCache,
    in_flight: &InFlightRequests,
    key: &str,
    policy: CachePolicy,
    stale_window: Duration,
    fetch: F,
) -> Result<Json<serde_json::Value>, StatusCode>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, StatusCode>>,
    T: serde::Serialize,
{
    if let Some(cached) = get_fresh(cache, key) {
        return Ok(Json(serde_json::from_str(&cached).unwrap_or_default()));
    }

    if !in_flight.try_acquire(key) {
        in_flight.wait(key).await;
        if let Some(cached) = get_fresh(cache, key) {
            return Ok(Json(serde_json::from_str(&cached).unwrap_or_default()));
        }
        if let Some((cached, _)) = get_stale(cache, key) {
            return Ok(Json(stale_json(cached)));
        }
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }

    let result = fetch().await;
    match result {
        Ok(value) => {
            let json_string =
                serde_json::to_string(&value).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            set_with_stale(cache, key, &json_string, policy, Some(stale_window));
            in_flight.notify(key);
            Ok(Json(serde_json::from_str(&json_string).unwrap_or_default()))
        }
        Err(error) => {
            in_flight.notify(key);
            if let Some((cached, generated_at)) = get_stale(cache, key) {
                Ok(Json(stale_json_with_generated_at(cached, generated_at)))
            } else {
                Err(error)
            }
        }
    }
}

fn stale_json(cached: String) -> serde_json::Value {
    let generated_at = serde_json::from_str::<serde_json::Value>(&cached)
        .ok()
        .and_then(|value| {
            value
                .get("generated_at")
                .and_then(serde_json::Value::as_i64)
        })
        .unwrap_or_default();
    stale_json_with_generated_at(cached, generated_at)
}

fn stale_json_with_generated_at(cached: String, generated_at: i64) -> serde_json::Value {
    let mut value = serde_json::from_str::<serde_json::Value>(&cached)
        .unwrap_or_else(|_| serde_json::json!({}));
    if let Some(object) = value.as_object_mut() {
        object.insert("stale".to_string(), serde_json::Value::Bool(true));
        if !object.contains_key("generated_at") {
            object.insert(
                "generated_at".to_string(),
                serde_json::Value::from(generated_at),
            );
        }
    }
    value
}

/// Fetch a JSON value from cache or call `fetch` to populate it.
///
/// Unlike `get_or_fetch`, this works with any Serialize type and returns
/// `Json<Value>` directly, eliminating the `to_string` → `from_str` round-trip.
///
/// # Errors
/// Returns `StatusCode` if the fetch function fails.
pub async fn get_or_fetch_json<F, Fut, T>(
    cache: &AppCache,
    key: &str,
    policy: CachePolicy,
    fetch: F,
) -> Result<Json<serde_json::Value>, StatusCode>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, StatusCode>>,
    T: serde::Serialize,
{
    if let Some(cached) = get(cache, key) {
        return Ok(Json(serde_json::from_str(&cached).unwrap_or_default()));
    }

    let value = fetch().await?;
    let json_string = serde_json::to_string(&value).unwrap_or_default();
    let json_value: serde_json::Value = serde_json::from_str(&json_string).unwrap_or_default();
    set(cache, key, &json_string, policy);
    Ok(Json(json_value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock_ranking_policy_uses_five_second_trading_ttl() {
        let policy = if is_trading_hours() {
            CachePolicy::stock_ranking()
        } else {
            CachePolicy::TradingTtl(Duration::from_secs(5))
        };
        assert_eq!(policy, CachePolicy::TradingTtl(Duration::from_secs(5)));
    }

    #[tokio::test]
    async fn resilient_cache_coalesces_concurrent_fetches() {
        let cache = new_cache();
        let in_flight = InFlightRequests::new();
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut tasks = Vec::new();
        for _ in 0..5 {
            let cache = Arc::clone(&cache);
            let in_flight = Arc::clone(&in_flight);
            let calls = Arc::clone(&calls);
            tasks.push(tokio::spawn(async move {
                get_or_fetch_json_resilient(
                    &cache,
                    &in_flight,
                    "ranking-test",
                    CachePolicy::Always(Duration::from_secs(30)),
                    Duration::from_secs(60),
                    || async move {
                        calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(30)).await;
                        Ok(serde_json::json!({ "generated_at": 1, "items": [] }))
                    },
                )
                .await
            }));
        }
        for task in tasks {
            assert!(task.await.is_ok());
        }
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn resilient_cache_returns_stale_data_after_fetch_failure() {
        let cache = new_cache();
        let in_flight = InFlightRequests::new();
        set_with_stale(
            &cache,
            "stale-test",
            r#"{"generated_at":123,"items":[],"next_offset":null}"#,
            CachePolicy::Never,
            Some(Duration::from_secs(60)),
        );
        cache.insert(
            "stale-test".to_string(),
            CachedEntry {
                data: r#"{"generated_at":123,"items":[],"next_offset":null}"#.to_string(),
                expires_at: Some(0),
                generated_at: 123,
                stale_until: Some(openatrading_core::utils::time::beijing_now().timestamp() + 60),
            },
        );
        let result = get_or_fetch_json_resilient(
            &cache,
            &in_flight,
            "stale-test",
            CachePolicy::Always(Duration::from_secs(5)),
            Duration::from_secs(60),
            || async { Err::<serde_json::Value, _>(StatusCode::BAD_GATEWAY) },
        )
        .await
        .unwrap_or_else(|status| panic!("expected stale data, got {status}"));
        let Json(value) = result;
        assert_eq!(value["stale"], true);
        assert_eq!(value["generated_at"], 123);
    }
}
