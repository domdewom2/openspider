//! M2-N25: `export.catalog` fragt bei jedem Datenstand eines laufenden Crawls, welche Filter auf die Crawl-Analyse
//! warten. Das darf nicht alle Filter zählen: Die volle Zählung lief während des ganzen Crawls im Kreis und hielt
//! einen Kern und eine Leseverbindung belegt. Gemessen wird in SQLite-VM-Schritten (deterministisch).
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
use os_model::{
    time::{IdSource, SeededIds},
    CrawlConfig, CrawlMode, Timestamp,
};
use os_store::{CrawlFile, CrawlInit, StorageProfile};
use rusqlite::ffi;
use std::sync::atomic::{AtomicU64, Ordering};

static TICKS: AtomicU64 = AtomicU64::new(0);

extern "C" fn tick(_: *mut std::ffi::c_void) -> std::ffi::c_int {
    TICKS.fetch_add(1, Ordering::Relaxed);
    0
}

fn file_with_pages(pages: usize) -> (tempfile::TempDir, CrawlFile, CrawlConfig) {
    let dir = tempfile::tempdir().unwrap();
    let cfg = CrawlConfig::default();
    let at = Timestamp::from_db("2026-11-01T12:00:00.000Z").unwrap();
    let init = CrawlInit {
        crawl_id: SeededIds::new(25).new_crawl_id(at),
        mode: CrawlMode::Spider,
        start_url: Some("https://site.test/1".into()),
        list_source: None,
        project: "test".into(),
        name: "n25".into(),
        config: cfg.clone(),
        created_at: at,
    };
    let file = CrawlFile::create(
        &dir.path().join("n25.ospider"),
        &init,
        StorageProfile::Balanced,
    )
    .unwrap();
    file.conn()
        .execute_batch(&format!(
            "WITH RECURSIVE n(v) AS (SELECT 1 UNION ALL SELECT v + 1 FROM n WHERE v < {pages})
             INSERT INTO urls (id,url,address,origin,is_internal,state,discovered_via,folder_depth,url_length,
                               status_code,content_class,effective_index,indexability)
             SELECT v, 'https://site.test/' || v, 'https://site.test/' || v, 'https://site.test', 1, 'done',
                    'link', 1, 24, 200, 'HTML', 1, 'Indexable' FROM n;"
        ))
        .unwrap();
    (dir, file, cfg)
}

/// Result and VM steps (×100 instructions) of `f` on `file`.
fn measured<T>(file: &CrawlFile, f: impl FnOnce() -> T) -> (T, u64) {
    TICKS.store(0, Ordering::Relaxed);
    // SAFETY: the handler has no user data and is removed before the connection is dropped.
    unsafe {
        ffi::sqlite3_progress_handler(file.conn().handle(), 100, Some(tick), std::ptr::null_mut())
    };
    let result = f();
    unsafe { ffi::sqlite3_progress_handler(file.conn().handle(), 0, None, std::ptr::null_mut()) };
    (result, TICKS.load(Ordering::Relaxed))
}

// spec: UI-145
// spec: CRAWL-130
// spec: OUT-112
#[test]
fn export_catalog_prerequisites_need_no_filter_count() {
    let mut steps = Vec::new();
    for pages in [500, 2000] {
        let (_dir, file, cfg) = file_with_pages(pages);
        let (pending, cheap) = measured(&file, || {
            os_export::reports::prereq::pending_analysis_for_file(&file, &cfg).unwrap()
        });
        let (counted, full) = measured(&file, || {
            let counts = os_analyze::views::filter_counts(&file, &cfg).unwrap();
            os_export::reports::prereq::pending_analysis_filters(&counts)
        });
        // same answer as the full count (analysis never ran: every post-crawl filter is pending)
        assert_eq!(pending, counted);
        assert!(!pending.is_empty());
        steps.push((cheap, full));
    }
    let [(small, full_small), (large, full_large)] = steps[..] else {
        unreachable!()
    };
    // independent of the crawl size, and far below one count of all filters
    assert!(
        large <= small + small / 2 + 10,
        "Voraussetzungen wachsen mit dem Crawl: {small} -> {large}"
    );
    assert!(
        large * 10 < full_large && small * 10 < full_small,
        "Voraussetzungen kosten fast eine volle Zählung: {large} vs {full_large}"
    );
}
