mod schema_common;
use os_store::{
    view_cache::{self, Entry},
    CrawlFile, OpenMode,
};

// spec: UI-141
#[test]
fn disposable_blobs_roundtrip_replace_and_clear() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = completed_fixture(dir.path(), "cache.ospider");
    assert_eq!(
        view_cache::load(&file, "Internal:HTML", "params").unwrap(),
        None
    );
    let mut entry = Entry {
        step_stamp: "finished:hash".into(),
        built_seq: 12,
        data: vec![0, 1, 255],
    };
    assert!(view_cache::save(&file, "Internal:HTML", "params", &entry).unwrap());
    assert_eq!(
        view_cache::load(&file, "Internal:HTML", "params").unwrap(),
        Some(entry.clone())
    );
    assert_eq!(
        view_cache::load(&file, "Internal:HTML", "different").unwrap(),
        None
    );
    entry.built_seq = 13;
    entry.data = vec![3, 4];
    view_cache::save(&file, "Internal:HTML", "params", &entry).unwrap();
    assert_eq!(
        view_cache::load(&file, "Internal:HTML", "params").unwrap(),
        Some(entry)
    );
    assert!(view_cache::clear(&file).unwrap());
    assert_eq!(
        view_cache::load(&file, "Internal:HTML", "params").unwrap(),
        None
    );
}
// spec: UI-142
#[test]
fn read_only_connections_do_not_mutate_cache() {
    let dir = tempfile::tempdir().unwrap();
    let (path, file) = completed_fixture(dir.path(), "cache.ospider");
    let entry = Entry {
        step_stamp: String::new(),
        built_seq: 5,
        data: vec![5],
    };
    view_cache::save(&file, "key", "params", &entry).unwrap();
    drop(file);
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    assert!(!view_cache::save(&file, "other", "params", &entry).unwrap());
    assert!(!view_cache::clear(&file).unwrap());
    assert_eq!(
        view_cache::load(&file, "key", "params").unwrap(),
        Some(entry)
    );
    assert_eq!(view_cache::load(&file, "other", "params").unwrap(), None);
}

// spec: UI-141
#[test]
fn variants_are_replaced_atomically_and_running_crawls_skip_persistence() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = completed_fixture(dir.path(), "bounded.ospider");
    let entry = Entry {
        step_stamp: String::new(),
        built_seq: 1,
        data: vec![1],
    };
    view_cache::save(&file, "Internal:All", "a", &entry).unwrap();
    view_cache::save(&file, "Internal:All", "b", &entry).unwrap();
    assert!(view_cache::load(&file, "Internal:All", "a")
        .unwrap()
        .is_none());
    assert_eq!(
        file.conn()
            .query_row("SELECT COUNT(*) FROM view_cache", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    file.set_meta("state", "Running").unwrap();
    assert!(!view_cache::save(&file, "Internal:All", "c", &entry).unwrap());
    assert!(view_cache::load(&file, "Internal:All", "b")
        .unwrap()
        .is_some());
}

// spec: UI-142
#[test]
fn busy_writer_never_delays_optional_cache_save_and_timeout_is_restored() {
    let dir = tempfile::tempdir().unwrap();
    let (path, file) = completed_fixture(dir.path(), "busy.ospider");
    let writer = rusqlite::Connection::open(path).unwrap();
    writer.execute_batch("BEGIN IMMEDIATE").unwrap();
    let entry = Entry {
        step_stamp: String::new(),
        built_seq: 1,
        data: vec![1],
    };
    let before: u64 = file
        .conn()
        .pragma_query_value(None, "busy_timeout", |r| r.get(0))
        .unwrap();
    let started = std::time::Instant::now();
    assert!(view_cache::save(&file, "key", "params", &entry).is_err());
    assert!(started.elapsed() < std::time::Duration::from_millis(250));
    assert_eq!(
        file.conn()
            .pragma_query_value(None, "busy_timeout", |r| r.get::<_, u64>(0))
            .unwrap(),
        before
    );
    writer.execute_batch("ROLLBACK").unwrap();
}

fn completed_fixture(dir: &std::path::Path, name: &str) -> (std::path::PathBuf, CrawlFile) {
    let (path, file) = schema_common::create(dir, name);
    file.set_meta("state", "Completed").unwrap();
    (path, file)
}
