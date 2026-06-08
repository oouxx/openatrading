use chrono::{Datelike, NaiveTime, Weekday};
use moka::sync::Cache;
use std::sync::Arc;
use std::time::Duration;

pub type AppCache = Cache<String, CachedEntry>;

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
    pub fn as_str(&self) -> &'static str {
        match self {
            MarketPhase::PreOpen => "preopen",
            MarketPhase::MorningOpen => "morningopen",
            MarketPhase::MorningBreak => "morningbreak",
            MarketPhase::AfternoonOpen => "afternoonopen",
            MarketPhase::Closed => "closed",
            MarketPhase::Weekend => "weekend",
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
    let open_am = NaiveTime::from_hms_opt(9, 30, 0).unwrap();
    let close_am = NaiveTime::from_hms_opt(11, 30, 0).unwrap();
    let open_pm = NaiveTime::from_hms_opt(13, 0, 0).unwrap();
    let close_pm = NaiveTime::from_hms_opt(15, 0, 0).unwrap();

    if time < open_am {
        MarketPhase::PreOpen
    } else if time < close_am {
        MarketPhase::MorningOpen
    } else if time < open_pm {
        MarketPhase::MorningBreak
    } else if time < close_pm {
        MarketPhase::AfternoonOpen
    } else {
        MarketPhase::Closed
    }
}

fn is_trading_hours() -> bool {
    matches!(
        current_market_phase(),
        MarketPhase::MorningOpen | MarketPhase::AfternoonOpen
    )
}

fn next_open_timestamp() -> i64 {
    let now = openatrading_core::utils::time::beijing_now();
    let time = now.time();
    let open_am = NaiveTime::from_hms_opt(9, 30, 0).unwrap();
    let open_pm = NaiveTime::from_hms_opt(13, 0, 0).unwrap();

    match current_market_phase() {
        MarketPhase::PreOpen => {
            let secs = (open_am - time).num_seconds();
            now.timestamp() + secs
        },
        MarketPhase::MorningBreak => {
            let secs = (open_pm - time).num_seconds();
            now.timestamp() + secs
        },
        MarketPhase::Closed | MarketPhase::Weekend => {
            // Find the next trading day (skip weekends)
            let mut days_ahead = 1i64;
            loop {
                let next = now + chrono::Duration::days(days_ahead);
                if next.weekday() != Weekday::Sat && next.weekday() != Weekday::Sun {
                    let open = next
                        .date_naive()
                        .and_hms_opt(9, 30, 0)
                        .unwrap()
                        .and_utc()
                        .timestamp();
                    return open;
                }
                days_ahead += 1;
            }
        },
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
            CachePolicy::TradingTtl(Duration::from_secs(30))
        } else {
            CachePolicy::NonTradingUntilOpen
        }
    }

    pub fn realtime() -> Self {
        if is_trading_hours() {
            CachePolicy::TradingTtl(Duration::from_secs(10))
        } else {
            CachePolicy::NonTradingUntilOpen
        }
    }

    pub fn snapshot() -> Self {
        if is_trading_hours() {
            CachePolicy::TradingTtl(Duration::from_secs(60))
        } else {
            CachePolicy::NonTradingUntilOpen
        }
    }

    pub fn daily() -> Self {
        CachePolicy::Always(Duration::from_secs(3600 * 6))
    }

    /// Replay data: cache after close until next open; no cache during trading.
    pub fn replay() -> Self {
        if is_trading_hours() {
            CachePolicy::Never
        } else {
            CachePolicy::TradingTtl(Duration::from_secs(60))
        }
    }

    pub fn never() -> Self {
        CachePolicy::Never
    }

    pub fn telegraph() -> Self {
        if is_trading_hours() {
            CachePolicy::TradingTtl(Duration::from_secs(30))
        } else {
            CachePolicy::Always(Duration::from_secs(300))
        }
    }

    fn compute_expires_at(&self) -> Option<i64> {
        let now = openatrading_core::utils::time::beijing_now().timestamp();
        match self {
            CachePolicy::Never => None,
            CachePolicy::Always(ttl) => Some(now + ttl.as_secs() as i64),
            CachePolicy::TradingTtl(ttl) => Some(now + ttl.as_secs() as i64),
            CachePolicy::NonTradingUntilOpen => {
                let ts = next_open_timestamp();
                if ts > now {
                    Some(ts)
                } else {
                    // Fallback: short TTL if we can't compute the next open
                    Some(now + 60)
                }
            },
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

/// Like `get_or_fetch` but works with any Serialize type and returns `Json<Value>` directly,
/// eliminating the to_string → from_str round-trip in every handler.
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
