//! Abweisen fremder, neuerer und beschädigter Dateien (OUT-230, CRAWL-127).

mod schema_common;

use std::fs;

use os_store::{CrawlFile, OpenMode, StoreError};
use rusqlite::Connection;
use schema_common::*;

fn bytes(p: &std::path::Path) -> Vec<u8> {
    fs::read(p).unwrap()
}

// spec: OUT-230
#[test]
fn foreign_sqlite_file_is_rejected_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fremd.ospider");
    {
        let c = Connection::open(&path).unwrap();
        c.execute_batch("CREATE TABLE t (a INTEGER); INSERT INTO t VALUES (1);")
            .unwrap();
    }
    let before = bytes(&path);
    for mode in [OpenMode::ReadOnly, OpenMode::ReadWrite] {
        let err = CrawlFile::open(&path, mode).err().unwrap();
        assert!(matches!(err, StoreError::NotACrawlFile { .. }), "{err}");
        assert!(err.to_string().contains("keine OpenSpider-Crawl-Datei"));
        assert!(err.to_string().contains("fremd.ospider"));
    }
    assert_eq!(
        bytes(&path),
        before,
        "fremde Datei darf nicht verändert werden"
    );
    assert!(!dir.path().join("fremd.ospider-wal").exists());
}

// spec: OUT-230, CRAWL-127
#[test]
fn non_sqlite_files_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    // PNG-Inhalt unter .ospider-Endung
    let png = dir.path().join("not-a-crawl.ospider");
    let png_bytes: Vec<u8> = [
        &[0x89u8, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A][..],
        &[7u8; 5000][..],
    ]
    .concat();
    fs::write(&png, &png_bytes).unwrap();
    let empty = dir.path().join("leer.ospider");
    fs::write(&empty, b"").unwrap();
    let short = dir.path().join("kurz.ospider");
    fs::write(&short, b"SQLite").unwrap();
    for p in [&png, &empty, &short] {
        let err = CrawlFile::open(p, OpenMode::ReadWrite).err().unwrap();
        assert!(
            matches!(err, StoreError::NotACrawlFile { .. }),
            "{p:?}: {err}"
        );
    }
    assert_eq!(bytes(&png), png_bytes);
    assert!(matches!(
        CrawlFile::open(
            &dir.path().join("gibt-es-nicht.ospider"),
            OpenMode::ReadOnly
        ),
        Err(StoreError::NotFound(_))
    ));
}

// spec: CRAWL-127
#[test]
fn newer_schema_version_is_rejected_without_touching_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let (path, f) = create(dir.path(), "v99.ospider");
    drop(f);
    {
        let c = Connection::open(&path).unwrap();
        c.pragma_update(None, "user_version", 99).unwrap();
    }
    // Eine alte Sicherung darf nicht entstehen; Prüfsumme (hier: Bytes) bleibt gleich.
    let before = bytes(&path);
    for mode in [OpenMode::ReadOnly, OpenMode::ReadWrite] {
        let err = CrawlFile::open(&path, mode).err().unwrap();
        match &err {
            StoreError::NewerVersion {
                found,
                supported,
                generator,
                ..
            } => {
                assert_eq!((*found, *supported), (99, 1));
                assert!(generator.starts_with("openspider "), "{generator}");
            }
            other => panic!("unerwartet: {other}"),
        }
        assert!(
            err.to_string().contains("bitte OpenSpider aktualisieren"),
            "{err}"
        );
    }
    assert_eq!(bytes(&path), before);
    // Keine Sicherungskopie; leere -wal/-shm-Dateien eines Nur-Lese-Zugriffs sind unschädlich.
    let leftovers: Vec<_> = fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| !n.ends_with("-wal") && !n.ends_with("-shm"))
        .collect();
    assert_eq!(
        leftovers,
        ["v99.ospider"],
        "unerwartete Dateien: {leftovers:?}"
    );
}

// spec: OUT-230, CRAWL-127
#[test]
fn truncated_file_is_reported_as_corrupt() {
    let dir = tempfile::tempdir().unwrap();
    let (path, f) = create(dir.path(), "corrupt.ospider");
    drop(f);
    let len = fs::metadata(&path).unwrap().len();
    assert!(len > 20_000, "Testdatei zu klein: {len}");
    let half = fs::OpenOptions::new().write(true).open(&path).unwrap();
    half.set_len(len / 2).unwrap();
    drop(half);
    let err = CrawlFile::open(&path, OpenMode::ReadWrite).err().unwrap();
    assert!(matches!(err, StoreError::Corrupt { .. }), "{err}");
    let msg = err.to_string();
    assert!(
        msg.contains("corrupt.ospider") && msg.contains("beschädigt"),
        "{msg}"
    );

    // Header-Rest: nur 50 Bytes einer echten Datei
    let tiny = dir.path().join("tiny.ospider");
    let (p2, f2) = create(dir.path(), "src.ospider");
    drop(f2);
    fs::write(&tiny, &bytes(&p2)[..50]).unwrap();
    assert!(matches!(
        CrawlFile::open(&tiny, OpenMode::ReadOnly),
        Err(StoreError::Corrupt { .. })
    ));
}

// spec: OUT-230
#[test]
fn incomplete_schema_is_reported_as_corrupt() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("halb.ospider");
    {
        let c = Connection::open(&path).unwrap();
        c.pragma_update(None, "application_id", os_store::APPLICATION_ID)
            .unwrap();
        c.execute_batch("CREATE TABLE x (a)").unwrap();
    }
    assert!(matches!(
        CrawlFile::open(&path, OpenMode::ReadWrite),
        Err(StoreError::Corrupt { .. })
    ));
}

// spec: OUT-230
#[test]
fn file_is_self_contained_after_close() {
    let dir = tempfile::tempdir().unwrap();
    let (path, f) = create(dir.path(), "a.ospider");
    f.set_meta("name", "Eins").unwrap();
    drop(f);
    // Kopie ohne Begleitdateien lässt sich öffnen und enthält alles.
    let copy = dir.path().join("kopie.ospider");
    fs::copy(&path, &copy).unwrap();
    let c = CrawlFile::open(&copy, OpenMode::ReadOnly).unwrap();
    assert_eq!(c.meta().unwrap().name, "Eins");
}
