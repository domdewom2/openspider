//! Lebenszyklus des Index `links_target` (K-74, M1-P32) und Gleichheit der Abfragen mit und ohne Index.
//!
//! Der Index fehlt, sobald ein Writer die Datei öffnet, und entsteht beim regulären Ende (Zustand `Completed`
//! im letzten Batch); Pause und Fehler bauen ihn nicht. `queries::inlinks` liefert mit und ohne Index dieselben
//! Zeilen (ohne Index in einem Lauf über `links`).

mod store_common;

use os_model::*;
use os_store::link_index;
use os_store::queries::{self, LinkSortKey, PageReq};
use os_store::{CrawlFile, CrawlSink, OpenMode, SqliteSink, StorageProfile};
use store_common::*;

fn index_present(path: &std::path::Path) -> bool {
    let file = CrawlFile::open(path, OpenMode::ReadOnly).unwrap();
    link_index::present(file.conn()).unwrap()
}

fn state_batch(state: &str) -> WriteBatch {
    WriteBatch {
        meta: vec![("state".into(), state.into())],
        ..WriteBatch::default()
    }
}

fn small_crawl(dir: &std::path::Path) -> (std::path::PathBuf, SqliteSink) {
    let path = new_crawl(dir, "idx.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);
    let mut batch = batch_with_urls((1..=6).map(|i| (i, true)));
    for i in 1..=5 {
        batch.pages.push(page(
            i,
            vec![hyperlink(i + 1, 0), hyperlink(1, 1), hyperlink(6, 2)],
        ));
    }
    sink.write(batch).unwrap();
    (path, sink)
}

// spec: PERF-002
#[test]
fn a_new_file_has_the_index_until_a_writer_opens_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "new.ospider", StorageProfile::Balanced);
    assert!(index_present(&path), "Migration 0001 legt den Index an");
    let _sink = sink(&path, StorageProfile::Balanced);
    assert!(!index_present(&path), "der Writer entfernt ihn beim Öffnen");
}

// spec: PERF-002
#[test]
fn the_index_is_absent_while_running_and_after_a_pause() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut sink) = small_crawl(dir.path());
    assert!(!index_present(&path), "laufender Crawl");
    sink.write(state_batch("Paused")).unwrap();
    sink.checkpoint().unwrap();
    assert!(!index_present(&path), "pausierter Crawl");
    sink.write(state_batch("Failed")).unwrap();
    assert!(!index_present(&path), "Fehler baut ihn nicht");
}

// spec: PERF-002
#[test]
fn the_regular_end_builds_the_index_in_the_last_batch() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut sink) = small_crawl(dir.path());
    let before = os_store::sink::diag::snapshot()
        .into_iter()
        .find(|(k, _)| *k == "link_index_builds")
        .unwrap()
        .1;
    // Ende, Stopp und Limit tragen alle den Zustand `Completed`; der Index gehört zur Transaktion dieses Batches.
    let mut last = state_batch("Completed");
    last.meta.push(("end_reason".into(), "queue_empty".into()));
    sink.write(last).unwrap();
    assert!(index_present(&path));
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    assert_eq!(
        file.get_meta("state").unwrap().as_deref(),
        Some("Completed")
    );
    let after = os_store::sink::diag::snapshot()
        .into_iter()
        .find(|(k, _)| *k == "link_index_builds")
        .unwrap()
        .1;
    assert!(after > before, "der Aufbau steht in den Messzählern");
    assert_eq!(
        count(
            &file,
            "SELECT COUNT(*) FROM crawl_events WHERE code = 'links_index_built' AND kind = 'info'"
        ),
        1,
        "der Aufbau steht als Ereignis in der Datei"
    );
    // ein weiterer Batch baut ihn nicht noch einmal
    sink.write(state_batch("Completed")).unwrap();
    assert!(index_present(&path));
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    assert_eq!(
        count(
            &file,
            "SELECT COUNT(*) FROM crawl_events WHERE code = 'links_index_built'"
        ),
        1
    );
}

// spec: PERF-002
#[test]
fn only_the_last_state_entry_of_a_merged_batch_counts() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut sink) = small_crawl(dir.path());
    // Der Gruppen-Commit der Engine fasst Batches zusammen: `Running` vor `Completed` zählt als Ende ...
    let mut merged = state_batch("Running");
    merged.meta.push(("state".into(), "Completed".into()));
    sink.write(merged).unwrap();
    assert!(index_present(&path));
    // ... und ein späteres `Paused` hebt es nicht auf, verhindert aber den Aufbau.
    let dir2 = tempfile::tempdir().unwrap();
    let (path2, mut sink2) = small_crawl(dir2.path());
    let mut merged = state_batch("Completed");
    merged.meta.push(("state".into(), "Paused".into()));
    sink2.write(merged).unwrap();
    assert!(!index_present(&path2));
}

// spec: PERF-002
#[test]
fn reopening_for_writing_removes_the_index_again() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut sink) = small_crawl(dir.path());
    sink.write(state_batch("Completed")).unwrap();
    sink.finish().unwrap();
    drop(sink);
    assert!(index_present(&path));
    let _again = SqliteSink::open(&path, StorageProfile::Balanced).unwrap();
    assert!(!index_present(&path), "Fortsetzen oder Neu-Crawlen");
}

// spec: PERF-002
#[test]
fn ensure_builds_the_index_on_demand_and_only_for_idle_files() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut sink) = small_crawl(dir.path());
    sink.write(state_batch("Paused")).unwrap();
    sink.finish().unwrap();
    drop(sink);

    // Nur-Lese-Verbindung: nichts zu tun
    let ro = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    assert!(!link_index::ensure_if_idle(&ro).unwrap());
    drop(ro);
    assert!(!index_present(&path));

    // pausierte Datei mit Schreibzugriff: gebaut, danach nicht noch einmal
    let rw = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    assert!(link_index::ensure_if_idle(&rw).unwrap());
    assert!(link_index::present(rw.conn()).unwrap());
    assert!(!link_index::ensure_if_idle(&rw).unwrap());
    assert!(!link_index::ensure(rw.conn()).unwrap());

    // ein lebendiger Crawl (Zustand Running mit frischem Heartbeat) baut ihn selbst am Ende
    link_index::drop_index(rw.conn()).unwrap();
    let now = os_model::time::SystemClock.now().to_db();
    rw.set_meta("state", "Running").unwrap();
    rw.set_meta("heartbeat_at", &now).unwrap();
    assert!(!link_index::ensure_if_idle(&rw).unwrap());
    assert!(!link_index::present(rw.conn()).unwrap());
}

fn all_sort_columns() -> Vec<&'static str> {
    vec![
        "Type",
        "Source",
        "Destination",
        "Size (Bytes)",
        "Alt Text",
        "Anchor",
        "Status Code",
        "Status",
        "Follow",
        "Target",
        "Rel",
        "Path Type",
        "Link Path",
        "Link Position",
        "Link Origin",
    ]
}

// spec: PERF-002
#[test]
fn inlinks_are_the_same_with_and_without_the_index() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "same.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);
    let mut batch = batch_with_urls((1..=12).map(|i| (i, i != 11)));
    for i in 2..=10 {
        let mut a = hyperlink(1, 0);
        a.anchor = Some(format!("anker {}", i % 3));
        let mut b = link(Some(1), LinkType::Image, 1);
        b.alt_text = Some(format!("alt {i}"));
        let mut c = hyperlink(i + 1, 2);
        c.follow = i % 2 == 0;
        batch.pages.push(page(i, vec![a, b, c, hyperlink(1, 3)]));
    }
    sink.write(batch).unwrap();
    sink.finish().unwrap();
    drop(sink);

    let without = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    assert!(!link_index::present(without.conn()).unwrap());
    let with_path = dir.path().join("with.ospider");
    std::fs::copy(&path, &with_path).unwrap();
    {
        let rw = CrawlFile::open(&with_path, OpenMode::ReadWrite).unwrap();
        assert!(link_index::ensure(rw.conn()).unwrap());
    }
    let with = CrawlFile::open(&with_path, OpenMode::ReadOnly).unwrap();
    assert!(link_index::present(with.conn()).unwrap());

    let mut requests = vec![
        PageReq::new(0, 500),
        PageReq::new(3, 4),
        PageReq::new(100, 5),
    ];
    for column in all_sort_columns() {
        for desc in [false, true] {
            let mut req = PageReq::new(2, 7);
            req.sort = vec![LinkSortKey {
                column: column.to_string(),
                desc,
            }];
            requests.push(req);
        }
    }
    for target in [1, 5, 11, 12] {
        for req in &requests {
            let a = queries::inlinks(without.conn(), UrlId(target), req.clone()).unwrap();
            let b = queries::inlinks(with.conn(), UrlId(target), req.clone()).unwrap();
            assert_eq!(a, b, "Ziel {target}, {req:?}");
        }
    }
    // Ziel 1 hat 27 Inlinks (9 Seiten x 3 Linktypen)
    let page = queries::inlinks(without.conn(), UrlId(1), PageReq::new(0, 500)).unwrap();
    assert_eq!(page.total, 27);
    assert_eq!(page.items.len(), 27);
}
