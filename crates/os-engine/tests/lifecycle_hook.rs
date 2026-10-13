//! Hook am Crawl-Ende (CRAWL-141, Engine-Seite): `EngineDeps.on_finished` läuft ≤ 1 s nach regulärem Ende,
//! Limit oder Stopp – nach dem abschließenden Checkpoint und vor der Auflösung von `wait()`, nie bei Pause.
//!
//! Die Zeitgrenze prüft ein Test mit kurzen Verzögerungen und der Wanduhr: In simulierter Zeit
//! (`start_paused`) springt die Uhr, während die Engine auf den echten Writer-Thread wartet, bis zum nächsten
//! Zeitgeber (Heartbeat, Speicherprüfung) – das würde die Latenz verfälschen. Die Grenze von 1 s ist gegen die
//! tatsächliche Latenz (Millisekunden) großzügig.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use os_engine::testing::{test_deps, SimFetcher, SimSite, TokioClock};
use os_engine::{start_crawl, CrawlHandle, CrawlRequest, CrawlSource, EngineDeps};
use os_model::{Clock, CrawlConfig, CrawlState, EndReason};
use os_store::{CrawlFile, OpenMode};

struct Probe {
    /// `(Zustand in der Datei, WAL-Größe, simulierte Zeit)` je Aufruf
    calls: Mutex<Vec<(String, u64, Duration)>>,
    /// Hook-Ende vor der Auflösung von `wait()`?
    hook_finished: AtomicBool,
    count: AtomicUsize,
}

fn deps_with_hook(path: std::path::PathBuf, probe: Arc<Probe>) -> EngineDeps {
    let clock = TokioClock::new();
    let mut deps = test_deps();
    deps.clock = clock.clone();
    let c = clock.clone();
    deps.on_finished = Some(Arc::new(move |_summary| {
        let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
        let state = file.meta().unwrap().state.as_str().to_string();
        let mut wal = path.as_os_str().to_owned();
        wal.push("-wal");
        let wal_len = std::fs::metadata(&wal).map_or(0, |m| m.len());
        probe
            .calls
            .lock()
            .unwrap()
            .push((state, wal_len, c.monotonic()));
        probe.count.fetch_add(1, Ordering::SeqCst);
        // ein langsamer Hook: `wait()` darf erst danach auflösen
        std::thread::sleep(Duration::from_millis(100));
        probe.hook_finished.store(true, Ordering::SeqCst);
    }));
    deps
}

async fn start(
    dir: &tempfile::TempDir,
    pages: usize,
    cfg: CrawlConfig,
    deps: EngineDeps,
    delay_ms: u64,
) -> (Arc<SimFetcher>, CrawlHandle) {
    let fetcher =
        SimFetcher::new(SimSite::tree("site.test", pages).delay(Duration::from_millis(delay_ms)));
    let handle = start_crawl(
        fetcher.clone(),
        CrawlRequest {
            config: cfg,
            source: CrawlSource::Spider {
                start_url: "https://site.test/".to_string(),
            },
            file: dir.path().join("crawl.ospider"),
            project: "hook".to_string(),
            name: "hook".to_string(),
        },
        deps,
    )
    .await
    .unwrap();
    (fetcher, handle)
}

fn probe() -> Arc<Probe> {
    Arc::new(Probe {
        calls: Mutex::new(Vec::new()),
        hook_finished: AtomicBool::new(false),
        count: AtomicUsize::new(0),
    })
}

fn threads(n: u32) -> CrawlConfig {
    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = n;
    cfg
}

// spec: CRAWL-141
#[tokio::test]
async fn the_hook_runs_within_a_second_of_the_regular_end_after_the_checkpoint_and_before_wait_returns(
) {
    let dir = tempfile::tempdir().unwrap();
    let p = probe();
    let deps = deps_with_hook(dir.path().join("crawl.ospider"), p.clone());
    let (fetcher, handle) = start(&dir, 30, threads(3), deps, 5).await;
    let summary = handle.wait().await.unwrap();
    assert_eq!(summary.end_reason, Some(EndReason::QueueEmpty));
    assert!(
        p.hook_finished.load(Ordering::SeqCst),
        "wait() löst erst nach dem Hook auf"
    );
    let calls = p.calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 1);
    let (state, wal_len, at) = &calls[0];
    assert_eq!(
        state, "Completed",
        "Zustand steht beim Hook schon in der Datei"
    );
    assert_eq!(*wal_len, 0, "abschließender Checkpoint ist gelaufen");
    let last_end = fetcher.requests().iter().map(|r| r.end).max().unwrap();
    assert!(
        *at <= last_end + Duration::from_secs(1),
        "Hook {at:?}, letzter Abruf endete {last_end:?}"
    );
}

// spec: CRAWL-141
#[tokio::test(start_paused = true)]
async fn the_hook_also_runs_after_the_url_limit_and_after_a_stop() {
    for (label, limit, stop) in [("Limit", 5u64, false), ("Stopp", 0, true)] {
        let dir = tempfile::tempdir().unwrap();
        let p = probe();
        let deps = deps_with_hook(dir.path().join("crawl.ospider"), p.clone());
        let mut cfg = threads(2);
        if limit > 0 {
            cfg.limits.max_urls = limit;
        }
        let (_fetcher, handle) = start(&dir, 100, cfg, deps, 100).await;
        if stop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            handle.stop();
        }
        let summary = handle.wait().await.unwrap();
        assert_eq!(summary.state, CrawlState::Completed, "{label}");
        assert_eq!(
            p.count.load(Ordering::SeqCst),
            1,
            "{label}: Hook genau einmal"
        );
        assert_eq!(p.calls.lock().unwrap()[0].0, "Completed", "{label}");
    }
}

// spec: CRAWL-141
#[tokio::test(start_paused = true)]
async fn a_pause_never_calls_the_hook_and_the_resumed_crawl_calls_it_once_at_the_end() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("crawl.ospider");
    let p = probe();
    let deps = deps_with_hook(path.clone(), p.clone());
    let (fetcher, handle) = start(&dir, 60, threads(2), deps.clone(), 100).await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    handle.pause();
    let paused = handle.wait().await.unwrap();
    assert_eq!(paused.state, CrawlState::Paused);
    assert_eq!(
        p.count.load(Ordering::SeqCst),
        0,
        "keine automatische Analyse bei Pause"
    );

    let resumed = os_engine::resume_crawl(fetcher, path, None, deps)
        .await
        .unwrap()
        .wait()
        .await
        .unwrap();
    assert_eq!(resumed.end_reason, Some(EndReason::QueueEmpty));
    assert_eq!(
        p.count.load(Ordering::SeqCst),
        1,
        "erst nach Fortsetzen und regulärem Ende"
    );
}
