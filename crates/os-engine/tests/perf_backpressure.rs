//! Gegendruck mit verlangsamtem Store (PERF-003, M1-P28): Ein `SlowSink` (100 ms je Batch, Wrapper um die echte
//! `SqliteSink`) wird über `EngineDeps.sink_factory` eingesteckt. Der Crawl läuft weiter, endet mit demselben
//! Ergebnis wie ohne Verlangsamung und verliert nichts; der Speicher des Kindprozesses unter 500 MB misst
//! `os-loadtest` (Szenario `perf-003`, Spitzen-RSS von außen).

#[path = "lifecycle_support.rs"]
mod support;

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use os_engine::SinkFactory;
use os_model::{CrawlState, EndReason, WriteBatch};
use os_store::{CrawlSink, SqliteSink, StorageProfile, StoreError};
use os_testsite::{SiteOptions, SyntheticConfig, TestSite};
use support::*;

const DELAY: Duration = Duration::from_millis(100);
const THREADS: u32 = 8;

struct SlowSink {
    inner: SqliteSink,
    batches: Arc<AtomicU64>,
}

impl CrawlSink for SlowSink {
    fn write(&mut self, batch: WriteBatch) -> Result<(), StoreError> {
        std::thread::sleep(DELAY);
        self.batches.fetch_add(1, Ordering::Relaxed);
        self.inner.write(batch)
    }

    fn checkpoint(&mut self) -> Result<(), StoreError> {
        self.inner.checkpoint()
    }
}

fn slow_factory(batches: Arc<AtomicU64>) -> SinkFactory {
    Arc::new(move |path: &Path, profile: StorageProfile| {
        Ok(Box::new(SlowSink {
            inner: SqliteSink::open(path, profile)?,
            batches: batches.clone(),
        }) as Box<dyn CrawlSink>)
    })
}

fn synthetic() -> SyntheticConfig {
    SyntheticConfig {
        pages: 1_500,
        links_per_page: 10,
        seed: 7,
        status_404_share: 0.02,
        ..SyntheticConfig::default()
    }
}

async fn site() -> TestSite {
    TestSite::start_synthetic(synthetic(), SiteOptions::default())
        .await
        .unwrap()
}

// spec: PERF-003
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_slow_store_throttles_the_crawl_but_it_continues_loses_nothing_and_ends() {
    // Referenz ohne Verlangsamung
    let ref_site = site().await;
    let ref_work = tempfile::tempdir().unwrap();
    let ref_path = crawl_file_in(&ref_work, "ref.ospider");
    let reference =
        run_to_end(start(config_for(&ref_site, THREADS), &ref_path, deps()).await).await;
    assert_eq!(reference.end_reason, Some(EndReason::QueueEmpty));
    assert!(reference.crawled >= 1_400, "{}", reference.crawled);

    // Mit SlowSink (100 ms je Batch)
    let site = site().await;
    let work = tempfile::tempdir().unwrap();
    let path = crawl_file_in(&work, "slow.ospider");
    let batches = Arc::new(AtomicU64::new(0));
    let mut d = deps();
    d.sink_factory = Some(slow_factory(batches.clone()));
    let started = Instant::now();
    let handle = start(config_for(&site, THREADS), &path, d).await;

    // Der Crawl kommt laufend voran (kein Stillstand, kein Rückgang), solange der Store langsam ist.
    let mut last = 0;
    let mut stalled_since = Instant::now();
    let mut max_stall = Duration::ZERO;
    while !matches!(
        handle.progress().state,
        CrawlState::Completed | CrawlState::Failed | CrawlState::Paused
    ) {
        let crawled = handle.progress().crawled;
        assert!(crawled >= last, "crawled sinkt: {last} -> {crawled}");
        if crawled > last {
            max_stall = max_stall.max(stalled_since.elapsed());
            stalled_since = Instant::now();
            last = crawled;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let summary = run_to_end(handle).await;
    let elapsed = started.elapsed();

    assert_eq!(summary.state, CrawlState::Completed);
    assert_eq!(summary.end_reason, Some(EndReason::QueueEmpty));
    assert_eq!(summary.crawled, reference.crawled, "nichts geht verloren");
    assert_eq!(
        statuses(&path),
        statuses(&ref_path),
        "URL-Menge und Status je URL wie ohne Verlangsamung"
    );
    let n = batches.load(Ordering::Relaxed);
    assert!(n >= 2, "{n} Batches");
    assert!(
        elapsed >= DELAY * u32::try_from(n).unwrap().saturating_sub(1),
        "der Store war langsam: {n} Batches in {elapsed:?}"
    );
    assert!(
        max_stall < Duration::from_secs(5),
        "der Crawl stand {max_stall:?} still"
    );
    eprintln!(
        "perf_backpressure: {} URLs in {elapsed:?}, {n} Batches à 100 ms, längster Stillstand {max_stall:?}",
        summary.crawled
    );
}
