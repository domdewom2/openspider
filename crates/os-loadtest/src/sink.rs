//! `SlowSink` – künstlich verlangsamter Store für PERF-003 (Gegendruck).
//!
//! Der Wrapper umschließt die echte [`SqliteSink`] und wartet vor jedem Batch (`delay`, Spec: 100 ms). Er wird über
//! `EngineDeps.sink_factory` eingesteckt; die Engine merkt davon nichts außer dem Rückstau im begrenzten Kanal des
//! Writer-Threads, der die Worker drosseln muss.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use os_engine::SinkFactory;
use os_model::{Clock, WriteBatch};
use os_store::{CrawlSink, SqliteSink, StorageProfile, StoreError};

/// Verzögerung je Batch laut PERF-003.
pub const SPEC_DELAY: Duration = Duration::from_millis(100);

/// Senke, die jeden Batch um `delay` verzögert und dann an die innere Senke weiterreicht.
pub struct SlowSink {
    inner: Box<dyn CrawlSink>,
    delay: Duration,
    batches: Arc<AtomicU64>,
}

impl SlowSink {
    /// Umschließt `inner`.
    pub fn new(inner: Box<dyn CrawlSink>, delay: Duration) -> Self {
        Self {
            inner,
            delay,
            batches: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Zähler der geschriebenen Batches (geteilt, auch nach dem Verschieben der Senke lesbar).
    pub fn batch_counter(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.batches)
    }
}

impl CrawlSink for SlowSink {
    fn write(&mut self, batch: WriteBatch) -> Result<(), StoreError> {
        // Der Writer-Thread ist ein Betriebssystem-Thread (kein Tokio-Worker): Schlafen blockiert nur ihn.
        std::thread::sleep(self.delay);
        self.batches.fetch_add(1, Ordering::Relaxed);
        self.inner.write(batch)
    }

    fn checkpoint(&mut self) -> Result<(), StoreError> {
        self.inner.checkpoint()
    }
}

/// Fabrik für `EngineDeps.sink_factory`: `SqliteSink` (mit der Uhr der Engine) hinter einem [`SlowSink`].
pub fn slow_sink_factory(delay: Duration, clock: Arc<dyn Clock>) -> SinkFactory {
    Arc::new(move |path: &Path, profile: StorageProfile| {
        let sqlite = SqliteSink::open(path, profile)?.with_clock(clock.clone());
        Ok(Box::new(SlowSink::new(Box::new(sqlite), delay)) as Box<dyn CrawlSink>)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::time::Instant;

    #[derive(Default)]
    struct Count(Arc<Mutex<usize>>);

    impl CrawlSink for Count {
        fn write(&mut self, _batch: WriteBatch) -> Result<(), StoreError> {
            *self.0.lock().unwrap() += 1;
            Ok(())
        }

        fn checkpoint(&mut self) -> Result<(), StoreError> {
            Ok(())
        }
    }

    // spec: PERF-003
    #[test]
    fn every_batch_takes_at_least_the_delay_and_reaches_the_inner_sink() {
        let inner = Count::default();
        let seen = Arc::clone(&inner.0);
        let mut slow = SlowSink::new(Box::new(inner), Duration::from_millis(30));
        let counter = slow.batch_counter();
        let started = Instant::now();
        for _ in 0..4 {
            slow.write(WriteBatch::default()).unwrap();
        }
        assert!(
            started.elapsed() >= Duration::from_millis(120),
            "{:?}",
            started.elapsed()
        );
        assert_eq!(*seen.lock().unwrap(), 4);
        assert_eq!(counter.load(Ordering::Relaxed), 4);
        // Checkpoints sind nicht verlangsamt
        let t = Instant::now();
        slow.checkpoint().unwrap();
        assert!(t.elapsed() < Duration::from_millis(30));
    }

    #[test]
    fn the_spec_delay_is_100_ms() {
        assert_eq!(SPEC_DELAY, Duration::from_millis(100));
    }
}
