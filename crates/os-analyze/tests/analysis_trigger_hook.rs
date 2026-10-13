//! Auslösung der Crawl-Analyse (CRAWL-141): Hook der Engine = `run_after_crawl`, Schalter `analysis.auto_at_end`,
//! keine Analyse bei Pause, manueller Start während der Pause → „unvollständig“, `ensure_analysis` für die CLI.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use os_analyze::analysis::persist::{self, RunState};
use os_analyze::analysis::trigger::{ensure_analysis, run_after_crawl, run_manual};
use os_analyze::analysis::CancelToken;
use os_analyze::state::AnalysisSnapshot;
use os_engine::testing::{crawl_fixture_with, test_deps, SimFetcher, SimSite};
use os_engine::{resume_crawl, start_crawl, CrawlRequest, CrawlSource, CrawlSummary, EngineDeps};
use os_model::{AnalysisStep, CrawlConfig, CrawlState, StepState};
use os_store::{CrawlFile, OpenMode};

/// Hook = `run_after_crawl` (so setzt ihn `os-cli` ein); zählt die Aufrufe.
fn analysis_deps() -> (EngineDeps, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = Arc::clone(&calls);
    let mut deps = test_deps();
    deps.on_finished = Some(Arc::new(move |s: &CrawlSummary| {
        c.fetch_add(1, Ordering::SeqCst);
        run_after_crawl(&s.file, &CancelToken::new()).expect("Analyse läuft");
    }));
    (deps, calls)
}

fn sitemap_crawl_config() -> CrawlConfig {
    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = 2;
    cfg.sitemaps.crawl_linked = true;
    cfg.sitemaps.urls = vec!["http://site.test/pages.xml".to_string()];
    cfg
}

fn run_state(file: &CrawlFile) -> RunState {
    persist::read_run_state(file).unwrap()
}

fn open(path: &Path) -> CrawlFile {
    CrawlFile::open(path, OpenMode::ReadWrite).unwrap()
}

// spec: CRAWL-141
#[tokio::test]
async fn the_analysis_starts_by_itself_when_the_crawl_ends_regularly() {
    let (deps, calls) = analysis_deps();
    let fx = crawl_fixture_with(
        "crawl-sitemaps",
        "http://site.test/",
        sitemap_crawl_config(),
        deps,
    )
    .await;
    assert_eq!(calls.load(Ordering::SeqCst), 1, "Hook genau einmal");
    assert_eq!(fx.summary.state, CrawlState::Completed);
    let file = open(fx.file.path());
    assert_eq!(run_state(&file), RunState::Completed);
    assert!(file
        .get_meta("analysis_at")
        .unwrap()
        .is_some_and(|v| !v.is_empty()));
    // alle zwölf Teilschritte sind entschieden (fertig, übersprungen oder nicht anwendbar), keiner steht aus
    let snap = AnalysisSnapshot::load(&file).unwrap();
    for step in AnalysisStep::ALL.iter().copied() {
        let rec = snap
            .record(step)
            .unwrap_or_else(|| panic!("{step:?} fehlt"));
        assert_ne!(rec.state, StepState::Pending, "{step:?}");
    }
}

// spec: CRAWL-141
#[tokio::test]
async fn with_the_switch_off_no_analysis_runs_and_ensure_analysis_runs_it_on_demand() {
    let (deps, calls) = analysis_deps();
    let mut cfg = sitemap_crawl_config();
    cfg.analysis.auto_at_end = false;
    let fx = crawl_fixture_with("crawl-sitemaps", "http://site.test/", cfg.clone(), deps).await;
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "die Engine ruft den Hook; er selbst prüft den Schalter"
    );
    let file = open(fx.file.path());
    assert_eq!(run_state(&file), RunState::NotStarted);
    assert!(AnalysisSnapshot::load(&file)
        .unwrap()
        .record(AnalysisStep::Sitemaps)
        .is_none());
    // run_after_crawl liefert ausdrücklich nichts
    assert!(run_after_crawl(fx.file.path(), &CancelToken::new())
        .unwrap()
        .is_none());

    // CLI: vor analyseabhängigen Exporten läuft die Analyse trotzdem
    let outcome = ensure_analysis(&file, &cfg, &CancelToken::new()).unwrap();
    assert!(outcome.complete);
    assert_eq!(run_state(&file), RunState::Completed);

    // ein zweiter Aufruf findet alles aktuell und tut nichts
    let at = file.get_meta("analysis_at").unwrap();
    let again = ensure_analysis(&file, &cfg, &CancelToken::new()).unwrap();
    assert!(again.complete);
    assert_eq!(
        file.get_meta("analysis_at").unwrap(),
        at,
        "kein erneuter Lauf"
    );
    assert_eq!(again.steps.len(), AnalysisStep::ALL.len());

    // veraltet markierte Schritte führt ensure_analysis erneut aus
    persist::mark_stale(&file, &[AnalysisStep::Canonicals]).unwrap();
    assert_eq!(run_state(&file), RunState::Stale);
    let after = ensure_analysis(&file, &cfg, &CancelToken::new()).unwrap();
    assert!(after.complete);
    assert_eq!(run_state(&file), RunState::Completed);
    assert!(persist::read_stale(&file).unwrap().is_empty());
}

// spec: CRAWL-141
#[tokio::test]
async fn a_pause_starts_no_analysis_a_manual_start_leaves_it_incomplete_and_the_end_after_resume_runs_it(
) {
    let (deps, calls) = analysis_deps();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("crawl.ospider");
    let fetcher = SimFetcher::new(SimSite::tree("site.test", 60).delay(Duration::from_millis(20)));
    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = 1;
    let handle = start_crawl(
        fetcher.clone(),
        CrawlRequest {
            config: cfg.clone(),
            source: CrawlSource::Spider {
                start_url: "https://site.test/".to_string(),
            },
            file: path.clone(),
            project: "analyse".to_string(),
            name: "analyse".to_string(),
        },
        deps.clone(),
    )
    .await
    .unwrap();
    while handle.progress().crawled < 10 {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    handle.pause();
    let paused = handle.wait().await.unwrap();
    assert_eq!(paused.state, CrawlState::Paused);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "Pause: keine automatische Analyse"
    );
    let file = open(&path);
    assert_eq!(run_state(&file), RunState::NotStarted);

    // manueller Start während der Pause: möglich, aber unvollständig
    let outcome = run_manual(&file, &cfg, &CancelToken::new(), &|_, _| {}).unwrap();
    assert!(outcome.complete, "alle Schritte sind entschieden …");
    assert_eq!(
        run_state(&file),
        RunState::Incomplete,
        "… das Ergebnis gilt wegen der Pause aber als unvollständig"
    );
    drop(file);

    // Fortsetzen bis zum regulären Ende: jetzt läuft der Hook und die Analyse ist abgeschlossen
    let resumed = resume_crawl(fetcher, path.clone(), None, deps)
        .await
        .unwrap()
        .wait()
        .await
        .unwrap();
    assert_eq!(resumed.state, CrawlState::Completed);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(run_state(&open(&path)), RunState::Completed);
}
