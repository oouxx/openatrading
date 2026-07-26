use chrono::{Datelike, NaiveTime, Weekday};
use moka::sync::Cache;
use std::sync::Arc;
use std::time::Duration;

pub type AppCache = Cache<String, CachedEntry>;

// ── A-share market hours constants ──────────────────────────
// Safety: these are compile-time-known valid clock times (HMS).
const MORNING_OPEN: NaiveTime = NaiveTime::from_hms_opt(9, 30, 0).expect("9:30 is valid");
const MORNING_CLOSE: NaiveTime = NaiveTime::from_hms_opt(11, 30, 0).expect("11:30 is valid");
const AFTERNOON_OPEN: NaiveTime = NaiveTime::from_hms_opt(13, 0, 0).expect("13:00 is valid");
const AFTERNOON_CLOSE: NaiveTime = NaiveTime::from_hms_opt(15, 0, 0).expect("15:00 is valid");

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CachedEntry {
    pub data: String,
    pub expires_at: Option<i64>,
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
    match current_market_phase() {
        MarketPhase::MorningOpen | MarketPhase::AfternoonOpen => true,
        _ => false,
    }
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

#[derive(Debug, Clone, Copy)]
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
    let entry = cache.get(key)?;
    if let Some(expires_at) = entry.expires_at {
        if openatrading_core::utils::time::beijing_now().timestamp() > expires_at {
            cache.invalidate(key);
            return None;
        }
    }
    Some(entry.data)
}

pub fn set(cache: &AppCache, key: &str, value: &str, policy: CachePolicy) {
    let expires_at = policy.compute_expires_at();
    if expires_at.is_none() && matches!(policy, CachePolicy::Never) {
        return;
    }
    cache.insert(
        key.to_string(),
        CachedEntry {
            data: value.to_string(),
            expires_at,
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
    if let Some(cached) = get(cache, key) {
        return Ok(cached);
    }

    let value = fetch().await?;
    set(cache, key, &value, policy);
    Ok(value)
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
