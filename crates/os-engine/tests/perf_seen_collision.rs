//! Seen-Menge mit erzwungenen Schlüsselkollisionen (K-78 a, PERF-001): Über eine injizierbare Hashfunktion fallen
//! viele URLs auf denselben 64-Bit-Schlüssel (der 32-Bit-Fingerabdruck bleibt verschieden). Die Kollisionen landen
//! in der Überlaufmenge mit der vollen Adresse: Jede URL wird entdeckt und abgerufen, keine doppelt, und ein
//! pausierter und fortgesetzter Crawl ergibt dasselbe Ergebnis wie ein ungestörter.
//!
//! Die Hashfunktion ist prozessweit überschreibbar (`seen::set_key_fn_override`); die Tests dieser Datei laufen
//! deshalb nacheinander.

#[path = "lifecycle_support.rs"]
mod support;

use std::collections::BTreeMap;

use os_engine::seen::{default_key_fp, set_key_fn_override, KeyFp};
use os_model::{CrawlState, EndReason};
use os_testsite::TestSite;
use support::*;

static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Nur 32 verschiedene Schlüssel für alle URLs; die Fingerabdrücke bleiben verschieden.
fn few_keys(encoded: &str) -> KeyFp {
    let (key, fp) = default_key_fp(encoded);
    (key & 0x1F, fp)
}

fn counters() -> BTreeMap<&'static str, u64> {
    os_engine::diag::snapshot().into_iter().collect()
}

async fn crawl_site_100(
    path: &std::path::Path,
) -> (
    os_testsite::TestSite,
    os_engine::CrawlSummary,
    tempfile::TempDir,
) {
    let dir = slow_site_dir(0);
    let site = start_site(dir.path()).await;
    let summary = run_to_end(start(config_for(&site, 1), path, deps()).await).await;
    (site, summary, dir)
}

// spec: PERF-001
#[tokio::test]
async fn urls_with_colliding_keys_are_all_discovered_and_fetched_exactly_once() {
    let _serial = SERIAL.lock().await;
    let ref_work = tempfile::tempdir().unwrap();
    let ref_path = crawl_file_in(&ref_work, "ref.ospider");
    let (ref_site, reference, _ref_dir) = crawl_site_100(&ref_path).await;
    assert_eq!(reference.end_reason, Some(EndReason::QueueEmpty));
    assert!(reference.crawled >= 100, "{}", reference.crawled);

    let before = counters();
    set_key_fn_override(Some(few_keys));
    let work = tempfile::tempdir().unwrap();
    let path = crawl_file_in(&work, "crawl.ospider");
    let (site, summary, _dir) = crawl_site_100(&path).await;
    set_key_fn_override(None);
    let after = counters();

    assert_eq!(summary.end_reason, Some(EndReason::QueueEmpty));
    assert_eq!(
        summary.crawled, reference.crawled,
        "jede URL wurde abgerufen"
    );
    assert!(
        after["seen_collisions"] > before["seen_collisions"] + 50,
        "Kollisionen: {} -> {}",
        before["seen_collisions"],
        after["seen_collisions"]
    );
    assert!(after["mem_seen_overflow"] > 50, "{after:?}");
    // dieselbe Datei wie ohne Kollisionen (Kennungen, Zustände, Status, Tiefen) und kein Abruf doppelt
    assert_eq!(snapshot(&path), snapshot(&ref_path));
    assert_eq!(duplicate_requests(&site), vec![]);
    assert_eq!(site.requests().len(), ref_site.requests().len());
}

// spec: CRAWL-126
#[tokio::test]
async fn a_resumed_crawl_with_colliding_keys_equals_the_uninterrupted_one() {
    let _serial = SERIAL.lock().await;
    let ref_work = tempfile::tempdir().unwrap();
    let ref_path = crawl_file_in(&ref_work, "ref.ospider");
    let (ref_site, reference, _ref_dir) = crawl_site_100(&ref_path).await;

    set_key_fn_override(Some(few_keys));
    let dir = slow_site_dir(2);
    let site: TestSite = start_site(dir.path()).await;
    let work = tempfile::tempdir().unwrap();
    let path = crawl_file_in(&work, "crawl.ospider");
    let handle = start(config_for(&site, 1), &path, deps()).await;
    until_crawled(&handle, 30).await;
    handle.pause();
    let paused = run_to_end(handle).await;
    assert_eq!(paused.state, CrawlState::Paused);
    let cfg = resume_config(&path, &site, |_| {});
    let resumed = run_to_end(resume(cfg, &path, deps()).await).await;
    set_key_fn_override(None);

    assert_eq!(resumed.state, CrawlState::Completed);
    assert_eq!(resumed.crawled, reference.crawled);
    assert_eq!(snapshot(&path), snapshot(&ref_path));
    assert_eq!(duplicate_requests(&site), vec![]);
    assert_eq!(site.requests().len(), ref_site.requests().len());
}
