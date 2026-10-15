//! `SlowSink` – künstlich verlangsamter Store für PERF-003 (Gegendruck).
//!
//! Der Wrapper umschließt die echte [`SqliteSink`] und wartet vor jedem Batch (`delay`, Spec: 100 ms). Er wird über
//! `EngineDeps.sink_factory` eingesteckt; die Engine merkt davon nichts außer dem Rückstau im begrenzten Kanal des
//! Writer-Threads, der die Worker drosseln muss.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use os_engine::SinkFactory;
use os_model::{Clock, WriteBatch};
use os_store::{CrawlSink, SqliteSink, StorageProfile, StoreError};

/// Verzögerung je Batch laut PERF-003.
pub const SPEC_DELAY: Duration = Duration::from_millis(100);

/// DIAG-Zähler dieses Kindes (Gruppe `loadtest`): Summe der tatsächlich gewarteten Zeit in Nanosekunden.
static SLOW_WAIT_NS: AtomicU64 = AtomicU64::new(0);
/// DIAG-Zähler: Zahl der verlangsamten Schreibaufrufe.
static SLOW_CALLS: AtomicU64 = AtomicU64::new(0);

/// Name der DIAG-Gruppe der Lasttest-eigenen Zähler (`DIAG loadtest …`, im Bericht `loadtest_<name>`).
pub const DIAG_GROUP: &str = "loadtest";

/// Stand der Zähler der Senke als `(name, wert)` für eine `DIAG`-Zeile: `slow_wait_ns` (Summe der Wartezeit aller
/// Aufrufe, gemessen mit der Uhr, nicht die nominelle Verzögerung) und `slow_calls`. So lässt sich die Wartezeit
/// des künstlich verlangsamten Stores von der Arbeit der Engine trennen (Fable-Gate N14).
pub fn diag_snapshot() -> Vec<(&'static str, u64)> {
    vec![
        ("slow_wait_ns", SLOW_WAIT_NS.load(Ordering::Relaxed)),
        ("slow_calls", SLOW_CALLS.load(Ordering::Relaxed)),
    ]
}

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
        let waited = Instant::now();
        std::thread::sleep(self.delay);
        let ns = u64::try_from(waited.elapsed().as_nanos()).unwrap_or(u64::MAX);
        SLOW_WAIT_NS.fetch_add(ns, Ordering::Relaxed);
        SLOW_CALLS.fetch_add(1, Ordering::Relaxed);
        self.batches.fetch_add(1, Ordering::Relaxed);
        self.inner.write(batch)
    }

    fn checkpoint(&mut self) -> Result<(), StoreError> {
        self.inner.checkpoint()
    }
}

/// Fabrik für `EngineDeps.sink_factory`: `SqliteSink` (mit der Uhr der Engine) hinter einem [`SlowSink`].
pub fn slow_sink_factory(delay: Duration, clock: Arc<dyn Clock>) -> SinkFactory {
    tuned_sink_factory(Some(delay), None, clock)
}

/// Fabrik für `EngineDeps.sink_factory` mit optionaler Verlangsamung (`slow`, PERF-003) und optionalem
/// `PRAGMA wal_autocheckpoint` (Seiten; Messung für den Writer-Engpass, siehe Bericht von M1-P28). Ohne beides
/// entspricht sie dem Standard der Engine.
pub fn tuned_sink_factory(
    slow: Option<Duration>,
    wal_autocheckpoint: Option<u32>,
    clock: Arc<dyn Clock>,
) -> SinkFactory {
    Arc::new(move |path: &Path, profile: StorageProfile| {
        let sqlite = SqliteSink::open(path, profile)?.with_clock(clock.clone());
        if let Some(pages) = wal_autocheckpoint {
            sqlite
                .file()
                .conn()
                .pragma_update(None, "wal_autocheckpoint", pages)?;
        }
        let sink: Box<dyn CrawlSink> = match slow {
            Some(delay) => Box::new(SlowSink::new(Box::new(sqlite), delay)),
            None => Box::new(sqlite),
        };
        Ok(sink)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

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
        let before = diag_snapshot();
        for _ in 0..4 {
            slow.write(WriteBatch::default()).unwrap();
        }
        // DIAG-Zähler: Aufrufe und gewartete Zeit (andere Tests dürfen parallel dazuzählen, daher mindestens)
        let after = diag_snapshot();
        assert!(after[1].1 >= before[1].1 + 4, "{before:?} {after:?}");
        assert!(
            after[0].1 >= before[0].1 + 4 * 30_000_000,
            "{before:?} {after:?}"
        );
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

    // spec: PERF-003
    #[test]
    fn the_factory_wraps_the_real_sqlite_sink_and_applies_the_delay() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f.ospider");
        crate::engine::create_crawl_file(&path, StorageProfile::Balanced).unwrap();
        let clock: Arc<dyn Clock> = Arc::new(os_model::SystemClock);
        let factory =
            tuned_sink_factory(Some(Duration::from_millis(40)), Some(5_000), clock.clone());
        let mut sink = factory(&path, StorageProfile::Balanced).unwrap();
        let t = Instant::now();
        sink.write(WriteBatch::default()).unwrap();
        assert!(
            t.elapsed() >= Duration::from_millis(40),
            "{:?}",
            t.elapsed()
        );
        sink.checkpoint().unwrap();
        drop(sink);
        // ohne Verzögerung und ohne Pragma: der Standard der Engine
        let mut plain =
            tuned_sink_factory(None, None, clock)(&path, StorageProfile::Balanced).unwrap();
        let t = Instant::now();
        plain.write(WriteBatch::default()).unwrap();
        assert!(t.elapsed() < Duration::from_millis(40));
    }

    #[test]
    fn the_spec_delay_is_100_ms() {
        assert_eq!(SPEC_DELAY, Duration::from_millis(100));
    }
}
