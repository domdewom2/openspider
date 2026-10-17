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
                assert_eq!((*found, *supported), (99, os_store::SCHEMA_VERSION));
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

/// Hauptdatei, deren Kopf mehr Seiten angibt, als die Datei hat: der Zustand nach einem harten Abbruch mitten im
/// WAL-Checkpoint (die neue Seitenzahl steht schon im Kopf, die Seiten sind noch nicht alle in der Hauptdatei).
/// Die `-wal`-Datei enthält den Rest; SQLite stellt die Datei beim Öffnen daraus wieder her.
fn crash_copy_during_checkpoint(dir: &std::path::Path) -> (std::path::PathBuf, u64) {
    let (path, live) = create(dir, "live.ospider");
    let conn = live.conn();
    conn.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
    conn.execute_batch("CREATE TABLE filler (a BLOB);").unwrap();
    {
        let tx = conn.unchecked_transaction().unwrap();
        for _ in 0..300 {
            tx.execute("INSERT INTO filler VALUES (zeroblob(4000))", [])
                .unwrap();
        }
        tx.commit().unwrap();
    }
    let pages: u64 = conn
        .query_row("PRAGMA page_count", [], |r| r.get(0))
        .unwrap();

    // Kopie des laufenden Zustands (Hauptdatei + WAL), wie ihn ein Abbruch hinterlässt.
    let crash = dir.join("crash.ospider");
    let wal_of = |p: &std::path::Path| {
        let mut name = p.as_os_str().to_owned();
        name.push("-wal");
        std::path::PathBuf::from(name)
    };
    fs::copy(&path, &crash).unwrap();
    fs::copy(wal_of(&path), wal_of(&crash)).unwrap();
    assert!(fs::metadata(wal_of(&crash)).unwrap().len() > 0);
    drop(live);

    // Kopf: neue Seitenzahl, „version-valid-for“ gleich dem Änderungszähler (gültige Größenangabe).
    let mut head = bytes(&crash);
    head[28..32].copy_from_slice(&u32::try_from(pages).unwrap().to_be_bytes());
    let counter: [u8; 4] = head[24..28].try_into().unwrap();
    head[92..96].copy_from_slice(&counter);
    fs::write(&crash, &head).unwrap();
    let page_size = u64::from(u16::from_be_bytes([head[16], head[17]]));
    assert!(
        fs::metadata(&crash).unwrap().len() < pages * page_size,
        "Testaufbau: die Hauptdatei muss kürzer sein als der Kopf angibt"
    );
    (crash, pages)
}

// spec: CRAWL-125
#[test]
fn main_file_shorter_than_its_header_opens_when_a_wal_exists() {
    let dir = tempfile::tempdir().unwrap();
    let (crash, _) = crash_copy_during_checkpoint(dir.path());
    for mode in [OpenMode::ReadWrite, OpenMode::ReadOnly] {
        let file = CrawlFile::open(&crash, mode).expect("mit -wal-Datei muss das Öffnen gelingen");
        assert!(file.integrity_check().unwrap());
        let rows: i64 = file
            .conn()
            .query_row("SELECT COUNT(*) FROM filler", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 300, "alle Zeilen stammen aus der -wal-Datei");
    }
}

// spec: CRAWL-125
#[test]
fn main_file_shorter_than_its_header_is_still_rejected_without_a_wal() {
    let dir = tempfile::tempdir().unwrap();
    let (crash, _) = crash_copy_during_checkpoint(dir.path());
    // erst die Begleitdateien entfernen (öffnen und schließen legt sie neu an), dann prüfen
    let (alone, _) = (dir.path().join("alone.ospider"), ());
    fs::copy(&crash, &alone).unwrap();
    let wal_of_alone = dir.path().join("alone.ospider-wal");
    assert!(!wal_of_alone.exists(), "Testaufbau: keine -wal-Datei");
    let err = CrawlFile::open(&alone, OpenMode::ReadWrite)
        .err()
        .expect("ohne -wal-Datei bleibt die Datei abgeschnitten");
    assert!(matches!(err, StoreError::Corrupt { .. }), "{err}");
    assert!(err.to_string().contains("abgeschnitten"), "{err}");
    // eine leere -wal-Datei ändert daran nichts (SQLite verwirft sie)
    fs::write(&wal_of_alone, b"").unwrap();
    let err = CrawlFile::open(&alone, OpenMode::ReadOnly).err().unwrap();
    assert!(matches!(err, StoreError::Corrupt { .. }), "{err}");
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
