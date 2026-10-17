//! Migrationsrahmen (CRAWL-127): 0→1, Sicherungskopie bei älteren Dateien, Fehler bei kaputten Listen.

mod schema_common;

use std::fs;

use os_store::schema::{self, Migration};
use os_store::{backup_path, CrawlFile, OpenMode, StoreError, MIGRATIONS};
use rusqlite::Connection;
use schema_common::*;

const TEST_V1: Migration = Migration {
    version: 1,
    name: "test-init",
    sql: "CREATE TABLE a (x INTEGER); INSERT INTO a VALUES (1);",
};
const TEST_V2: Migration = Migration {
    version: 2,
    name: "test-add-b",
    sql: "CREATE TABLE b (y TEXT); ALTER TABLE urls ADD COLUMN extra INTEGER;",
};

// spec: CRAWL-127
#[test]
fn migration_0_to_1_creates_schema_and_sets_user_version() {
    let mut conn = Connection::open_in_memory().unwrap();
    assert_eq!(schema::user_version(&conn).unwrap(), 0);
    let reached = schema::migrate(&mut conn, 0, &[TEST_V1]).unwrap();
    assert_eq!(reached, 1);
    assert_eq!(schema::user_version(&conn).unwrap(), 1);
    let x: i64 = conn.query_row("SELECT x FROM a", [], |r| r.get(0)).unwrap();
    assert_eq!(x, 1);
    // erneutes Anwenden ab Version 1 ändert nichts
    assert_eq!(schema::migrate(&mut conn, 1, &[TEST_V1]).unwrap(), 1);
}

// spec: CRAWL-127
#[test]
fn production_migration_list_is_consecutive_and_matches_schema_version() {
    assert_eq!(schema::latest_version(MIGRATIONS), os_store::SCHEMA_VERSION);
    let mut conn = Connection::open_in_memory().unwrap();
    schema::migrate(&mut conn, 0, MIGRATIONS).unwrap();
    assert_eq!(schema::user_version(&conn).unwrap(), 3);
}

// spec: CRAWL-127
#[test]
fn failing_migration_rolls_back_completely() {
    let mut conn = Connection::open_in_memory().unwrap();
    let bad = Migration {
        version: 1,
        name: "kaputt",
        sql: "CREATE TABLE ok (a); INSERT INTO gibtsnicht VALUES (1);",
    };
    assert!(schema::migrate(&mut conn, 0, &[bad]).is_err());
    assert_eq!(schema::user_version(&conn).unwrap(), 0);
    let n: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE name = 'ok'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 0, "Transaktion muss zurückgerollt sein");
}

// spec: CRAWL-127
#[test]
fn migration_list_with_gap_is_rejected() {
    let mut conn = Connection::open_in_memory().unwrap();
    let skip = Migration {
        version: 3,
        name: "x",
        sql: "SELECT 1",
    };
    assert!(matches!(
        schema::migrate(&mut conn, 0, &[TEST_V1, skip]),
        Err(StoreError::Migration(_))
    ));
    assert!(matches!(
        schema::migrate(&mut conn, 0, &[TEST_V2]),
        Err(StoreError::Migration(_))
    ));
}

// spec: CRAWL-127
#[test]
fn older_file_is_backed_up_then_migrated() {
    let dir = tempfile::tempdir().unwrap();
    let (path, f) = create(dir.path(), "crawl.ospider");
    f.set_meta("name", "Alt").unwrap();
    drop(f);
    let before = fs::read(&path).unwrap();

    // Das aktuelle Produktionsschema (Version 3) gilt als Ausgangspunkt; es kommt eine Testversion 4 hinzu.
    let migrations = [
        MIGRATIONS[0],
        MIGRATIONS[1],
        MIGRATIONS[2],
        Migration {
            version: 4,
            ..TEST_V2
        },
    ];
    let f = CrawlFile::open_with(&path, OpenMode::ReadWrite, &migrations).unwrap();

    let backup = dir.path().join("crawl.v3.bak.ospider");
    assert_eq!(backup, backup_path(&path, 3));
    assert!(backup.exists(), "Sicherungskopie fehlt");
    assert_eq!(
        fs::read(&backup).unwrap(),
        before,
        "Sicherung = Zustand vor der Migration"
    );

    // Datei ist migriert, Metadaten behalten.
    assert_eq!(schema::user_version(f.conn()).unwrap(), 4);
    assert_eq!(f.get_meta("schema_version").unwrap().as_deref(), Some("4"));
    assert_eq!(f.meta().unwrap().name, "Alt");
    let n: i64 = f
        .conn()
        .query_row("SELECT count(*) FROM b", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 0);
    drop(f);

    // Die Sicherung ist selbst eine gültige v3-Datei.
    let old = CrawlFile::open(&backup, OpenMode::ReadOnly).unwrap();
    assert_eq!(old.meta().unwrap().schema_version, 3);
    assert!(old
        .conn()
        .query_row("SELECT count(*) FROM b", [], |r| r.get::<_, i64>(0))
        .is_err());
}

// spec: CRAWL-127
#[test]
fn read_only_open_of_older_file_does_not_migrate() {
    let dir = tempfile::tempdir().unwrap();
    let (path, f) = create(dir.path(), "crawl.ospider");
    drop(f);
    let before = fs::read(&path).unwrap();
    let migrations = [
        MIGRATIONS[0],
        MIGRATIONS[1],
        MIGRATIONS[2],
        Migration {
            version: 4,
            ..TEST_V2
        },
    ];
    let err = CrawlFile::open_with(&path, OpenMode::ReadOnly, &migrations)
        .err()
        .unwrap();
    assert!(
        matches!(
            err,
            StoreError::MigrationRequired {
                found: 3,
                supported: 4,
                ..
            }
        ),
        "{err}"
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(!backup_path(&path, 3).exists());
}

// spec: CRAWL-127
#[test]
fn backup_path_follows_naming_rule() {
    use std::path::Path;
    assert_eq!(
        backup_path(Path::new("/x/crawl.ospider"), 3),
        Path::new("/x/crawl.v3.bak.ospider")
    );
    assert_eq!(
        backup_path(Path::new("rel/mein.crawl.ospider"), 1),
        Path::new("rel/mein.crawl.v1.bak.ospider")
    );
}
