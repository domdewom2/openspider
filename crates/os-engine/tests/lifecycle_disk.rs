//! Speicherplatzwächter (CRAWL-124): Warnung beim Start unter 2 GB, automatische Pause unter 1 GB (Prüfung alle
//! 5 s, in simulierter Zeit), Datei bleibt konsistent und der pausierte Crawl lässt sich fortsetzen.
//!
//! Die Messung ersetzt `disk_guard::set_probe` prozessweit; deshalb steht alles in **einem** Test.

#[path = "lifecycle_support.rs"]
mod support;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use os_engine::disk_guard;
use os_engine::testing::{SimFetcher, SimSite, TokioClock};
use os_engine::{resume_crawl, start_crawl, CrawlRequest, CrawlSource};
use os_model::{CrawlConfig, CrawlState, EndReason};
use os_store::files::{classify_free_space, DiskSpaceStatus};
use support::*;

const GIB: u64 = 1 << 30;

fn fake_free(bytes: Arc<AtomicU64>) -> disk_guard::Probe {
    Arc::new(move |p| {
        let free_bytes = bytes.load(Ordering::SeqCst);
        Ok(DiskSpaceStatus {
            checked: p.to_path_buf(),
            free_bytes,
            level: classify_free_space(free_bytes),
        })
    })
}

// spec: CRAWL-124
#[tokio::test(start_paused = true)]
async fn the_crawl_warns_at_two_gb_pauses_below_one_gb_and_resumes_cleanly() {
    let free = Arc::new(AtomicU64::new(1_500_000_000)); // 1,4 GiB: Warnung, aber noch kein Pausengrund
    disk_guard::set_probe(Some(fake_free(Arc::clone(&free))));

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("crawl.ospider");
    let mut deps = deps();
    deps.clock = TokioClock::new();
    let site = SimSite::tree("site.test", 300).delay(Duration::from_millis(200));
    let fetcher = SimFetcher::new(site);
    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = 2;
    let handle = start_crawl(
        fetcher.clone(),
        CrawlRequest {
            config: cfg.clone(),
            source: CrawlSource::Spider {
                start_url: "https://site.test/".to_string(),
            },
            file: path.clone(),
            project: "disk".to_string(),
            name: "disk".to_string(),
        },
        deps.clone(),
    )
    .await
    .unwrap();

    // Startwarnung als Ereignis in der Crawl-Datei und als Text für die Kommandozeile
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert!(disk_guard::start_warning(&path)
        .unwrap()
        .starts_with("Wenig freier Speicherplatz (1,4 GB)"));
    assert_eq!(
        handle.progress().state,
        CrawlState::Running,
        "bei 1,4 GiB läuft der Crawl weiter"
    );

    // Speicher fällt unter 1 GiB: spätestens nach der nächsten Prüfung (5 s) pausiert der Crawl
    free.store(GIB / 2, Ordering::SeqCst);
    let dropped_at = tokio::time::Instant::now();
    let summary = handle.wait().await.unwrap();
    assert_eq!(summary.state, CrawlState::Paused);
    assert_eq!(summary.end_reason, None);
    let took = dropped_at.elapsed();
    assert!(
        took <= Duration::from_secs(6),
        "Pause nach {took:?} (Prüfintervall 5 s)"
    );

    let file = open_ro(&path);
    assert_eq!(file.meta().unwrap().state, CrawlState::Paused);
    assert_eq!(
        file.get_meta("pause_reason").unwrap().as_deref(),
        Some("disk_space")
    );
    let warned = scalar(
        &file,
        "SELECT COUNT(*) FROM crawl_events WHERE code = 'disk_space_low'",
    );
    assert_eq!(warned, 1, "Startwarnung als Ereignis in der Crawl-Datei");
    let message: String = file
        .conn()
        .query_row(
            "SELECT message FROM crawl_events WHERE code = 'disk_space'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(message.starts_with("Speicherplatz knapp"), "{message}");
    assert!(file.integrity_check().unwrap(), "PRAGMA integrity_check");
    assert!(scalar(&file, "SELECT COUNT(*) FROM queue") > 0);
    drop(file);

    // wieder genug Platz: Fortsetzen läuft bis zum Ende; der Pausengrund ist gelöscht
    free.store(50 * GIB, Ordering::SeqCst);
    let resumed = resume_crawl(fetcher.clone(), path.clone(), None, deps)
        .await
        .unwrap();
    let summary = resumed.wait().await.unwrap();
    assert_eq!(summary.state, CrawlState::Completed);
    assert_eq!(summary.end_reason, Some(EndReason::QueueEmpty));
    let file = open_ro(&path);
    assert_eq!(file.get_meta("pause_reason").unwrap().as_deref(), Some(""));
    assert_eq!(scalar(&file, "SELECT COUNT(*) FROM queue"), 0);
    // jede URL genau einmal angefragt (robots.txt kam aus der Datei)
    let mut urls: Vec<String> = fetcher.requests().into_iter().map(|r| r.url).collect();
    let total = urls.len();
    urls.sort();
    urls.dedup();
    assert_eq!(urls.len(), total, "keine Anfrage doppelt");
    disk_guard::set_probe(None);
}
