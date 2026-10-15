//! Crawl-Dateien (CRAWL-127, OUT-230): Fixtures, Fehlerbilder, `save_as`, Fortsetzen, Reparatur, Import.
//!
//! Die Fixtures liegen in `tests/fixtures/crawl-files/`; mit `UPDATE_FIXTURES=1 cargo test -p os-store
//! --test crawl_files regenerate_fixtures` werden sie neu erzeugt (danach `git add -f`, `*.ospider` steht in
//! `.gitignore`). `schema-v1.ospider` ist eine echte Datei im Schema v1 (von der Version vor M1-P32 geschrieben)
//! und wird **nicht** neu erzeugt: sie ist die Eingabe des Migrationstests (v1 → v2).

mod store_common;

use std::path::Path;

use os_model::time::{ManualClock, SeededIds};
use os_model::{CrawlConfig, CrawlState, Timestamp, UrlId};
use os_store::{
    files, queries, registry, schema, CrawlFile, CrawlSink, OpenMode, StorageProfile, StoreError,
};
use store_common::*;

fn checksum(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap()
}

fn companions(path: &Path) -> Vec<String> {
    ["-wal", "-shm", "-journal"]
        .iter()
        .map(|s| format!("{}{s}", path.display()))
        .filter(|p| Path::new(p).exists())
        .collect()
}

#[test]
fn regenerate_fixtures() {
    if std::env::var_os("UPDATE_FIXTURES").is_none() {
        return;
    }
    write_crawl_file_fixtures(&fixture_dir());
}

/// Die eingecheckten Fixtures stimmen mit dem Generator überein (Inhalt, nicht Bytes).
#[test]
fn committed_fixtures_match_the_generator() {
    let work = tempfile::tempdir().unwrap();
    write_crawl_file_fixtures(work.path());
    let generated = work.path().join("schema-v2.ospider");
    let committed_dir = tempfile::tempdir().unwrap();
    let committed = copy_fixture("schema-v2.ospider", committed_dir.path());
    let a = CrawlFile::open(&generated, OpenMode::ReadOnly).unwrap();
    let b = CrawlFile::open(&committed, OpenMode::ReadOnly).unwrap();
    assert_eq!(
        dump(&a),
        dump(&b),
        "Fixture schema-v2.ospider veraltet (UPDATE_FIXTURES=1)"
    );
    for name in ["corrupt.ospider", "not-a-crawl.ospider"] {
        assert_eq!(
            checksum(&work.path().join(name)),
            checksum(&fixture_dir().join(name)),
            "Fixture {name} veraltet"
        );
    }
}

// spec: OUT-230
#[test]
fn schema_v2_fixture_is_a_valid_crawl_file_with_embedded_config() {
    let dir = tempfile::tempdir().unwrap();
    let path = copy_fixture("schema-v2.ospider", dir.path());
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    assert_eq!(
        schema::application_id(file.conn()).unwrap(),
        schema::APPLICATION_ID
    );
    assert_eq!(
        schema::user_version(file.conn()).unwrap(),
        schema::SCHEMA_VERSION
    );
    let meta = file.meta().unwrap();
    assert_eq!(meta.name, "Pausierter Crawl");
    assert_eq!(meta.state, CrawlState::Paused);
    assert_eq!(count(&file, "SELECT COUNT(*) FROM urls"), 42);
    assert_eq!(file.config().unwrap(), CrawlConfig::default());
    assert!(file.integrity_check().unwrap());
    // Zähler stimmen mit der Neuberechnung überein.
    assert!(os_store::counters::verify_all(file.conn())
        .unwrap()
        .is_empty());
}

// spec: CRAWL-127
#[test]
fn newer_schema_version_is_rejected_and_file_stays_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let path = copy_fixture("schema-v99.ospider", dir.path());
    let before = checksum(&path);
    for mode in [OpenMode::ReadOnly, OpenMode::ReadWrite] {
        let err = CrawlFile::open(&path, mode)
            .err()
            .expect("v99 muss abgelehnt werden");
        match &err {
            StoreError::NewerVersion {
                found,
                supported,
                generator,
                ..
            } => {
                assert_eq!((*found, *supported), (99, 2));
                assert!(
                    generator.contains("9.9.0"),
                    "Erzeuger in der Meldung: {generator}"
                );
            }
            other => panic!("unerwarteter Fehler: {other:?}"),
        }
        let text = err.to_string();
        assert!(
            text.contains("schema-v99.ospider") && text.contains("aktualisieren"),
            "{text}"
        );
    }
    assert_eq!(checksum(&path), before, "Prüfsumme vorher = nachher");
    assert!(companions(&path).is_empty());
    // Auch Verwaltung und Reparatur lassen die Datei unangetastet.
    assert!(matches!(
        files::repair_copy(&path),
        Err(StoreError::NewerVersion { .. })
    ));
    assert!(matches!(
        files::verify(&path),
        Err(StoreError::NewerVersion { .. })
    ));
    assert_eq!(checksum(&path), before);
}

// spec: CRAWL-127, OUT-230
#[test]
fn png_renamed_to_ospider_is_reported_with_file_name_and_cause() {
    let dir = tempfile::tempdir().unwrap();
    let path = copy_fixture("not-a-crawl.ospider", dir.path());
    let before = checksum(&path);
    let err = CrawlFile::open(&path, OpenMode::ReadWrite).err().unwrap();
    assert!(matches!(err, StoreError::NotACrawlFile { .. }), "{err:?}");
    let text = err.to_string();
    assert!(
        text.contains("not-a-crawl.ospider"),
        "Dateiname fehlt: {text}"
    );
    assert!(
        text.contains("keine OpenSpider-Crawl-Datei"),
        "Ursache fehlt: {text}"
    );
    assert!(matches!(
        files::verify(&path),
        Err(StoreError::NotACrawlFile { .. })
    ));
    assert!(matches!(
        files::repair_copy(&path),
        Err(StoreError::NotACrawlFile { .. })
    ));
    assert!(matches!(
        registry::read_listing(&path),
        Err(StoreError::NotACrawlFile { .. })
    ));
    assert_eq!(checksum(&path), before, "keine Veränderung der Datei");
    assert!(
        !files::repaired_path(&path).exists(),
        "keine Reparatur-Kopie für fremde Dateien"
    );
}

// spec: CRAWL-127
#[test]
fn truncated_file_is_reported_as_corrupt_with_file_name() {
    let dir = tempfile::tempdir().unwrap();
    let path = copy_fixture("corrupt.ospider", dir.path());
    let before = checksum(&path);
    let full = std::fs::metadata(fixture_dir().join("schema-v2.ospider"))
        .unwrap()
        .len();
    assert_eq!(
        before.len() as u64,
        full / 2,
        "Fixture ist nach 50 % abgeschnitten"
    );
    let err = CrawlFile::open(&path, OpenMode::ReadWrite).err().unwrap();
    assert!(matches!(err, StoreError::Corrupt { .. }), "{err:?}");
    let text = err.to_string();
    assert!(
        text.contains("corrupt.ospider") && text.contains("beschädigt"),
        "{text}"
    );
    assert_eq!(checksum(&path), before);
    assert!(companions(&path).is_empty());
}

// spec: OUT-230
#[test]
fn foreign_sqlite_file_is_rejected_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fremd.ospider");
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE t (x); INSERT INTO t VALUES (1);")
            .unwrap();
        conn.pragma_update_and_check(None, "journal_mode", "DELETE", |_| Ok(()))
            .unwrap();
    }
    let before = checksum(&path);
    let err = CrawlFile::open(&path, OpenMode::ReadWrite).err().unwrap();
    assert!(matches!(err, StoreError::NotACrawlFile { .. }), "{err:?}");
    assert!(err.to_string().contains("keine OpenSpider-Crawl-Datei"));
    assert_eq!(checksum(&path), before);
}

// spec: OUT-230, CRAWL-127
#[test]
fn save_as_is_self_contained_and_contents_are_identical() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("live.ospider");
    build_paused_crawl(&source);

    // Ein weiterer Batch bleibt im WAL (kein Checkpoint), die Kopie muss ihn enthalten.
    let mut sink = sink(&source, StorageProfile::Balanced);
    let mut batch = os_model::WriteBatch::default();
    batch.pages.push(page(26, vec![hyperlink(27, 0)]));
    sink.write(batch).unwrap();
    assert!(
        std::fs::metadata(format!("{}-wal", source.display())).map_or(0, |m| m.len()) > 0,
        "Quelle hat ungesicherte WAL-Daten"
    );

    let copy = dir.path().join("kopie").join("save.ospider");
    sink.file().save_as(&copy).unwrap();
    assert!(
        companions(&copy).is_empty(),
        "Kopie ohne -wal/-shm: {:?}",
        companions(&copy)
    );
    assert!(!Path::new(&format!("{}.saving", copy.display())).exists());

    // Nur die Hauptdatei an einen anderen Ort kopieren und dort öffnen (kein Begleitdatei-Bedarf).
    let elsewhere = tempfile::tempdir().unwrap();
    let moved = elsewhere.path().join("moved.ospider");
    std::fs::copy(&copy, &moved).unwrap();
    let opened = CrawlFile::open(&moved, OpenMode::ReadOnly).unwrap();
    assert_eq!(
        schema::application_id(opened.conn()).unwrap(),
        schema::APPLICATION_ID
    );
    assert_eq!(
        schema::user_version(opened.conn()).unwrap(),
        schema::SCHEMA_VERSION
    );
    assert!(opened.integrity_check().unwrap());
    assert_eq!(
        count(&opened, "SELECT COUNT(*) FROM links WHERE source_id = 26"),
        1
    );
    assert_eq!(
        dump(&opened),
        dump(sink.file()),
        "identische Tabelleninhalte"
    );
    assert_eq!(opened.config().unwrap(), sink.file().config().unwrap());

    // Ein vorhandenes Ziel wird nie überschrieben.
    assert!(matches!(
        sink.file().save_as(&copy),
        Err(StoreError::AlreadyExists(_))
    ));
}

// spec: OUT-230
#[test]
fn paused_crawl_with_15_queued_urls_resumes_exactly_those() {
    let dir = tempfile::tempdir().unwrap();
    let path = copy_fixture("schema-v2.ospider", dir.path());
    let saved = dir.path().join("gespeichert.ospider");
    CrawlFile::open(&path, OpenMode::ReadWrite)
        .unwrap()
        .save_as(&saved)
        .unwrap();
    // "App neu starten": frisch öffnen.
    let file = CrawlFile::open(&saved, OpenMode::ReadWrite).unwrap();
    let state = file.resume_state().unwrap();
    let ids: Vec<i64> = state.queue.iter().map(|q| q.id.0).collect();
    assert_eq!(ids, (26..=40).collect::<Vec<_>>());
    assert!(state
        .queue
        .iter()
        .all(|q| q.depth == Some(2) && q.priority == 0));
    assert_eq!(state.next_url_id, 43);
    assert_eq!(state.next_seq, 41);
    // Fortsetzen: die Warteschlange abarbeiten ergibt einen leeren Zustand.
    let mut sink = os_store::SqliteSink::open(&saved, StorageProfile::Balanced).unwrap();
    let mut batch = os_model::WriteBatch::default();
    for q in &state.queue {
        batch.queue.push(os_model::QueueOp::Pop { id: q.id });
        batch.pages.push(page(q.id.0, vec![]));
    }
    sink.write(batch).unwrap();
    assert!(file.resume_state().unwrap().queue.is_empty());
    assert_eq!(
        count(&file, "SELECT COUNT(*) FROM urls WHERE state = 'done'"),
        40
    );
}

// spec: OUT-230
#[test]
fn damaged_file_is_detected_and_repaired_into_a_new_copy() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("gross.ospider");
    CrawlFile::create(&source, &init(11), StorageProfile::Balanced).unwrap();
    {
        let mut sink = sink(&source, StorageProfile::Balanced);
        for chunk in 0..4 {
            let first = chunk * 500 + 1;
            let mut batch = batch_with_urls((first..first + 500).map(|i| (i, true)));
            for i in first..first + 500 {
                let links = (1..=8)
                    .map(|k| hyperlink((i + k * 37) % 2000 + 1, u32::try_from(k).unwrap()))
                    .collect();
                batch.pages.push(page(i, links));
            }
            sink.write(batch).unwrap();
        }
        sink.finish().unwrap();
    }
    let saved = dir.path().join("gesichert.ospider");
    CrawlFile::open(&source, OpenMode::ReadOnly)
        .unwrap()
        .save_as(&saved)
        .unwrap();
    let original_urls = count(
        &CrawlFile::open(&saved, OpenMode::ReadOnly).unwrap(),
        "SELECT COUNT(*) FROM urls",
    );
    assert_eq!(original_urls, 2000);

    // Zufällige Bytes überschreiben (deterministisch): je ein Blatt der Tabellen `urls` und `links` aus der
    // Mitte der Datei – die Datei bleibt als SQLite-Datei erkennbar, die Tabellen sind teilweise unlesbar.
    let (page_size, victims) = {
        let conn = rusqlite::Connection::open(&saved).unwrap();
        let page_size: usize = conn
            .pragma_query_value(None, "page_size", |r| r.get(0))
            .unwrap();
        let mut victims = Vec::new();
        for table in ["urls", "links"] {
            let leaves: Vec<usize> = conn
                .prepare("SELECT pageno FROM dbstat WHERE name = ?1 AND pagetype = 'leaf' ORDER BY pageno")
                .unwrap()
                .query_map([table], |r| r.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            assert!(leaves.len() >= 3, "{table} hat mehrere Blätter");
            victims.push(leaves[leaves.len() / 2]);
        }
        (page_size, victims)
    };
    let mut bytes = std::fs::read(&saved).unwrap();
    let mut x = 0x2545_F491_4F6C_DD1Du64;
    for page_no in victims {
        for b in &mut bytes[(page_no - 1) * page_size..page_no * page_size] {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            *b = x.to_le_bytes()[0];
        }
    }
    std::fs::write(&saved, &bytes).unwrap();
    let damaged = checksum(&saved);

    let err = files::verify(&saved).expect_err("Beschädigung wird erkannt");
    assert!(matches!(err, StoreError::Corrupt { .. }), "{err:?}");
    assert!(err.to_string().contains("gesichert.ospider"));

    let report = files::repair_copy(&saved).unwrap();
    assert_eq!(
        report.repaired,
        dir.path().join("gesichert.repaired.ospider")
    );
    assert!(report.repaired.exists());
    assert!(
        report.integrity_ok,
        "reparierte Datei besteht die Integritätsprüfung"
    );
    assert!(report.usable);
    assert_eq!(checksum(&saved), damaged, "Quelle bleibt unverändert");
    let repaired = CrawlFile::open(&report.repaired, OpenMode::ReadOnly).unwrap();
    assert!(repaired.integrity_check().unwrap());
    let urls = count(&repaired, "SELECT COUNT(*) FROM urls");
    assert!(
        (1900..2000).contains(&urls),
        "nur die Zeilen der zerstörten Seite fehlen: {urls}"
    );
    eprintln!(
        "Reparatur: {urls} von 2000 URLs gerettet, {} Zeilen übersprungen: {:?}",
        report.lost_rows(),
        report
            .tables
            .iter()
            .filter(|t| t.lost > 0)
            .collect::<Vec<_>>()
    );
    let urls_table = report.tables.iter().find(|t| t.table == "urls").unwrap();
    assert_eq!(i64::try_from(urls_table.copied).unwrap(), urls);
    assert!(report.lost_rows() > 0, "unlesbare Zeilen werden gezählt");
    let links = report.tables.iter().find(|t| t.table == "links").unwrap();
    assert!(links.copied > 0 && links.lost > 0, "{links:?}");
    assert_eq!(repaired.meta().unwrap().name, "Testcrawl");
    // Ein zweiter Versuch überschreibt die Reparatur-Kopie nicht.
    assert!(matches!(
        files::repair_copy(&saved),
        Err(StoreError::AlreadyExists(_))
    ));
}

// spec: CRAWL-127
#[test]
fn import_copies_into_the_crawl_folder_and_assigns_a_new_id_on_clash() {
    let work = tempfile::tempdir().unwrap();
    let crawls = work.path().join("crawls");
    let src = copy_fixture("schema-v2.ospider", work.path());
    let original_id = CrawlFile::open(&src, OpenMode::ReadOnly)
        .unwrap()
        .meta()
        .unwrap()
        .crawl_id;
    let ids = SeededIds::new(5);
    let clock = ManualClock::new(Timestamp::from_db("2026-10-14T10:00:00.000Z").unwrap());
    let before = checksum(&src);

    let first = registry::import_file(&src, &crawls, &ids, &clock).unwrap();
    assert_eq!(first, registry::path_for(&crawls, &original_id));
    assert!(companions(&first).is_empty());
    let second = registry::import_file(&src, &crawls, &ids, &clock).unwrap();
    assert_ne!(second, first, "zweiter Import erhält eine neue Kennung");
    let second_meta = CrawlFile::open(&second, OpenMode::ReadOnly)
        .unwrap()
        .meta()
        .unwrap();
    assert_ne!(second_meta.crawl_id, original_id);
    assert_eq!(second, registry::path_for(&crawls, &second_meta.crawl_id));
    assert_eq!(checksum(&src), before, "Quelle unverändert");

    let first_file = CrawlFile::open(&first, OpenMode::ReadOnly).unwrap();
    let second_file = CrawlFile::open(&second, OpenMode::ReadOnly).unwrap();
    let urls = |f: &CrawlFile| count(f, "SELECT COUNT(*) FROM urls");
    assert_eq!((urls(&first_file), urls(&second_file)), (42, 42));
    assert_eq!(registry::list_crawls(&crawls).unwrap().len(), 2);

    // Eine Nicht-Crawl-Datei mit Endung .ospider: Fehlermeldung, nichts wird kopiert.
    let fake = copy_fixture("not-a-crawl.ospider", work.path());
    let other = work.path().join("andere");
    let err = registry::import_file(&fake, &other, &ids, &clock)
        .err()
        .unwrap();
    assert!(matches!(err, StoreError::NotACrawlFile { .. }));
    assert!(!other.exists() || std::fs::read_dir(&other).unwrap().next().is_none());
}

// spec: CRAWL-127
#[test]
fn opened_crawl_keeps_the_embedded_config_until_it_is_adopted() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "cfg.ospider", StorageProfile::Balanced);
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    let mut other = CrawlConfig::default();
    other.speed.threads = 3;
    // Die aktuelle Konfiguration der App ändert die Datei nicht; erst `set_config` ("übernehmen") tut es.
    assert_eq!(file.config().unwrap(), CrawlConfig::default());
    file.set_config(&other).unwrap();
    assert_eq!(file.config().unwrap().speed.threads, 3);
    let reopened = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    assert_eq!(reopened.config().unwrap().speed.threads, 3);
    let _ = UrlId(0);
    let _ = queries::change_seq(reopened.conn());
}

// spec: CRAWL-127, CRAWL-124
#[test]
fn set_config_with_clock_stamps_the_time_from_the_clock() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "clk.ospider", StorageProfile::Balanced);
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    let updated_at = |f: &CrawlFile| -> String {
        f.conn()
            .query_row(
                "SELECT updated_at FROM crawl_config WHERE id = 1",
                [],
                |r| r.get(0),
            )
            .unwrap()
    };
    let start = Timestamp::from_db("2026-10-13T08:00:00.000Z").unwrap();
    let clock = ManualClock::new(start);
    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = 4;
    file.set_config_with_clock(&cfg, &clock).unwrap();
    assert_eq!(updated_at(&file), start.to_db());
    assert_eq!(file.config().unwrap().speed.threads, 4);

    // Gleiche Uhr, gleiche Konfiguration: derselbe Zeitstempel; nach `advance` der spätere.
    clock.advance(std::time::Duration::from_secs(90));
    cfg.speed.threads = 5;
    file.set_config_with_clock(&cfg, &clock).unwrap();
    assert_eq!(
        updated_at(&file),
        start.plus(std::time::Duration::from_secs(90)).to_db()
    );
    assert_eq!(file.config().unwrap().speed.threads, 5);

    // `set_config` behält seine Signatur und stempelt mit der Systemuhr (nicht 2026-10-13T08:01:30).
    file.set_config(&cfg).unwrap();
    assert_ne!(
        updated_at(&file),
        start.plus(std::time::Duration::from_secs(90)).to_db()
    );
    assert_eq!(file.config().unwrap().speed.threads, 5);
}

#[test]
fn effective_state_marks_crawls_without_heartbeat_as_incomplete() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "hb.ospider", StorageProfile::Balanced);
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    let clock = ManualClock::new(Timestamp::from_db("2026-10-13T08:00:00.000Z").unwrap());
    file.set_meta("heartbeat_at", "2026-10-13T08:00:00.000Z")
        .unwrap();
    assert_eq!(file.effective_state(&clock).unwrap(), CrawlState::Running);
    clock.advance(std::time::Duration::from_secs(10));
    assert_eq!(
        file.effective_state(&clock).unwrap(),
        CrawlState::Running,
        "genau 10 s: noch am Leben"
    );
    clock.advance(std::time::Duration::from_millis(1));
    assert_eq!(
        file.effective_state(&clock).unwrap(),
        CrawlState::Incomplete
    );
    // Andere Zustände bleiben unverändert.
    file.set_meta("state", "Paused").unwrap();
    assert_eq!(file.effective_state(&clock).unwrap(), CrawlState::Paused);
    file.set_meta("state", "Completed").unwrap();
    assert_eq!(file.effective_state(&clock).unwrap(), CrawlState::Completed);
    // Ein frisch angelegter Crawl ohne Heartbeat zählt über `created_at`.
    let fresh = new_crawl(dir.path(), "fresh.ospider", StorageProfile::Balanced);
    let fresh = CrawlFile::open(&fresh, OpenMode::ReadOnly).unwrap();
    let near = ManualClock::new(Timestamp::from_db("2026-10-13T07:35:05.000Z").unwrap());
    assert_eq!(fresh.effective_state(&near).unwrap(), CrawlState::Running);
    assert_eq!(
        fresh.effective_state(&clock).unwrap(),
        CrawlState::Incomplete
    );
}

#[test]
fn url_key_iteration_covers_all_urls_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let path = copy_fixture("schema-v2.ospider", dir.path());
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    let mut seen = Vec::new();
    let n = file
        .for_each_url_key(&mut |id, url| {
            seen.push((id.0, url.to_string()));
            Ok(())
        })
        .unwrap();
    assert_eq!(n, 42);
    assert_eq!(seen[0], (1, "https://site.test/p1".to_string()));
    assert!(seen.windows(2).all(|w| w[0].0 < w[1].0));
    let page = file.url_keys_page(Some(UrlId(40)), 10).unwrap();
    assert_eq!(
        page.iter().map(|(i, _)| i.0).collect::<Vec<_>>(),
        vec![41, 42]
    );
    assert_eq!(file.url_keys_page(None, 5).unwrap().len(), 5);
}

// spec: PERF-002
#[test]
fn a_schema_v1_file_migrates_to_v2_with_identical_counters() {
    let dir = tempfile::tempdir().unwrap();
    let path = copy_fixture("schema-v1.ospider", dir.path());
    let before = std::fs::read(&path).unwrap();

    // Nur lesen: die Migration ist nötig, die Datei bleibt unverändert.
    match CrawlFile::open(&path, OpenMode::ReadOnly) {
        Err(StoreError::MigrationRequired {
            found, supported, ..
        }) => assert_eq!((found, supported), (1, 2)),
        other => panic!("MigrationRequired erwartet: {:?}", other.err()),
    }
    assert_eq!(std::fs::read(&path).unwrap(), before);

    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    assert_eq!(schema::user_version(file.conn()).unwrap(), 2);
    // Sicherungskopie in der alten Version
    let backup = os_store::backup_path(&path, 1);
    assert!(backup.exists());
    {
        // die Sicherung bleibt ein Schema-v1-Crawl (der Dateikopf unterscheidet sich vom Original: WAL-Modus)
        let old = rusqlite::Connection::open(&backup).unwrap();
        assert_eq!(schema::user_version(&old).unwrap(), 1);
        let urls: i64 = old
            .query_row("SELECT COUNT(*) FROM urls", [], |r| r.get(0))
            .unwrap();
        assert_eq!(urls, 42);
        assert!(old
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE name = 'url_counters'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .is_err());
    }

    // Die zehn bisherigen Zähler stehen unverändert in der neuen Tabelle ...
    let differing = count(
        &file,
        "SELECT COUNT(*) FROM urls u JOIN url_counters c ON c.url_id = u.id WHERE \
         u.inlinks <> c.inlinks OR u.unique_inlinks <> c.unique_inlinks OR u.self_inlinks <> c.self_inlinks \
         OR u.inlinks_all <> c.inlinks_all OR u.img_inlinks <> c.img_inlinks OR u.outlinks <> c.outlinks \
         OR u.unique_outlinks <> c.unique_outlinks OR u.self_outlinks <> c.self_outlinks \
         OR u.external_outlinks <> c.external_outlinks OR u.unique_external_outlinks <> c.unique_external_outlinks",
    );
    assert_eq!(differing, 0);
    assert_eq!(
        count(&file, "SELECT COUNT(*) FROM url_counters"),
        count(&file, "SELECT COUNT(*) FROM urls")
    );
    assert!(count(&file, "SELECT SUM(inlinks) FROM url_counters") > 0);
    // ... die zwei neuen Zähler sind aus den Links berechnet: alle 25 abgerufenen Seiten verlinken die Startseite.
    assert_eq!(
        count(
            &file,
            "SELECT html_2xx_sources FROM url_counters WHERE url_id = 1"
        ),
        25
    );
    assert_eq!(
        count(&file, "SELECT SUM(image_link_inlinks) FROM url_counters"),
        0
    );
    // Gegenprobe über die vollständige Neuberechnung
    assert!(os_store::counters::verify_all(file.conn())
        .unwrap()
        .is_empty());
    // Trigger: neue URLs erhalten ihre Zeile
    file.conn()
        .execute(
            "INSERT INTO urls (id, url, address, origin, is_internal, state, discovered_via, folder_depth, url_length) \
             VALUES (500, 'https://neu.test/', 'https://neu.test/', 'https://neu.test', 1, 'queued', 'link', 0, 17)",
            [],
        )
        .unwrap();
    assert_eq!(
        count(
            &file,
            "SELECT COUNT(*) FROM url_counters WHERE url_id = 500"
        ),
        1
    );
}
