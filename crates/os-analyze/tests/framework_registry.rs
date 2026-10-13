//! Eigenschaftstest über alle registrierten Tabs (`tabs::all()`): F ⊆ All, Zähler = Zeilenzahl, Zustand statt
//! Zahl. Läuft später automatisch für die echten Tab-Definitionen von P20–P25 mit.

#[path = "framework_common.rs"]
mod common;

use common::*;
use os_analyze::tabs;
use os_analyze::views::{filter_counts, query_view, stream_view, PageStart, ViewQuery};
use os_model::{CrawlConfig, FilterState};

fn populated() -> (tempfile::TempDir, os_store::CrawlFile) {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = create(dir.path(), "registry.ospider", 9);
    let mut rows = Vec::new();
    for i in 1..=12 {
        let mut r = row(i, &format!("seite-{i:02}.html"))
            .int("status_code", if i % 5 == 0 { 404 } else { 200 })
            .text("content_class", if i % 3 == 0 { "PDF" } else { "HTML" })
            .text("title_1", &format!("Titel {}", i % 4))
            .int("title_1_len", i * 9)
            .int("title_count", 1)
            .int("canonical_count", i64::from(i % 2 == 0));
        if i > 9 {
            r = r.external();
        }
        rows.push(r);
    }
    insert(&file, &rows);
    (dir, file)
}

fn all_ids(file: &os_store::CrawlFile, cfg: &CrawlConfig, tab: &str, filter: &str) -> Vec<i64> {
    let mut out = Vec::new();
    let mut start = PageStart::Offset(0);
    loop {
        let mut q = ViewQuery::new(tab, filter);
        q.limit = 5;
        q.start = start;
        let page = query_view(file, cfg, &q).unwrap();
        out.extend(page.row_keys.iter().map(|k| k.0));
        match page.next_cursor {
            Some(c) => start = PageStart::Cursor(c),
            None => return out,
        }
    }
}

// spec: ONPAGE-001, ONPAGE-190
#[test]
fn every_filter_is_a_subset_of_all_and_counts_match_the_views() {
    let (_dir, file) = populated();
    let cfg = CrawlConfig::default();
    let counts = filter_counts(&file, &cfg).unwrap();
    let registered: usize = tabs::all().iter().map(|t| t.filters.len()).sum();
    assert_eq!(
        counts.len(),
        registered,
        "ein Zähler je registriertem Filter"
    );

    for tab in tabs::all() {
        let all = all_ids(&file, &cfg, tab.id, "All");
        for f in &tab.filters {
            let c = counts
                .iter()
                .find(|c| c.tab == tab.id && c.filter == f.id)
                .unwrap();
            let ids = all_ids(&file, &cfg, tab.id, f.id);
            for id in &ids {
                assert!(
                    all.contains(id),
                    "{}:{} enthält URL {id} außerhalb von All",
                    tab.id,
                    f.id
                );
            }
            match c.state {
                FilterState::Ready => {
                    assert_eq!(c.count, Some(ids.len() as u64), "{}:{}", tab.id, f.id)
                }
                _ => {
                    assert_eq!(c.count, None, "{}:{}: Zustand statt Zahl", tab.id, f.id);
                    assert!(
                        ids.is_empty(),
                        "{}:{}: keine Zeilen ohne Zustand Ready",
                        tab.id,
                        f.id
                    );
                }
            }
            // Streamen liefert dieselbe Zeilenzahl.
            let mut n = 0u64;
            stream_view(&file, &cfg, &ViewQuery::new(tab.id, f.id), &mut |_| {
                n += 1;
                Ok(())
            })
            .unwrap();
            assert_eq!(n, ids.len() as u64, "{}:{}", tab.id, f.id);
        }
        // Grundmenge: Platzhalter-Tabs zählen alle Zeilen (Population „1“); echte Tabs höchstens alle.
        assert!(all.len() <= 12, "{}", tab.id);
    }
}

// spec: ONPAGE-190
#[test]
fn overview_labels_use_the_current_thresholds_and_states_are_complete() {
    let (_dir, file) = populated();
    let mut cfg = CrawlConfig::default();
    cfg.title.max_chars = 55;
    let counts = filter_counts(&file, &cfg).unwrap();
    let c = counts
        .iter()
        .find(|c| c.tab == "Page Titles" && c.filter == "Over {title.max_chars} Characters")
        .unwrap();
    assert_eq!(c.label, "Over 55 Characters");
    // Nachlauf-Filter ohne Analyse zeigen den Zustand statt einer Zahl.
    let pending: Vec<_> = counts
        .iter()
        .filter(|c| c.state == FilterState::NeedsCrawlAnalysis)
        .collect();
    assert!(
        !pending.is_empty(),
        "es gibt Nachlauf-Filter (Sitemaps, Canonicals …)"
    );
    assert!(pending.iter().all(|c| c.count.is_none() && c.pct.is_none()));
    let all_filters = counts.iter().filter(|c| c.filter == "All");
    for c in all_filters {
        assert_eq!(
            c.state,
            FilterState::Ready,
            "{}:All ist immer bereit",
            c.tab
        );
        assert_eq!(
            c.pct.map(|p| p == 100.0 || p == 0.0),
            Some(true),
            "{}",
            c.tab
        );
    }
}
