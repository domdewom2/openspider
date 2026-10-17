mod schema_common;
use os_store::{
    view_cache::{self, Entry},
    CrawlFile, OpenMode,
};

// spec: UI-141
#[test]
fn disposable_blobs_roundtrip_replace_and_clear() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = schema_common::create(dir.path(), "cache.ospider");
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
    let (path, file) = schema_common::create(dir.path(), "cache.ospider");
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
