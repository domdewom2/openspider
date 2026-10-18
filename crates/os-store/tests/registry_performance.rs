//! M2-O02: Metadaten-Fixture mit 1 000 Crawls und Import-Benchmark für eine Million URLs.
mod store_common;
use os_model::{Clock, IdSource, ManualClock, SeededIds, Timestamp};
use os_store::{
    registry::{self, Manager},
    CrawlFile, OpenMode, StorageProfile,
};
use std::{
    path::Path,
    time::{Duration, Instant},
};
use store_common::*;

fn metadata_fixture(root: &Path, n: usize) {
    std::fs::create_dir_all(root).unwrap();
    let ids = SeededIds::new(42);
    let now = created_at();
    for i in 0..n {
        let id = ids.new_crawl_id(now);
        let path = registry::path_for(root, &id);
        let conn = rusqlite::Connection::open(path).unwrap();
        conn.pragma_update(None, "application_id", os_store::schema::APPLICATION_ID)
            .unwrap();
        conn.pragma_update(
            None,
            "user_version",
            os_store::schema::latest_version(os_store::schema::MIGRATIONS),
        )
        .unwrap();
        conn.execute_batch("CREATE TABLE crawl_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL);CREATE TABLE urls(id INTEGER PRIMARY KEY);CREATE TABLE queue(url_id INTEGER PRIMARY KEY);").unwrap();
        let tx = conn.unchecked_transaction().unwrap();
        for (key, value) in [
            ("crawl_id", id.to_string()),
            ("mode", "spider".into()),
            ("project", format!("Projekt {}", i % 3)),
            ("name", format!("Crawl {i:04}")),
            ("state", "Completed".into()),
            ("start_url", format!("https://site.test/{i}")),
            ("started_at", now.to_db()),
            ("ended_at", now.to_db()),
        ] {
            tx.execute("INSERT INTO crawl_meta VALUES (?1,?2)", [key, &value])
                .unwrap();
        }
        tx.commit().unwrap();
    }
}

// spec: OUT-228
#[test]
fn a_thousand_crawls_load_from_persistent_index_in_under_500_ms() {
    let dir = tempfile::tempdir().unwrap();
    metadata_fixture(dir.path(), 1000);
    let mut m = Manager::open(dir.path()).unwrap();
    assert_eq!(m.list().unwrap().crawls.len(), 1000);
    drop(m);
    let start = Instant::now();
    let mut reopened = Manager::open(dir.path()).unwrap();
    let list = reopened.list().unwrap();
    assert_eq!(list.crawls.len(), 1000);
    assert!(
        start.elapsed() < Duration::from_millis(500),
        "Indexöffnung: {:?}",
        start.elapsed()
    );
    let start = Instant::now();
    let mut names: Vec<_> = list.crawls.iter().map(|l| &l.meta.name).collect();
    names.sort();
    assert!(start.elapsed() < Duration::from_millis(200));
    let start = Instant::now();
    let found = list
        .crawls
        .iter()
        .filter(|l| l.meta.name.to_lowercase().contains("0042"))
        .count();
    assert_eq!(found, 1);
    assert!(start.elapsed() < Duration::from_millis(100));
}

// spec: UI-147
#[test]
#[ignore = "Benchmark: Import einer Million URLs, Budget 60 s; manuell auf ruhiger Maschine"]
fn one_million_url_import_reports_progress_and_finishes_within_sixty_seconds() {
    let source = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let path = new_crawl(source.path(), "large.ospider", StorageProfile::Balanced);
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    file.set_meta("state", "Paused").unwrap();
    file.conn().execute_batch("BEGIN;
        WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<1000000)
        INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,folder_depth,url_length)
        SELECT x,'https://site.test/'||x,'https://site.test/'||x,'https://site.test',1,'queued','link',0,30 FROM n;
        COMMIT; PRAGMA wal_checkpoint(TRUNCATE);").unwrap();
    drop(file);
    let mut manager = Manager::open(target.path()).unwrap();
    let progress = std::sync::Mutex::new(Vec::new());
    let clock = ManualClock::new(Timestamp::from_unix_millis(created_at().unix_millis()));
    let start = Instant::now();
    let copy = manager
        .import(
            &path,
            target.path(),
            &SeededIds::new(99),
            &clock,
            &|done, total| {
                progress.lock().unwrap().push((done, total));
                true
            },
        )
        .unwrap();
    assert!(
        start.elapsed() <= Duration::from_secs(60),
        "Import: {:?}",
        start.elapsed()
    );
    assert_eq!(registry::read_listing(&copy).unwrap().url_count, 1_000_000);
    let progress = progress.lock().unwrap();
    assert!(progress.len() > 2);
    assert_eq!(progress.first().unwrap().0, 0);
    let (done, total) = progress.last().unwrap();
    assert_eq!(done, total);
    assert!(manager.list().unwrap().crawls[0].can_resume(clock.now()));
}
