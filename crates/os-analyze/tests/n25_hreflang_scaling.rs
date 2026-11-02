//! M2-N25: Die Zählung des Tabs „Hreflang“ wächst linear mit der Crawl-Größe.
//!
//! Befund: Mit `JOIN` durfte SQLite die Reihenfolge der Beziehungsfilter umdrehen und für jede Quellseite `u` alle
//! gecrawlten URLs `q` über einen Index auf `urls` durchlaufen – quadratisch, auch ohne eine einzige Annotation
//! (8 000 Seiten: 84 s für vier Filter). Die Oberfläche zählt alle Filter bei jedem Datenstand neu (Overview,
//! Filter-Dropdown, Export-Katalog), daher lief das während des ganzen Crawls im Kreis.
//!
//! Gemessen wird die Zahl der SQLite-VM-Schritte (Fortschrittsrückruf), nicht die Zeit: deterministisch und
//! unabhängig von der Maschine.
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

fn file_with_pages(pages: usize, annotated: usize) -> (tempfile::TempDir, CrawlFile, CrawlConfig) {
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
                    'link', 1, 24, 200, 'HTML', 1, 'Indexable' FROM n;
             -- a few pages with one annotation each, pointing to the next page (no return links)
             WITH RECURSIVE n(v) AS (SELECT 1 UNION ALL SELECT v + 1 FROM n WHERE v < {annotated})
             INSERT INTO hreflang (url_id,source,position,code_raw,code_norm,code_valid,href_raw,url,target_id,in_head)
             SELECT v, 'html', 0, 'en', 'en', 1, 'https://site.test/' || (v + 1), 'https://site.test/' || (v + 1),
                    v + 1, 1 FROM n;"
        ))
        .unwrap();
    (dir, file, cfg)
}

/// VM steps (in units of 100 instructions) of counting all filters of the Hreflang tab.
fn steps(pages: usize) -> u64 {
    let (_dir, file, cfg) = file_with_pages(pages, 10);
    let tabs = [os_analyze::tabs::hreflang::tab()];
    TICKS.store(0, Ordering::Relaxed);
    // SAFETY: the handler has no user data and is removed before the connection is dropped.
    unsafe {
        ffi::sqlite3_progress_handler(file.conn().handle(), 100, Some(tick), std::ptr::null_mut())
    };
    let counts = os_analyze::views::filter_counts_in(&tabs, &file, &cfg).unwrap();
    unsafe { ffi::sqlite3_progress_handler(file.conn().handle(), 0, None, std::ptr::null_mut()) };
    // sanity: the count itself is right (every page is in the population, ten carry an annotation)
    let get = |f: &str| counts.iter().find(|c| c.filter == f).and_then(|c| c.count);
    assert_eq!(get("All"), Some(pages as u64));
    assert_eq!(get("Contains Hreflang"), Some(10));
    assert_eq!(get("Missing Return Links"), Some(10));
    TICKS.load(Ordering::Relaxed)
}

// spec: PERF-004
// spec: UI-141
// spec: UI-145
#[test]
fn hreflang_counts_grow_linearly_with_the_crawl() {
    let small = steps(500);
    let large = steps(2000);
    // linear: ×4 pages ≈ ×4 steps; the quadratic plan needed ×16
    assert!(
        large < small * 8,
        "Hreflang-Zählung wächst überlinear: 500 Seiten {small}, 2000 Seiten {large} (×100 VM-Schritte)"
    );
}
