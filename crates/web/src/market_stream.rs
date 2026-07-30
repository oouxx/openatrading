//! Shared market-stream multiplexer.
//!
//! Many SSE clients asking for the same set of symbols share ONE upstream
//! poll. A single producer task per canonical stream key fetches on a fixed
//! interval and publishes the payload to a [`tokio::sync::broadcast`]
//! channel; each SSE connection just subscribes. This replaces the previous
//! design where every connection spawned its own polling task hitting the
//! upstream independently (N connections = N× the upstream rate, risking
//! rate limits) and where each connection bypassed the cache.
//!
//! Lifecycle: the producer exits after [`IDLE_TICKS`] consecutive intervals
//! with no subscribers, so idle streams are reclaimed automatically; a later
//! subscriber lazily re-creates the producer. Producer panics are caught and
//! logged (the spawned task never silently dies).

use std::collections::HashMap;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures::FutureExt;
use tokio::sync::{broadcast, Mutex};
use tokio::task::JoinHandle;

/// A fetcher maps the current symbol set to a JSON payload string (the batch
/// result that gets broadcast). Boxed so the hub stays non-generic.
pub type FetchFn = Arc<
    dyn Fn(&[String]) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send>> + Send + Sync,
>;

/// Configuration for a single shared stream. `key` dedups producers; the same
/// key reuses an already-running producer.
pub struct StreamSpec {
    /// Canonical dedup key (e.g. `realtime:sym1,sym2`).
    pub key: String,
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
    handle: JoinHandle<()>,
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
    /// Returns a broadcast receiver plus the most recent payload (if any). A
    /// fresh subscriber should emit `last` as the first SSE event, then forward
    /// the receiver, skipping the first live message that equals `last` (the
    /// race between reading `last` and subscribing can otherwise duplicate it).
    ///
    /// Lazily spawns a single shared producer per `spec.key` — concurrent
    /// subscribers reuse it instead of spawning more upstream polls. Holds the
    /// hub lock only across cheap, non-network work (channel + spawn), never
    /// across an HTTP fetch.
    ///
    /// # Panics
    /// Never; the producer task catches its own panics and logs them.
    pub async fn subscribe(
        &self,
        spec: StreamSpec,
        symbols: Vec<String>,
    ) -> (broadcast::Receiver<String>, Option<String>) {
        let key = spec.key.clone();
        let mut streams = self.streams.lock().await;

        // Reuse an existing live producer, or evict an idle-exited one.
        if let Some(entry) = streams.get(&key) {
            if !entry.handle.is_finished() {
                if entry.tx.receiver_count() >= MAX_SUBSCRIBERS {
                    openatrading_core::log::warn!(
                        "[market-stream] {key}: subscriber cap ({MAX_SUBSCRIBERS}) reached"
                    );
                }
                let rx = entry.tx.subscribe();
                let last = entry.last.lock().await.clone();
                return (rx, last);
            }
            streams.remove(&key);
        }

        // Create a fresh producer under the same lock — spawn itself does no
        // awaiting, so there's no race window for a duplicate producer.
        let (tx, rx) = broadcast::channel(CHANNEL_CAPACITY);
        let last: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let producer = Producer {
            tx: tx.clone(),
            last: Arc::clone(&last),
            fetch: Arc::clone(&spec.fetch),
            symbols,
            trading_interval: spec.trading_interval,
            nontrading_interval: spec.nontrading_interval,
        };
        let key_for_log = key.clone();
        let handle = tokio::spawn(
            AssertUnwindSafe(producer.run())
                .catch_unwind()
                .map(move |res| Self::log_producer_exit(&key_for_log, res)),
        );
        streams.insert(key, Arc::new(StreamEntry { tx, last, handle }));
        (rx, None)
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
    symbols: Vec<String>,
    trading_interval: Duration,
    nontrading_interval: Duration,
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

    async fn fetch_and_publish(&self) {
        match (self.fetch)(&self.symbols).await {
            Ok(json) => {
                // broadcast::send only fails when there are no receivers, which
                // is fine (the idle logic handles teardown).
                let _ = self.tx.send(json.clone());
                *self.last.lock().await = Some(json);
            }
            Err(e) => {
                openatrading_core::log::warn!(
                    "[market-stream] fetch {:?} failed: {e}",
                    self.symbols
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
