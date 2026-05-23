use chrono::{Datelike, Local, NaiveTime, Weekday};
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

pub fn current_market_phase() -> MarketPhase {
    let now = Local::now();
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
    let now = Local::now();
    let time = now.time();
    let open_am = NaiveTime::from_hms_opt(9, 30, 0).unwrap();
    let open_pm = NaiveTime::from_hms_opt(13, 0, 0).unwrap();

    match current_market_phase() {
        MarketPhase::PreOpen => {
            let secs = (open_am - time).num_seconds();
            now.timestamp() + secs
        }
        MarketPhase::MorningBreak => {
            let secs = (open_pm - time).num_seconds();
            now.timestamp() + secs
        }
        MarketPhase::Closed => {
            let tomorrow = (now + chrono::Duration::days(1))
                .date_naive()
                .and_hms_opt(9, 30, 0)
                .unwrap();
            tomorrow.and_utc().timestamp()
        }
        MarketPhase::Weekend => {
            let days_to_mon = match now.weekday() {
                Weekday::Sat => 2,
                Weekday::Sun => 1,
                _ => 0,
            };
            let monday = (now + chrono::Duration::days(days_to_mon))
                .date_naive()
                .and_hms_opt(9, 30, 0)
                .unwrap();
            monday.and_utc().timestamp()
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

    pub fn telegraph() -> Self {
        if is_trading_hours() {
            CachePolicy::TradingTtl(Duration::from_secs(30))
        } else {
            CachePolicy::Always(Duration::from_secs(300))
        }
    }

    fn compute_expires_at(&self) -> Option<i64> {
        let now = Local::now().timestamp();
        match self {
            CachePolicy::Never => None,
            CachePolicy::Always(ttl) => Some(now + ttl.as_secs() as i64),
            CachePolicy::TradingTtl(ttl) => Some(now + ttl.as_secs() as i64),
            CachePolicy::NonTradingUntilOpen => {
                let ts = next_open_timestamp();
                if ts > now {
                    Some(ts)
                } else {
                    None
                }
            }
        }
    }
}

pub fn get(cache: &AppCache, key: &str) -> Option<String> {
    let entry = cache.get(key)?;
    if let Some(expires_at) = entry.expires_at {
        if Local::now().timestamp() > expires_at {
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

use axum::http::StatusCode;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_daily() {
        let policy = CachePolicy::daily();
        let expires = policy.compute_expires_at();
        assert!(expires.is_some());
    }

    #[test]
    fn test_cache_set_get() {
        let cache = AppCache::new(100);
        set(
            &cache,
            "test_key",
            "test_value",
            CachePolicy::Always(Duration::from_secs(60)),
        );
        assert_eq!(get(&cache, "test_key"), Some("test_value".to_string()));
    }

    #[test]
    fn test_cache_expired() {
        let cache = AppCache::new(100);
        cache.insert(
            "test_key".to_string(),
            CachedEntry {
                data: "test_value".to_string(),
                expires_at: Some(Local::now().timestamp() - 1),
            },
        );
        assert_eq!(get(&cache, "test_key"), None);
    }
}
