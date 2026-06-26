use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// 断路器状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// 正常
    Closed,
    /// 熔断中
    Open,
    /// 半开（尝试恢复）
    HalfOpen,
}

/// 断路器——保护对外部平台的请求不被重复失败的调用打垮。
///
/// 连续失败达 `threshold` 次后切换到 Open，拒绝所有请求；
/// 经过 `reset_timeout` 后切换到 HalfOpen，允许一次试探；
/// 试探成功则回到 Closed，否则回到 Open。
#[derive(Debug)]
pub struct CircuitBreaker {
    inner: Arc<CircuitBreakerInner>,
}

#[derive(Debug)]
struct CircuitBreakerInner {
    failures: AtomicU32,
    state: AtomicU8,
    opened_at: Mutex<Option<Instant>>,
    threshold: u32,
    reset_timeout: std::time::Duration,
}

const STATE_CLOSED: u8 = 0;
const STATE_OPEN: u8 = 1;
const STATE_HALF_OPEN: u8 = 2;

impl CircuitBreaker {
    pub fn new(threshold: u32, reset_timeout: std::time::Duration) -> Self {
        Self {
            inner: Arc::new(CircuitBreakerInner {
                failures: AtomicU32::new(0),
                state: AtomicU8::new(STATE_CLOSED),
                opened_at: Mutex::new(None),
                threshold,
                reset_timeout,
            }),
        }
    }

    /// 检查当前是否允许请求通过。
    pub fn is_allowed(&self) -> bool {
        let state = self.inner.state.load(Ordering::Acquire);
        match state {
            STATE_CLOSED => true,
            STATE_HALF_OPEN => true,
            STATE_OPEN => {
                let opened = self.inner.opened_at.lock().unwrap();
                if let Some(at) = *opened {
                    if at.elapsed() >= self.inner.reset_timeout {
                        self.inner
                            .state
                            .compare_exchange(
                                STATE_OPEN,
                                STATE_HALF_OPEN,
                                Ordering::AcqRel,
                                Ordering::Acquire,
                            )
                            .ok();
                        return true;
                    }
                }
                false
            },
            _ => false,
        }
    }

    /// 记录一次成功——回到 Closed，重置失败计数。
    pub fn record_success(&self) {
        self.inner.failures.store(0, Ordering::Release);
        let old = self.inner.state.swap(STATE_CLOSED, Ordering::AcqRel);
        if old == STATE_OPEN {
            *self.inner.opened_at.lock().unwrap() = None;
        }
    }

    /// 记录一次失败——达到阈值则切到 Open。
    pub fn record_failure(&self) {
        let prev = self.inner.failures.fetch_add(1, Ordering::AcqRel);
        if prev + 1 >= self.inner.threshold {
            let old = self.inner.state.swap(STATE_OPEN, Ordering::AcqRel);
            if old != STATE_OPEN {
                *self.inner.opened_at.lock().unwrap() = Some(Instant::now());
            }
        }
    }

    /// 获取当前状态（用于监控/日志）。
    pub fn state(&self) -> CircuitState {
        match self.inner.state.load(Ordering::Acquire) {
            STATE_CLOSED => CircuitState::Closed,
            STATE_OPEN => CircuitState::Open,
            STATE_HALF_OPEN => CircuitState::HalfOpen,
            _ => unreachable!(),
        }
    }

    /// 获取当前连续失败次数。
    pub fn failures(&self) -> u32 {
        self.inner.failures.load(Ordering::Acquire)
    }

    /// 重置为初始状态。
    pub fn reset(&self) {
        self.inner.failures.store(0, Ordering::Release);
        self.inner.state.store(STATE_CLOSED, Ordering::Release);
        *self.inner.opened_at.lock().unwrap() = None;
    }
}

impl Clone for CircuitBreaker {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl Default for CircuitBreaker {
    /// 默认：连续 5 次失败熔断 30 秒。
    fn default() -> Self {
        Self::new(5, std::time::Duration::from_secs(30))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_starts_closed() {
        let cb = CircuitBreaker::default();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.is_allowed());
    }

    #[test]
    fn test_opens_after_threshold() {
        let cb = CircuitBreaker::new(3, std::time::Duration::from_secs(60));
        assert!(cb.is_allowed());
        cb.record_failure();
        assert!(cb.is_allowed());
        cb.record_failure();
        assert!(cb.is_allowed());
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.is_allowed());
    }

    #[test]
    fn test_success_resets() {
        let cb = CircuitBreaker::new(2, std::time::Duration::from_secs(60));
        cb.record_failure();
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.failures(), 0);
    }

    #[test]
    fn test_clone() {
        let cb1 = CircuitBreaker::default();
        let cb2 = cb1.clone();
        cb1.record_failure();
        assert_eq!(cb2.failures(), 1);
    }
}
