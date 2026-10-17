//! Migration älterer Schemaversionen auf v3 (OUT-184, CRAWL-127): Sicherungskopie, Migration als Kopie bei
//! unverändertem Original (Byte-Vergleich), Fehlerfälle.

mod store_common;

use std::path::{Path, PathBuf};

use os_store::{backup_path, schema, CrawlFile, OpenMode, StoreError, SCHEMA_VERSION};
use store_common::*;

fn bytes(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap()
}

fn companions(path: &Path) -> Vec<String> {
    ["-wal", "-shm", "-journal"]
        .iter()
        .map(|s| format!("{}{s}", path.display()))
        .filter(|p| Path::new(p).exists())
        .collect()
}

fn version_of(path: &Path) -> u32 {
    let conn = rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .unwrap();
    schema::user_version(&conn).unwrap()
}

// spec: OUT-184 [M2]
// spec: CRAWL-127
#[test]
fn v1_and_v2_files_migrate_in_place_with_a_byte_identical_backup() {
    for (fixture, old) in [("schema-v1.ospider", 1u32), ("schema-v2.ospider", 2u32)] {
        let dir = tempfile::tempdir().unwrap();
        let path = copy_fixture(fixture, dir.path());
        let original = bytes(&path);
        assert_eq!(version_of(&path), old);

        let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
        assert_eq!(schema::user_version(file.conn()).unwrap(), SCHEMA_VERSION);
        assert_eq!(count(&file, "SELECT COUNT(*) FROM urls"), 42);
        assert_eq!(
            file.get_meta("schema_version").unwrap().as_deref(),
            Some(SCHEMA_VERSION.to_string().as_str())
        );
        // Die Sicherung ist der Zustand vor der Migration, Byte für Byte.
        let backup = backup_path(&path, old);
        assert_eq!(bytes(&backup), original, "{fixture}: Sicherung = Original");
        assert_eq!(version_of(&backup), old);
        // Die Zähler stimmen nach der Migration mit der Neuberechnung überein.
        assert!(os_store::counters::verify_all(file.conn()).unwrap().is_empty());
    }
}

// spec: OUT-184 [M2]
#[test]
fn migrating_as_a_copy_leaves_the_original_unchanged_byte_for_byte() {
    for (fixture, old) in [("schema-v1.ospider", 1u32), ("schema-v2.ospider", 2u32)] {
        let dir = tempfile::tempdir().unwrap();
        let src = copy_fixture(fixture, dir.path());
        let original = bytes(&src);
        let dst = dir.path().join("kopie.ospider");

        let copy = CrawlFile::migrate_copy(&src, &dst).unwrap();
        assert_eq!(schema::user_version(copy.conn()).unwrap(), SCHEMA_VERSION);
        assert_eq!(count(&copy, "SELECT COUNT(*) FROM urls"), 42);
        assert_eq!(count(&copy, "SELECT COUNT(*) FROM queue"), 15);
        assert!(os_store::counters::verify_all(copy.conn()).unwrap().is_empty());
        drop(copy);

        assert_eq!(bytes(&src), original, "{fixture}: Original unverändert");
        assert_eq!(version_of(&src), old);
        assert!(companions(&src).is_empty(), "keine Begleitdateien am Original");
        assert!(
            !backup_path(&dst, old).exists() && !backup_path(&src, old).exists(),
            "bei einer Kopie braucht es keine Sicherung: das Original ist sie"
        );
        // Die Kopie ist eine eigenständige v3-Datei mit vollständigem Manifest.
        let reopened = CrawlFile::open(&dst, OpenMode::ReadOnly).unwrap();
        reopened.check_read_api().unwrap();
    }
}

#[test]
fn migrating_a_current_file_as_a_copy_is_a_plain_copy_of_the_content() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("aktuell.ospider");
    build_paused_crawl(&src);
    let before = bytes(&src);
    let dst = dir.path().join("kopie.ospider");
    let copy = CrawlFile::migrate_copy(&src, &dst).unwrap();
    assert_eq!(count(&copy, "SELECT COUNT(*) FROM urls"), 42);
    assert_eq!(bytes(&src), before);
}

#[test]
fn migrating_as_a_copy_rejects_missing_foreign_newer_files_and_existing_targets() {
    let dir = tempfile::tempdir().unwrap();
    let dst = dir.path().join("kopie.ospider");

    assert!(matches!(
        CrawlFile::migrate_copy(&dir.path().join("fehlt.ospider"), &dst),
        Err(StoreError::NotFound(_))
    ));
    assert!(!dst.exists());

    let foreign = copy_fixture("not-a-crawl.ospider", dir.path());
    assert!(matches!(
        CrawlFile::migrate_copy(&foreign, &dst),
        Err(StoreError::NotACrawlFile { .. })
    ));
    assert!(!dst.exists());

    let newer = copy_fixture("schema-v99.ospider", dir.path());
    let before = bytes(&newer);
    assert!(matches!(
        CrawlFile::migrate_copy(&newer, &dst),
        Err(StoreError::NewerVersion { found: 99, .. })
    ));
    assert!(!dst.exists());
    assert_eq!(bytes(&newer), before);

    let v2 = copy_fixture("schema-v2.ospider", dir.path());
    std::fs::write(&dst, b"vorhanden").unwrap();
    assert!(matches!(
        CrawlFile::migrate_copy(&v2, &dst),
        Err(StoreError::AlreadyExists(_))
    ));
    assert_eq!(bytes(&dst), b"vorhanden");
}

#[test]
fn migrated_files_never_leave_half_written_copies_behind() {
    let dir = tempfile::tempdir().unwrap();
    let corrupt = copy_fixture("corrupt.ospider", dir.path());
    let dst: PathBuf = dir.path().join("kopie.ospider");
    assert!(CrawlFile::migrate_copy(&corrupt, &dst).is_err());
    assert!(!dst.exists());
    assert!(companions(&dst).is_empty());
}
