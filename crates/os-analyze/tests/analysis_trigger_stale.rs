//! Veraltung nach „Neu crawlen“ und „Entfernen“ (CRAWL-131, CRAWL-141 Punkt 6): Die Engine markiert die Analyse
//! als veraltet, startet sie aber nicht; `ensure_analysis` holt sie nach.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use os_analyze::analysis::persist::{self, RunState};
use os_analyze::analysis::trigger::{ensure_analysis, run_after_crawl};
use os_analyze::analysis::CancelToken;
use os_engine::testing::{crawl_fixture_with, test_deps};
use os_engine::{recrawl_urls, remove_urls, CrawlSummary};
use os_model::{AnalysisStep, CrawlConfig, UrlId};
use os_store::{CrawlFile, OpenMode};

// spec: CRAWL-131, CRAWL-141
#[tokio::test]
async fn remove_and_recrawl_make_the_analysis_stale_without_starting_it() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut deps = test_deps();
    let c = Arc::clone(&calls);
    deps.on_finished = Some(Arc::new(move |s: &CrawlSummary| {
        c.fetch_add(1, Ordering::SeqCst);
        run_after_crawl(&s.file, &CancelToken::new()).unwrap();
    }));
    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = 2;
    let fx = crawl_fixture_with(
        "crawl-sitemaps",
        "http://site.test/",
        cfg.clone(),
        deps.clone(),
    )
    .await;
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let path = fx.file.path().to_path_buf();
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    assert_eq!(persist::read_run_state(&file).unwrap(), RunState::Completed);
    assert!(persist::read_stale(&file).unwrap().is_empty());
    let a_id: i64 = file
        .conn()
        .query_row(
            "SELECT id FROM urls WHERE url = 'http://site.test/a.html'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    drop(file);

    // Entfernen: veraltet, kein Hook-Aufruf
    remove_urls(&path, &[UrlId(a_id)]).unwrap();
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    assert_eq!(persist::read_run_state(&file).unwrap(), RunState::Stale);
    assert!(persist::read_stale(&file)
        .unwrap()
        .contains(AnalysisStep::Canonicals));
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "keine automatische Analyse nach dem Entfernen"
    );

    // Nachholen
    let outcome = ensure_analysis(&file, &cfg, &CancelToken::new()).unwrap();
    assert!(outcome.complete);
    assert_eq!(persist::read_run_state(&file).unwrap(), RunState::Completed);
    assert!(persist::read_stale(&file).unwrap().is_empty());

    // Neu crawlen: wieder veraltet (die Seite /b.html existiert noch)
    let b_id: i64 = file
        .conn()
        .query_row(
            "SELECT id FROM urls WHERE url = 'http://site.test/b.html'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    drop(file);
    let site = os_testsite::TestSite::start("crawl-sitemaps")
        .await
        .unwrap();
    let fetcher = os_engine::testing::site_fetcher(&site, &cfg, &deps);
    recrawl_urls(fetcher, &path, &[UrlId(b_id)], deps)
        .await
        .unwrap();
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    assert_eq!(persist::read_run_state(&file).unwrap(), RunState::Stale);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "auch Neu crawlen löst keine Analyse aus"
    );
}
