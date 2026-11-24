//! Shared market-stream multiplexer.
//!
//! **一种行情一个生产者**（`StreamSpec::kind`：`realtime` / `order-book` /
//! `kline:{period}:{count}`），而不是"一个符号集一个生产者"：生产者每轮拉的是
//! **所有订阅者符号的并集**，再把同一份 payload 广播给全部连接（前端按 symbol
//! 只取自己那几只，多余的键本来就会被忽略）。
//!
//! 这样上游调用量只跟"全站关注的股票总数"有关，与连接数、分片数无关：
//! 三个客户端各看 60 只不相干的股票，上游还是一次每轮；以前是三次。
//!
//! 每个连接订阅时给并集里的每个符号 +1 引用计数，连接断开时 -1，归零就移出
//! 并集——不会为没人看的票一直打上游。
//!
//! Lifecycle: 生产者连续 [`IDLE_TICKS`] 轮没有订阅者就退出，闲置流自动回收；
//! 后来的订阅者懒创建。生产者 panic 会被抓住并记日志（任务不会静默死掉）。

use std::collections::HashMap;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use futures::FutureExt;
use tokio::sync::{broadcast, Mutex};
use tokio::task::JoinHandle;

/// A fetcher maps the current symbol set to a JSON payload string (the batch
/// result that gets broadcast). Boxed so the hub stays non-generic.
pub type FetchFn = Arc<
    dyn Fn(&[String]) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send>> + Send + Sync,
>;

/// Configuration for a single shared stream. `kind` dedups producers; the same
/// kind reuses an already-running producer (its symbol set grows/shrinks with
/// subscribers).
pub struct StreamSpec {
    /// 生产者标识：同 kind 的所有订阅共用一条上游轮询（`realtime` /
    /// `order-book` / `kline:{period}:{count}`）。**不要**把符号拼进去，
    /// 那样每个不同的自选组合都会各打各的上游。
    pub kind: String,
    /// Polling interval during A-share trading hours.
    pub trading_interval: Duration,
    /// Polling interval outside trading hours (still emits initial data, then
    /// only keep-alive ticks).
    pub nontrading_interval: Duration,
    /// Produces the batch JSON payload for the current symbol set.
    pub fetch: FetchFn,
}

/// Consecutive intervals with zero subscribers before the producer reclaims
/// itself.
const IDLE_TICKS: u32 = 2;
/// Per-stream broadcast buffer capacity (each interval produces one message).
const CHANNEL_CAPACITY: usize = 64;
/// Hard cap on concurrent subscribers per stream to prevent runaway fan-out.
const MAX_SUBSCRIBERS: usize = 256;

struct StreamEntry {
    tx: broadcast::Sender<String>,
    /// Most recent payload — replayed to late subscribers so they get
    /// immediate data instead of waiting up to one interval.
    last: Arc<Mutex<Option<String>>>,
    /// 所有订阅者的符号并集（符号 → 还有几个连接在看它）。
    symbols: Arc<StdMutex<HashMap<String, usize>>>,
    handle: JoinHandle<()>,
}

/// 订阅守卫：只要它还活着，这些符号就留在上游并集里；drop 时按引用计数摘掉。
///
/// SSE 循环把它和一个具名局部变量一起持有到连接结束。
pub struct StreamGuard {
    symbols: Arc<StdMutex<HashMap<String, usize>>>,
    subscribed: Vec<String>,
}

impl Drop for StreamGuard {
    fn drop(&mut self) {
        let mut counts = lock_counts(&self.symbols);
        for symbol in self.subscribed.drain(..) {
            if let Some(count) = counts.get_mut(&symbol) {
                *count -= 1;
                if *count == 0 {
                    counts.remove(&symbol);
                }
            }
        }
    }
}

/// 一条行情订阅：接收端 + 最近一帧 + 守卫。
pub struct StreamSubscription {
    rx: broadcast::Receiver<String>,
    last: Option<String>,
    guard: StreamGuard,
}

impl StreamSubscription {
    /// 拆成 `((接收端, 最近一帧), 守卫)`——守卫必须跟着 SSE 流一起持有。
    #[must_use]
    pub fn into_parts(self) -> ((broadcast::Receiver<String>, Option<String>), StreamGuard) {
        ((self.rx, self.last), self.guard)
    }
}

/// 引用计数用 `std::sync::Mutex`：`Drop` 里要同步递减，不能用 tokio 的锁。
/// 只做"取并集 / 加减 1"这种瞬间操作，不跨 await 持有。
fn lock_counts(
    counts: &StdMutex<HashMap<String, usize>>,
) -> std::sync::MutexGuard<'_, HashMap<String, usize>> {
    counts
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// A process-wide multiplexer of market SSE streams. Store one in your app
/// state and call [`MarketStreamHub::subscribe`] from each SSE handler.
pub struct MarketStreamHub {
    streams: Mutex<HashMap<String, Arc<StreamEntry>>>,
}

impl MarketStreamHub {
    /// Create a fresh, empty hub.
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            streams: Mutex::new(HashMap::new()),
        })
    }

    /// Subscribe to `spec`'s stream for `symbols`.
    ///
    /// 同一个 `spec.kind` 只有一条上游轮询：订阅时把自己的符号并进去（引用计数 +1），
    /// 生产者下一轮就会把它们一起拉回来。返回的 [`StreamSubscription`] 要在 SSE
    /// 循环里一直持有——drop 时自动把符号从并集里摘掉。
    ///
    /// 新订阅者会拿到最近一帧（`last`）先回放，不用等下一轮。
    ///
    /// # Panics
    /// Never; the producer task catches its own panics and logs them.
    pub async fn subscribe(&self, spec: StreamSpec, symbols: Vec<String>) -> StreamSubscription {
        let kind = spec.kind.clone();
        let mut streams = self.streams.lock().await;

        // Reuse an existing live producer, or evict an idle-exited one.
        if let Some(entry) = streams.get(&kind) {
            if !entry.handle.is_finished() {
                if entry.tx.receiver_count() >= MAX_SUBSCRIBERS {
                    openatrading_core::log::warn!(
                        "[market-stream] {kind}: subscriber cap ({MAX_SUBSCRIBERS}) reached"
                    );
                }
                let rx = entry.tx.subscribe();
                let last = entry.last.lock().await.clone();
                {
                    let mut counts = lock_counts(&entry.symbols);
                    for symbol in &symbols {
                        *counts.entry(symbol.clone()).or_insert(0) += 1;
                    }
                }
                return StreamSubscription {
                    rx,
                    last,
                    guard: StreamGuard {
                        symbols: Arc::clone(&entry.symbols),
                        subscribed: symbols,
                    },
                };
            }
            streams.remove(&kind);
        }

        // Create a fresh producer under the same lock — spawn itself does no
        // awaiting, so there's no race window for a duplicate producer.
        let (tx, rx) = broadcast::channel(CHANNEL_CAPACITY);
        let last: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let counts: Arc<StdMutex<HashMap<String, usize>>> = Arc::new(StdMutex::new(HashMap::new()));
        {
            let mut guard = lock_counts(&counts);
            for symbol in &symbols {
                *guard.entry(symbol.clone()).or_insert(0) += 1;
            }
        }
        let producer = Producer {
            tx: tx.clone(),
            last: Arc::clone(&last),
            fetch: Arc::clone(&spec.fetch),
            symbols: Arc::clone(&counts),
            trading_interval: spec.trading_interval,
            nontrading_interval: spec.nontrading_interval,
            kind: kind.clone(),
        };
        let kind_for_log = kind.clone();
        let handle = tokio::spawn(
            AssertUnwindSafe(producer.run())
                .catch_unwind()
                .map(move |res| Self::log_producer_exit(&kind_for_log, res)),
        );
        streams.insert(
            kind,
            Arc::new(StreamEntry {
                tx,
                last,
                symbols: Arc::clone(&counts),
                handle,
            }),
        );
        StreamSubscription {
            rx,
            last: None,
            guard: StreamGuard {
                symbols: counts,
                subscribed: symbols,
            },
        }
    }

    fn log_producer_exit(key: &str, res: Result<(), Box<dyn std::any::Any + Send>>) {
        match res {
            Ok(()) => openatrading_core::log::info!("[market-stream] producer {key} exited"),
            Err(panic) => {
                openatrading_core::log::error!(
                    "[market-stream] producer {key} panicked: {}",
                    panic_to_string(panic)
                );
            }
        }
    }
}

impl Drop for MarketStreamHub {
    fn drop(&mut self) {
        // Best-effort shutdown: abort any still-running producers so they don't
        // outlive the hub (tokio tasks otherwise keep running on handle drop).
        if let Ok(streams) = self.streams.try_lock() {
            for entry in streams.values() {
                entry.handle.abort();
            }
        }
    }
}

struct Producer {
    tx: broadcast::Sender<String>,
    last: Arc<Mutex<Option<String>>>,
    fetch: FetchFn,
    /// 订阅中的符号并集（引用计数）——每轮现取，订阅者增删立刻反映到下一帧。
    symbols: Arc<StdMutex<HashMap<String, usize>>>,
    trading_interval: Duration,
    nontrading_interval: Duration,
    /// 日志用的生产者标识。
    kind: String,
}

impl Producer {
    async fn run(self) {
        // Initial fetch on connect (even outside trading hours) so subscribers
        // immediately get something to render.
        self.fetch_and_publish().await;

        // The producer stays alive across brief disconnect/reconnect windows
        // (e.g. a tab refresh): it only tears down after [`IDLE_TICKS`]
        // consecutive intervals with zero subscribers. A returning subscriber
        // gets the cached `last` payload immediately, then live ticks resume.
        let mut idle = 0u32;
        loop {
            // Re-evaluate trading status before each sleep so a session
            // boundary (e.g. the 9:30 open) sets the right interval.
            let delay = if crate::cache::is_trading_hours() {
                self.trading_interval
            } else {
                self.nontrading_interval
            };
            tokio::time::sleep(delay).await;

            if self.tx.receiver_count() == 0 {
                idle += 1;
                if idle >= IDLE_TICKS {
                    break;
                }
                continue;
            }
            idle = 0;

            // Re-check AFTER sleeping — the boundary may have crossed during the
            // wait. Outside trading hours we keep the last payload on the client
            // (axum's keep-alive holds the connection) and skip refetching.
            if crate::cache::is_trading_hours() {
                self.fetch_and_publish().await;
            }
        }
    }

    /// 本轮要拉的符号：并集快照，排序后给上游一个稳定的请求（同样的票顺序不同
    /// 也能命中同一条 URL / 同一份响应）。
    fn current_symbols(&self) -> Vec<String> {
        let counts = lock_counts(&self.symbols);
        let mut symbols: Vec<String> = counts.keys().cloned().collect();
        symbols.sort();
        symbols
    }

    async fn fetch_and_publish(&self) {
        let symbols = self.current_symbols();
        if symbols.is_empty() {
            return;
        }
        match (self.fetch)(&symbols).await {
            Ok(json) => {
                // broadcast::send only fails when there are no receivers, which
                // is fine (the idle logic handles teardown).
                let _ = self.tx.send(json.clone());
                *self.last.lock().await = Some(json);
            }
            Err(e) => {
                openatrading_core::log::warn!(
                    "[market-stream] {}: fetch {} symbols failed: {e}",
                    self.kind,
                    symbols.len()
                );
            }
        }
    }
}

fn panic_to_string(p: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = p.downcast_ref::<&'static str>() {
        (*s).to_string()
    } else if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}
