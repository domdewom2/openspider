//! Lebenszyklus der Sekundärindizes von `urls` mit zufälligen Schlüsseln (M1-N22, K-80; Vorbild `link_index.rs`).
//!
//! Die Indizes `urls_title1`, `urls_md1`, `urls_h1`, `urls_h2` und `urls_hash` fehlen in einem großen Crawl (ab
//! `DROP_FROM_URLS` URLs; die Tests setzen die Schwelle herunter) und entstehen beim regulären Ende (Zustand
//! `Completed` im letzten Batch) in dessen Transaktion. Alle übrigen Indizes von `urls` bleiben stehen, kleine Crawls
//! behalten alle.

mod store_common;

use os_model::*;
use os_store::link_index;
use os_store::url_index::{self, WRITE_PHASE_DROPPED};
use os_store::{CrawlFile, CrawlSink, OpenMode, StorageProfile};
use store_common::*;

fn names(path: &std::path::Path) -> Vec<String> {
    let file = CrawlFile::open(path, OpenMode::ReadOnly).unwrap();
    let mut stmt = file
        .conn()
        .prepare("SELECT name FROM sqlite_master WHERE type = 'index' AND tbl_name = 'urls' ORDER BY name")
        .unwrap();
    stmt.query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn dropped_present(path: &std::path::Path) -> Vec<String> {
    names(path)
        .into_iter()
        .filter(|n| WRITE_PHASE_DROPPED.iter().any(|i| i.name == n))
        .collect()
}

fn state_batch(state: &str) -> WriteBatch {
    WriteBatch {
        meta: vec![("state".into(), state.into())],
        ..WriteBatch::default()
    }
}

fn small_crawl(dir: &std::path::Path) -> (std::path::PathBuf, os_store::SqliteSink) {
    let path = new_crawl(dir, "urlidx.ospider", StorageProfile::Balanced);
    // Schwelle 1: schon die sechs URLs gelten als „groß“
    let mut sink = sink(&path, StorageProfile::Balanced).with_url_index_drop_at(1);
    let mut batch = batch_with_urls((1..=6).map(|i| (i, true)));
    for i in 1..=5 {
        batch.pages.push(page(i, vec![hyperlink(i + 1, 0)]));
    }
    sink.write(batch).unwrap();
    (path, sink)
}

// spec: PERF-002
#[test]
fn the_random_key_indexes_are_absent_while_writing_and_the_others_stay() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "new.ospider", StorageProfile::Balanced);
    assert_eq!(
        dropped_present(&path).len(),
        5,
        "Migration 0001 legt sie an"
    );
    let (path, mut sink) = small_crawl(dir.path());
    assert!(dropped_present(&path).is_empty(), "laufender Crawl");
    sink.write(state_batch("Paused")).unwrap();
    sink.checkpoint().unwrap();
    assert!(dropped_present(&path).is_empty(), "pausierter Crawl");
    sink.write(state_batch("Failed")).unwrap();
    assert!(dropped_present(&path).is_empty(), "Fehler baut sie nicht");
    // der Änderungsfeed, der Dublettenschutz und die Indizes über Spalten mit wenigen Werten bleiben
    let rest = names(&path);
    for keep in [
        "urls_change_seq",
        "urls_status",
        "urls_class",
        "urls_state",
        "urls_depth",
        "urls_redirect",
        "urls_canon",
        "urls_index",
    ] {
        assert!(rest.iter().any(|n| n == keep), "{keep} bleibt: {rest:?}");
    }
    assert!(
        rest.iter().any(|n| n.starts_with("sqlite_autoindex_urls")),
        "UNIQUE(url) bleibt"
    );
}

// spec: PERF-002
#[test]
fn a_small_crawl_keeps_every_index_and_the_threshold_drops_them_in_the_batch_that_reaches_it() {
    assert_eq!(os_store::url_index::DROP_FROM_URLS, 250_000);
    let dir = tempfile::tempdir().unwrap();
    // Standardschwelle: sechs URLs sind klein
    let path = new_crawl(dir.path(), "small.ospider", StorageProfile::Balanced);
    let mut small = sink(&path, StorageProfile::Balanced);
    small
        .write(batch_with_urls((1..=6).map(|i| (i, true))))
        .unwrap();
    assert_eq!(
        dropped_present(&path).len(),
        5,
        "kleiner Crawl: alle Indizes bleiben"
    );
    // Schwelle 10: der erste Batch (URLs 1 bis 6) behält sie, der zweite (bis 12) entfernt sie vor dem Schreiben
    let path = new_crawl(dir.path(), "mid.ospider", StorageProfile::Balanced);
    let mut mid = sink(&path, StorageProfile::Balanced).with_url_index_drop_at(10);
    mid.write(batch_with_urls((1..=6).map(|i| (i, true))))
        .unwrap();
    assert_eq!(dropped_present(&path).len(), 5);
    mid.write(batch_with_urls((7..=12).map(|i| (i, true))))
        .unwrap();
    assert!(dropped_present(&path).is_empty());
    // ein Lauf ohne Schwelle (Vergleichslauf) behält sie bis zum Ende
    let path = new_crawl(dir.path(), "keep.ospider", StorageProfile::Balanced);
    let mut keep = sink(&path, StorageProfile::Balanced)
        .with_url_index_drop_at(1)
        .keep_url_indexes();
    keep.write(batch_with_urls((1..=6).map(|i| (i, true))))
        .unwrap();
    assert_eq!(dropped_present(&path).len(), 5);
    // Fortsetzen: eine Datei, die schon über der Schwelle liegt, verliert sie mit dem ersten Schreiben
    drop(keep);
    let mut resumed = sink(&path, StorageProfile::Balanced).with_url_index_drop_at(5);
    assert_eq!(
        dropped_present(&path).len(),
        5,
        "Öffnen allein ändert nichts"
    );
    resumed.write(state_batch("Running")).unwrap();
    assert!(
        dropped_present(&path).is_empty(),
        "die Datei hat 6 URLs, Schwelle 5"
    );
}

/// Definitionen der fünf Indizes in der Datei (Name, SQL), nach Name sortiert.
fn definitions(path: &std::path::Path) -> Vec<(String, String)> {
    let file = CrawlFile::open(path, OpenMode::ReadOnly).unwrap();
    let mut stmt = file
        .conn()
        .prepare(
            "SELECT name, sql FROM sqlite_master WHERE type = 'index' AND name IN \
             ('urls_title1', 'urls_md1', 'urls_h1', 'urls_h2', 'urls_hash') ORDER BY name",
        )
        .unwrap();
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn completed_batch() -> WriteBatch {
    let mut last = state_batch("Completed");
    last.meta.push(("end_reason".into(), "queue_empty".into()));
    last
}

// spec: PERF-002
#[test]
fn k83_a_large_file_does_not_get_the_five_indexes_back_at_the_regular_end_but_links_target_stands()
{
    let dir = tempfile::tempdir().unwrap();
    let (path, mut sink) = small_crawl(dir.path()); // Schwelle 1: die sechs URLs gelten als „groß“
    assert!(dropped_present(&path).is_empty());
    sink.write(completed_batch()).unwrap();
    assert!(
        dropped_present(&path).is_empty(),
        "kein Aufbau der fünf Indizes im Endbatch (K-83)"
    );
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    assert!(
        link_index::present(file.conn()).unwrap(),
        "links_target entsteht weiterhin im Endbatch (K-74)"
    );
    assert_eq!(count(&file, "SELECT COUNT(*) FROM links"), 5);
    // alle übrigen Indizes bleiben
    let rest = names(&path);
    for keep in [
        "urls_change_seq",
        "urls_status",
        "urls_class",
        "urls_redirect",
    ] {
        assert!(rest.iter().any(|n| n == keep), "{keep}: {rest:?}");
    }
}

// spec: PERF-002
#[test]
fn k83_a_small_file_keeps_all_five_indexes_through_the_regular_end() {
    let dir = tempfile::tempdir().unwrap();
    let original = definitions(&new_crawl(
        dir.path(),
        "ref.ospider",
        StorageProfile::Balanced,
    ));
    assert_eq!(original.len(), 5);
    let path = new_crawl(dir.path(), "small-end.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced); // Standardschwelle 250 000
    let mut batch = batch_with_urls((1..=6).map(|i| (i, true)));
    for i in 1..=5 {
        batch.pages.push(page(i, vec![hyperlink(i + 1, 0)]));
    }
    sink.write(batch).unwrap();
    sink.write(completed_batch()).unwrap();
    assert_eq!(
        definitions(&path),
        original,
        "alle fünf stehen mit der Definition aus Migration 0001"
    );
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    assert!(link_index::present(file.conn()).unwrap());
    // fehlt in einer kleinen Datei ausnahmsweise einer, baut sie das Ende wieder auf (wie bisher)
    drop(file);
    let rw = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    rw.conn().execute_batch("DROP INDEX urls_h1").unwrap();
    drop(rw);
    let mut again = sink_again(&path);
    again.write(completed_batch()).unwrap();
    assert_eq!(definitions(&path), original);
}

/// Eine Datei, deren größte URL-Kennung die Standardschwelle erreicht (ohne Test-Hook des Writers).
fn large_by_id(dir: &std::path::Path) -> (std::path::PathBuf, os_store::SqliteSink) {
    let path = new_crawl(dir, "large.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);
    let mut ids: Vec<(i64, bool)> = (1..=6).map(|i| (i, true)).collect();
    ids.push((url_index::DROP_FROM_URLS, true));
    let mut batch = batch_with_urls(ids);
    for i in 1..=5 {
        batch.pages.push(page(i, vec![hyperlink(i + 1, 0)]));
    }
    sink.write(batch).unwrap();
    (path, sink)
}

// spec: PERF-002
#[test]
fn k83_ensure_if_idle_builds_links_target_but_not_the_five_indexes_of_a_large_file() {
    let dir = tempfile::tempdir().unwrap();
    let (path, sink) = large_by_id(dir.path());
    drop(sink); // Abbruch ohne Ende
    assert!(
        dropped_present(&path).is_empty(),
        "Schwelle erreicht: entfernt"
    );
    let rw = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    assert!(url_index::is_large(rw.conn()).unwrap());
    assert!(!link_index::present(rw.conn()).unwrap());
    assert!(
        link_index::ensure_if_idle(&rw).unwrap(),
        "links_target wird nachgezogen"
    );
    assert!(link_index::present(rw.conn()).unwrap());
    assert!(
        !url_index::present(rw.conn()).unwrap(),
        "die fünf Indizes nicht (K-83)"
    );
    assert!(
        !link_index::ensure_if_idle(&rw).unwrap(),
        "zweiter Aufruf: nichts zu tun, auch ohne die fünf Indizes"
    );
    // ensure bleibt öffentlich für künftige Leser
    assert_eq!(url_index::ensure(rw.conn()).unwrap(), 5);
    assert!(url_index::present(rw.conn()).unwrap());
}

// spec: PERF-002
#[test]
fn k83_the_regular_end_of_a_large_file_by_the_default_threshold_builds_only_links_target() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut sink) = large_by_id(dir.path());
    sink.write(completed_batch()).unwrap();
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    assert!(link_index::present(file.conn()).unwrap());
    assert!(dropped_present(&path).is_empty());
    assert!(url_index::is_large(file.conn()).unwrap());
}

// spec: PERF-002
#[test]
fn k83_a_small_file_is_not_large_and_is_completed_by_ensure_if_idle() {
    let dir = tempfile::tempdir().unwrap();
    let (path, sink) = small_crawl(dir.path()); // vom Test-Hook entfernt, aber nur sechs URLs
    drop(sink);
    let rw = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    assert!(!url_index::is_large(rw.conn()).unwrap());
    assert!(link_index::ensure_if_idle(&rw).unwrap());
    assert!(
        url_index::present(rw.conn()).unwrap(),
        "kleine Datei: alle stehen"
    );
}

// spec: PERF-002
#[test]
fn ensure_if_idle_rebuilds_after_an_abort_and_a_new_writer_drops_them_again() {
    let dir = tempfile::tempdir().unwrap();
    let (path, sink) = small_crawl(dir.path());
    // harter Abbruch: weder Pause noch Ende (hier: die Senke wird fallen gelassen, ohne `Completed`)
    drop(sink);
    assert!(dropped_present(&path).is_empty());
    let rw = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    // Der gespeicherte Zustand ist nicht `Completed`; ein Lauf, dessen Heartbeat fehlt, gilt als beendet.
    assert!(link_index::ensure_if_idle(&rw).unwrap());
    assert!(url_index::present(rw.conn()).unwrap());
    assert!(link_index::present(rw.conn()).unwrap());
    assert!(
        !link_index::ensure_if_idle(&rw).unwrap(),
        "zweiter Aufruf: nichts zu tun"
    );
    assert_eq!(url_index::ensure(rw.conn()).unwrap(), 0);
    drop(rw);
    // Fortsetzen: der nächste Writer entfernt sie mit seinem ersten Schreiben wieder (Öffnen allein ändert nichts)
    let mut again = sink_again(&path).with_url_index_drop_at(1);
    assert_eq!(dropped_present(&path).len(), 5);
    again.write(state_batch("Running")).unwrap();
    assert!(dropped_present(&path).is_empty());
}

fn sink_again(path: &std::path::Path) -> os_store::SqliteSink {
    sink(path, StorageProfile::Balanced)
}

// spec: PERF-002
#[test]
fn ensure_builds_only_the_missing_indexes() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "partial.ospider", StorageProfile::Balanced);
    let rw = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    rw.conn()
        .execute_batch("DROP INDEX urls_h1; DROP INDEX urls_hash")
        .unwrap();
    assert!(!url_index::present(rw.conn()).unwrap());
    assert_eq!(url_index::ensure(rw.conn()).unwrap(), 2);
    assert!(url_index::present(rw.conn()).unwrap());
    assert_eq!(url_index::ensure(rw.conn()).unwrap(), 0);
}

/// Zahl der Zeilen von `urls`, deren Titel (`title_1`) bei mindestens einer anderen Zeile vorkommt (so fragt der Filter
/// „Duplicate“ der Tabs, korreliertes `EXISTS`).
fn duplicate_titles(file: &CrawlFile) -> (i64, String) {
    let sql = "SELECT COUNT(*) FROM urls u WHERE u.title_1 IS NOT NULL AND \
               EXISTS (SELECT 1 FROM urls d WHERE d.title_1 = u.title_1 AND d.id <> u.id)";
    let count = file.conn().query_row(sql, [], |r| r.get(0)).unwrap();
    let mut stmt = file
        .conn()
        .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
        .unwrap();
    let plan: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(3))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    (count, plan.join(" | "))
}

// spec: PERF-002
#[test]
fn duplicate_queries_give_the_same_rows_without_the_indexes() {
    let dir = tempfile::tempdir().unwrap();
    let (path, sink) = small_crawl(dir.path());
    drop(sink);
    // Titel 1 und 2 sind gleich, 3 und 4 sind gleich, 5 ist einmalig, 6 hat keinen
    let rw = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    for (id, title) in [(1, "A"), (2, "A"), (3, "B"), (4, "B"), (5, "C")] {
        rw.conn()
            .execute(
                "UPDATE urls SET title_1 = ?1 WHERE id = ?2",
                rusqlite::params![title, id],
            )
            .unwrap();
    }
    assert!(!url_index::present(rw.conn()).unwrap());
    let (without, plan_without) = duplicate_titles(&rw);
    assert_eq!(without, 4, "ohne Index: {plan_without}");
    assert!(url_index::ensure(rw.conn()).unwrap() > 0);
    let (with, plan_with) = duplicate_titles(&rw);
    assert_eq!(with, without, "mit Index: {plan_with}");
    assert!(plan_with.contains("urls_title1"), "mit Index: {plan_with}");
}

// spec: PERF-002
#[test]
fn the_prefetch_readers_cover_every_row_and_small_files_skip_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "prefetch.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);
    sink.write(batch_with_urls((1..=300).map(|i| (i, true))))
        .unwrap();
    drop(sink);
    let rows = || {
        os_store::sink::diag::snapshot()
            .into_iter()
            .find(|(k, _)| *k == "url_index_prefetch_rows")
            .unwrap()
            .1
    };
    assert!(
        url_index::Prefetch::start(&path).is_none(),
        "unter 100 000 Kennungen lohnt das Vorwärmen nicht"
    );
    let before = rows();
    let prefetch = url_index::Prefetch::start_with(&path, 4, 10).expect("300 Kennungen");
    drop(prefetch); // wartet auf die Leser
    assert_eq!(rows() - before, 300, "jede Zeile wird genau einmal gelesen");
}
