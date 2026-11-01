//! M2-N26: Overview/Issues count every filter of every tab once per data version. The relationship
//! filters of the Hreflang tab ran their correlated subquery from `urls q` (index `urls_state` or
//! `urls_status`) instead of from the page's annotations, so each population row scanned all crawled
//! URLs: 8.5 s for the Hreflang tab alone on a finished crawl with 5 640 URLs and no hreflang at all,
//! 93 % of the whole Overview/Issues count (`docs/plan/M2-reports/n26.md`).

use std::time::{Duration, Instant};

use os_analyze::tabs;
use os_analyze::views::{filter_counts_in, plan, ViewQuery, URLS_FROM};
use os_model::time::{IdSource, SeededIds};
use os_model::{CrawlConfig, CrawlMode, Timestamp, Value};
use os_store::{CrawlFile, CrawlInit, StorageProfile};

const RELATIONSHIP_FILTERS: [&str; 6] = [
    "Non-200 Hreflang URLs",
    "Missing Return Links",
    "Unlinked Hreflang URLs",
    "Inconsistent Language & Region Return Links",
    "Non-Canonical Return Links",
    "Noindex Return Links",
];

fn file(dir: &std::path::Path) -> CrawlFile {
    let at = Timestamp::from_db("2026-11-01T12:00:00.000Z").unwrap();
    let init = CrawlInit {
        crawl_id: SeededIds::new(7).new_crawl_id(at),
        mode: CrawlMode::Spider,
        start_url: Some("https://site.test/".into()),
        list_source: None,
        project: "Projekt".into(),
        name: "N26".into(),
        config: CrawlConfig::default(),
        created_at: at,
    };
    CrawlFile::create(&dir.join("n26.ospider"), &init, StorageProfile::Balanced).unwrap()
}

/// `pages` crawled internal URLs, alternately HTML pages and images (status 200, indexable).
fn crawl(file: &CrawlFile, pages: i64) {
    let tx = file.conn().unchecked_transaction().unwrap();
    {
        let mut stmt = tx
            .prepare(
                "INSERT INTO urls (id, url, address, origin, is_internal, state, discovered_via, folder_depth, \
                 url_length, status_code, content_class, effective_index, canonical_count, canonical_self) \
                 VALUES (?1, ?2, ?2, 'https://site.test', 1, 'done', 'link', 1, 30, 200, ?3, 1, 1, 1)",
            )
            .unwrap();
        for id in 1..=pages {
            let class = if id % 2 == 1 { "HTML" } else { "Image" };
            stmt.execute(rusqlite::params![
                id,
                format!("https://site.test/{id}"),
                class
            ])
            .unwrap();
        }
    }
    tx.commit().unwrap();
}

/// `EXPLAIN QUERY PLAN` of population and filter as counted for Overview/Issues.
fn explain(file: &CrawlFile, cfg: &CrawlConfig, filter: &str) -> Vec<String> {
    let p = plan(&ViewQuery::new("Hreflang", filter), cfg).unwrap();
    let sql = format!(
        "EXPLAIN QUERY PLAN SELECT COUNT(*) FROM {URLS_FROM} WHERE ({}) AND ({})",
        p.population_sql, p.filter_sql
    );
    let mut stmt = file.conn().prepare(&sql).unwrap();
    for (name, value) in &p.params {
        if let Some(idx) = stmt.parameter_index(name).unwrap() {
            match value {
                Value::Int(i) => stmt.raw_bind_parameter(idx, *i).unwrap(),
                Value::Bool(b) => stmt.raw_bind_parameter(idx, i64::from(*b)).unwrap(),
                Value::Float(x) => stmt.raw_bind_parameter(idx, *x).unwrap(),
                Value::Text(t) => stmt.raw_bind_parameter(idx, t.as_str()).unwrap(),
                Value::Null => stmt.raw_bind_parameter(idx, rusqlite::types::Null).unwrap(),
            }
        }
    }
    let mut rows = stmt.raw_query();
    let mut out = Vec::new();
    while let Some(r) = rows.next().unwrap() {
        out.push(r.get::<_, String>(3).unwrap());
    }
    out
}

// spec: UI-145
// spec: UI-147 [P1]
// spec: TECH-073
// spec: TECH-074
#[test]
fn hreflang_relationship_filters_look_up_the_target_row_of_each_annotation() {
    let dir = tempfile::tempdir().unwrap();
    let file = file(dir.path());
    crawl(&file, 20);
    let cfg = CrawlConfig::default();
    for filter in RELATIONSHIP_FILTERS {
        let plan = explain(&file, &cfg, filter);
        // The joined URL row (`q` target, `p` source of a return link) is a primary-key lookup per
        // annotation, never a search over all crawled URLs.
        for alias in ["q", "p"] {
            for line in plan.iter().filter(|l| {
                l.starts_with(&format!("SEARCH {alias} "))
                    || l.starts_with(&format!("SCAN {alias}"))
            }) {
                assert!(
                    line.contains("INTEGER PRIMARY KEY"),
                    "{filter}: {line}\n{plan:#?}"
                );
            }
        }
        assert!(
            plan.iter().any(|l| l.starts_with("SEARCH h ")),
            "{filter}: {plan:#?}"
        );
    }
}

// spec: UI-147 [P1]
// spec: UI-145
#[test]
fn hreflang_tab_counts_of_a_crawl_without_annotations_scale_with_the_annotations() {
    let dir = tempfile::tempdir().unwrap();
    let file = file(dir.path());
    crawl(&file, 4_000);
    let cfg = CrawlConfig::default();
    let tab: Vec<_> = tabs::configured_for_file(&cfg, &file)
        .into_iter()
        .filter(|t| t.id == "Hreflang")
        .collect();
    let before = Instant::now();
    let counts = filter_counts_in(&tab, &file, &cfg).unwrap();
    let elapsed = before.elapsed();
    let count = |f: &str| counts.iter().find(|c| c.filter == f).and_then(|c| c.count);
    assert_eq!(count("All"), Some(2_000));
    assert_eq!(count("Missing"), Some(2_000));
    for filter in RELATIONSHIP_FILTERS {
        assert!(
            count(filter).is_none_or(|n| n == 0),
            "{filter}: {counts:#?}"
        );
    }
    // UI-147: Overview/Issues complete within 5 s for 5 million URLs; one tab of 4 000 URLs must not
    // cost whole seconds (before M2-N26: one scan of all URLs per page and relationship filter, 32.7 s in a
    // debug build). The limit leaves room for slow CI runners; the plan test above is the deterministic guard.
    assert!(
        elapsed < Duration::from_secs(10),
        "Hreflang-Zähler für 4 000 URLs: {elapsed:?}"
    );
}
