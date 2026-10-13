//! Writer-Thread und Gegendruck (M1-Plan 3.7 Schritt 6, PERF-003): ein künstlich langsamer Store drosselt die
//! Worker, der Rückstau bleibt begrenzt, der Crawl läuft weiter und verliert nichts.
//!
//! Läuft in echter Zeit (die Senke wartet als Betriebssystem-Thread), bleibt aber unter wenigen Sekunden.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use os_engine::testing::{test_deps, MemorySink, SimFetcher, SimSite};
use os_engine::writer_thread::{BATCH_PAGES_BACKLOG, CHANNEL_CAPACITY};
use os_engine::{start_crawl, CrawlRequest, CrawlSource, SinkFactory};
use os_model::{CrawlConfig, EndReason, WriteBatch};
use os_store::{CrawlSink, StoreError};

/// Senke, die bei jedem Batch misst, wie weit die Abrufe der geschriebenen Seiten voraus sind.
struct ProbeSink {
    fetcher: Arc<SimFetcher>,
    written: usize,
    max_ahead: Arc<AtomicUsize>,
    delay: Duration,
    inner: MemorySink,
}

impl CrawlSink for ProbeSink {
    fn write(&mut self, batch: WriteBatch) -> Result<(), StoreError> {
        std::thread::sleep(self.delay);
        self.written += batch.pages.len();
        let started = self.fetcher.page_requests().len();
        self.max_ahead
            .fetch_max(started.saturating_sub(self.written), Ordering::SeqCst);
        self.inner.clone().write(batch)
    }

    fn checkpoint(&mut self) -> Result<(), StoreError> {
        Ok(())
    }
}

// Infrastruktur (PERF-003), kein Spec-Tag.
#[tokio::test]
async fn a_slow_store_throttles_the_workers_and_nothing_is_lost() {
    const PAGES: usize = 6_000;
    let fetcher = SimFetcher::new(SimSite::tree("site.test", PAGES));
    let inner = MemorySink::new();
    let max_ahead = Arc::new(AtomicUsize::new(0));
    let factory: SinkFactory = {
        let (fetcher, inner, max_ahead) = (fetcher.clone(), inner.clone(), max_ahead.clone());
        Arc::new(move |_path, _profile| {
            Ok(Box::new(ProbeSink {
                fetcher: fetcher.clone(),
                written: 0,
                max_ahead: max_ahead.clone(),
                delay: Duration::from_millis(100),
                inner: inner.clone(),
            }) as Box<dyn CrawlSink>)
        })
    };
    let mut deps = test_deps();
    deps.sink_factory = Some(factory);
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = 8;
    let handle = start_crawl(
        fetcher.clone(),
        CrawlRequest {
            config: cfg,
            source: CrawlSource::Spider {
                start_url: "https://site.test/".to_string(),
            },
            file: dir.path().join("crawl.ospider"),
            project: "w".to_string(),
            name: "w".to_string(),
        },
        deps,
    )
    .await
    .unwrap();
    let summary = handle.wait().await.unwrap();

    assert_eq!(summary.end_reason, Some(EndReason::QueueEmpty));
    // nichts geht verloren: jede abgerufene Seite wurde geschrieben
    let pages = inner.pages();
    assert!(pages.len() >= PAGES, "{} Seiten geschrieben", pages.len());
    assert_eq!(
        pages.len(),
        fetcher.page_requests().len() + 1,
        "+ robots.txt-Zeile"
    );
    // Rückstau: Kanal (zwei Nachrichten je Seite) + sich füllender Batch + schreibender Batch + Threads
    let bound = CHANNEL_CAPACITY + 2 * BATCH_PAGES_BACKLOG + 100;
    let ahead = max_ahead.load(Ordering::SeqCst);
    assert!(
        ahead < PAGES / 2,
        "die Abrufe liefen {ahead} von {PAGES} Seiten voraus"
    );
    assert!(ahead <= bound, "Rückstau {ahead} über der Grenze {bound}");
}

// Infrastruktur (PERF-003), kein Spec-Tag.
#[tokio::test]
async fn batches_stay_within_the_configured_size_even_for_a_fast_store() {
    let fetcher = SimFetcher::new(SimSite::tree("site.test", 1_000));
    let sink = MemorySink::new();
    let mut deps = test_deps();
    deps.sink_factory = Some(sink.factory());
    let dir = tempfile::tempdir().unwrap();
    let handle = start_crawl(
        fetcher,
        CrawlRequest {
            config: CrawlConfig::default(),
            source: CrawlSource::Spider {
                start_url: "https://site.test/".to_string(),
            },
            file: dir.path().join("crawl.ospider"),
            project: "w".to_string(),
            name: "w".to_string(),
        },
        deps,
    )
    .await
    .unwrap();
    handle.wait().await.unwrap();
    let max = sink.batches().iter().map(|b| b.pages.len()).max().unwrap();
    assert!(
        max <= BATCH_PAGES_BACKLOG + 5,
        "größter Batch: {max} Seiten"
    );
    // Neue URLs stehen immer vor der Seite, die sie verlinkt (der Store braucht die Zielzeilen vorab).
    let mut known = std::collections::HashSet::new();
    for batch in sink.batches() {
        for u in &batch.new_urls {
            known.insert(u.id);
        }
        for p in &batch.pages {
            assert!(
                known.contains(&p.url_id),
                "Seite {} vor ihrer URL-Zeile",
                p.url_id
            );
            for l in &p.links {
                if let Some(t) = l.target_id {
                    assert!(known.contains(&t), "Linkziel {t} noch ohne Zeile");
                }
            }
        }
    }
}

/// Senke, die ab dem n-ten Batch Fehler meldet.
struct FailingSink {
    after: usize,
    seen: usize,
}

impl CrawlSink for FailingSink {
    fn write(&mut self, _batch: WriteBatch) -> Result<(), StoreError> {
        self.seen += 1;
        if self.seen > self.after {
            return Err(StoreError::NotImplemented("Testfehler der Senke"));
        }
        Ok(())
    }

    fn checkpoint(&mut self) -> Result<(), StoreError> {
        Ok(())
    }
}

// Infrastruktur (PERF-003), kein Spec-Tag.
#[tokio::test]
async fn a_failing_store_ends_the_crawl_with_the_store_error_instead_of_hanging() {
    let fetcher = SimFetcher::new(SimSite::tree("site.test", 3_000));
    let mut deps = test_deps();
    let factory: SinkFactory =
        Arc::new(|_p, _prof| Ok(Box::new(FailingSink { after: 1, seen: 0 }) as Box<dyn CrawlSink>));
    deps.sink_factory = Some(factory);
    let dir = tempfile::tempdir().unwrap();
    let handle = start_crawl(
        fetcher.clone(),
        CrawlRequest {
            config: CrawlConfig::default(),
            source: CrawlSource::Spider {
                start_url: "https://site.test/".to_string(),
            },
            file: dir.path().join("crawl.ospider"),
            project: "w".to_string(),
            name: "w".to_string(),
        },
        deps,
    )
    .await;
    // Der Fehler kann schon beim Start auftreten (erster Batch) oder erst im Lauf.
    let outcome = match handle {
        Ok(h) => h.wait().await,
        Err(e) => Err(e),
    };
    match outcome {
        Err(os_engine::EngineError::Store(e)) => {
            assert!(e.to_string().contains("Testfehler"), "{e}")
        }
        other => panic!("erwartet Store-Fehler, war {:?}", other.map(|s| s.state)),
    }
    assert!(
        fetcher.page_requests().len() < 3_000,
        "der Crawl bricht ab, statt die Site weiter abzurufen"
    );
}
