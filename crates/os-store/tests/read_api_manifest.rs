//! Manifest der Crawl-Datei (OUT-233): Schlüssel, Regel `completeness`, `config_sha256`, migrierte Dateien, Geheimnisse.

mod store_common;

use std::collections::BTreeMap;
use std::path::Path;

use os_model::time::{Clock, SystemClock};
use os_model::CrawlConfig;
use os_store::meta::{sha256_hex, OPENSPIDER_VERSION};
use os_store::read_api::{self, MANIFEST_KEYS};
use os_store::{CrawlFile, OpenMode, StorageProfile};
use store_common::*;

fn manifest(path: &Path) -> BTreeMap<String, Option<String>> {
    let file = CrawlFile::open(path, OpenMode::ReadOnly).unwrap();
    file.manifest().unwrap().into_iter().collect()
}

fn get(m: &BTreeMap<String, Option<String>>, key: &str) -> Option<String> {
    m.get(key).unwrap_or_else(|| panic!("Schlüssel {key} fehlt")).clone()
}

// spec: OUT-233
#[test]
fn the_manifest_has_exactly_the_keys_of_the_spec_with_the_expected_values() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "m.ospider", StorageProfile::Balanced);
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    file.set_meta("state", "Completed").unwrap();
    file.set_meta("end_reason", "queue_empty").unwrap();
    file.set_meta("started_at", "2026-10-13T07:36:00.000Z").unwrap();
    file.set_meta("ended_at", "2026-10-13T07:41:30.250Z").unwrap();
    file.set_meta("owner_kind", "cli").unwrap();
    file.set_meta("analysis_state", "completed").unwrap();
    drop(file);

    let m = manifest(&path);
    let mut expected: Vec<&str> = MANIFEST_KEYS.to_vec();
    expected.sort_unstable();
    assert_eq!(m.keys().map(String::as_str).collect::<Vec<_>>(), expected);

    assert_eq!(get(&m, "format").as_deref(), Some("ospider"));
    assert_eq!(get(&m, "manifest_version").as_deref(), Some("1"));
    assert_eq!(get(&m, "read_api_version").as_deref(), Some("1"));
    assert_eq!(get(&m, "schema_version").as_deref(), Some("3"));
    assert_eq!(get(&m, "openspider_version").as_deref(), Some(OPENSPIDER_VERSION));
    assert_eq!(get(&m, "last_writer_version").as_deref(), Some(OPENSPIDER_VERSION));
    assert_eq!(get(&m, "writer_kind").as_deref(), Some("cli"));
    assert_eq!(get(&m, "name").as_deref(), Some("Testcrawl"));
    assert_eq!(get(&m, "project").as_deref(), Some("Projekt"));
    assert_eq!(get(&m, "mode").as_deref(), Some("spider"));
    assert_eq!(get(&m, "start_url").as_deref(), Some("https://site.test/"));
    assert_eq!(get(&m, "list_source"), None, "nicht zutreffend ist NULL");
    assert_eq!(
        get(&m, "started_at").as_deref(),
        Some("2026-10-13T07:36:00+00:00")
    );
    assert_eq!(
        get(&m, "ended_at").as_deref(),
        Some("2026-10-13T07:41:30+00:00")
    );
    assert_eq!(get(&m, "state").as_deref(), Some("Completed"));
    assert_eq!(get(&m, "end_reason").as_deref(), Some("queue_empty"));
    assert_eq!(get(&m, "completeness").as_deref(), Some("complete"));
    assert_eq!(get(&m, "analysis_state").as_deref(), Some("done"));
    assert_eq!(get(&m, "urls_total").as_deref(), Some("0"));
    assert_eq!(get(&m, "urls_crawled").as_deref(), Some("0"));
    assert!(get(&m, "crawl_id").is_some_and(|c| c.len() == 36));
    assert!(get(&m, "user_agent").is_some_and(|ua| ua.contains("OpenSpider/")));
    let sha = get(&m, "config_sha256").unwrap();
    assert_eq!(sha.len(), 64);
    assert!(sha.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
}

// spec: OUT-233
#[test]
fn completeness_follows_the_rule_for_every_state() {
    let now = SystemClock.now().to_db();
    let stale = "2020-01-01T00:00:00.000Z";
    // (state, end_reason, heartbeat_at, started_at, erwartete completeness, erwarteter state, ended_at gesetzt)
    let cases: &[(&str, Option<&str>, &str, &str, &str, bool)] = &[
        ("Completed", Some("queue_empty"), stale, "complete", "Completed", true),
        ("Completed", Some("limit_reached"), stale, "limited", "Completed", true),
        ("Completed", Some("stopped"), stale, "stopped", "Completed", true),
        ("Paused", None, stale, "paused", "Paused", false),
        ("Running", None, &now, "running", "Running", false),
        ("Running", None, stale, "interrupted", "Incomplete", false),
        ("Incomplete", None, stale, "interrupted", "Incomplete", true),
        ("Failed", None, stale, "failed", "Failed", true),
    ];
    for (state, reason, heartbeat, completeness, shown_state, has_end) in cases {
        let dir = tempfile::tempdir().unwrap();
        let path = new_crawl(dir.path(), "c.ospider", StorageProfile::Balanced);
        let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
        file.set_meta("state", state).unwrap();
        file.set_meta("heartbeat_at", heartbeat).unwrap();
        file.set_meta("started_at", stale).unwrap();
        file.set_meta("ended_at", "2026-10-13T07:41:30.000Z").unwrap();
        if let Some(reason) = reason {
            file.set_meta("end_reason", reason).unwrap();
        }
        drop(file);
        let m = manifest(&path);
        let label = format!("{state}/{reason:?}/{heartbeat}");
        assert_eq!(get(&m, "completeness").as_deref(), Some(*completeness), "{label}");
        assert_eq!(get(&m, "state").as_deref(), Some(*shown_state), "{label}");
        assert_eq!(get(&m, "ended_at").is_some(), *has_end, "ended_at bei {label}");
        if !has_end {
            // Solange der Crawl läuft oder pausiert, gibt es weder Ende noch Endgrund.
            assert_eq!(get(&m, "end_reason"), None, "{label}");
        }
    }
}

// spec: OUT-233
#[test]
fn config_sha256_matches_the_embedded_osconfig_and_follows_config_changes() {
    // Bekannte Prüfsummen (FIPS 180-4), unabhängig von der Datei.
    assert_eq!(
        sha256_hex("abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        sha256_hex(""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );

    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "h.ospider", StorageProfile::Balanced);
    let osconfig = |p: &Path| -> String {
        let file = CrawlFile::open(p, OpenMode::ReadOnly).unwrap();
        file.conn()
            .query_row("SELECT osconfig FROM q_config", [], |r| r.get(0))
            .unwrap()
    };
    let first = manifest(&path);
    assert_eq!(
        get(&first, "config_sha256").unwrap(),
        sha256_hex(&osconfig(&path))
    );

    // „Konfiguration übernehmen“ (set_config) ändert Konfiguration und Prüfsumme gemeinsam.
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = 3;
    cfg.user_agent.http = Some("MeinCrawler/1.0".into());
    cfg.analysis.auto_at_end = false;
    file.set_config(&cfg).unwrap();
    drop(file);
    let second = manifest(&path);
    assert_ne!(get(&first, "config_sha256"), get(&second, "config_sha256"));
    assert_eq!(
        get(&second, "config_sha256").unwrap(),
        sha256_hex(&osconfig(&path))
    );
    assert_eq!(get(&second, "user_agent").as_deref(), Some("MeinCrawler/1.0"));
    // Analyse nie gelaufen und nicht automatisch: `off`.
    assert_eq!(get(&second, "analysis_state").as_deref(), Some("off"));
}

// spec: OUT-233
#[test]
fn analysis_state_maps_the_internal_states() {
    for (internal, shown) in [
        (None, "pending"),
        (Some("running"), "pending"),
        (Some("incomplete"), "pending"),
        (Some("completed"), "done"),
        (Some("stale"), "stale"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = new_crawl(dir.path(), "a.ospider", StorageProfile::Balanced);
        if let Some(internal) = internal {
            let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
            file.set_meta("analysis_state", internal).unwrap();
        }
        assert_eq!(
            get(&manifest(&path), "analysis_state").as_deref(),
            Some(shown),
            "{internal:?}"
        );
    }
}

// spec: OUT-233
#[test]
fn migrated_files_have_a_complete_manifest_and_unknown_values_are_null() {
    for fixture in ["schema-v1.ospider", "schema-v2.ospider"] {
        let dir = tempfile::tempdir().unwrap();
        let path = copy_fixture(fixture, dir.path());
        drop(CrawlFile::open(&path, OpenMode::ReadWrite).unwrap());
        let m = manifest(&path);
        let mut expected: Vec<&str> = MANIFEST_KEYS.to_vec();
        expected.sort_unstable();
        assert_eq!(
            m.keys().map(String::as_str).collect::<Vec<_>>(),
            expected,
            "{fixture}"
        );
        assert_eq!(get(&m, "openspider_version"), None, "{fixture}: unbekannt = NULL");
        assert_eq!(get(&m, "writer_kind"), None, "{fixture}");
        assert_eq!(get(&m, "schema_version").as_deref(), Some("3"), "{fixture}");
        assert_eq!(get(&m, "read_api_version").as_deref(), Some("1"));
        assert_eq!(get(&m, "name").as_deref(), Some("Pausierter Crawl"));
        assert_eq!(get(&m, "completeness").as_deref(), Some("paused"));
        assert_eq!(get(&m, "urls_total").as_deref(), Some("42"));
        assert_eq!(get(&m, "urls_crawled").as_deref(), Some("25"));
        assert!(get(&m, "config_sha256").is_some(), "{fixture}: aus der Konfiguration abgeleitet");
        assert!(get(&m, "user_agent").is_some(), "{fixture}");
    }
}

// spec: OUT-233
#[test]
fn opening_for_writing_keeps_the_creator_version_and_updates_only_the_last_writer() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "w.ospider", StorageProfile::Balanced);
    // Eine ältere Version hat angelegt und zuletzt geschrieben.
    {
        let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
        file.set_meta("openspider_version", "0.0.9").unwrap();
        file.set_meta("last_writer_version", "0.0.9").unwrap();
    }
    assert_eq!(
        get(&manifest(&path), "last_writer_version").as_deref(),
        Some("0.0.9"),
        "Nur-Lesen ändert nichts"
    );
    drop(CrawlFile::open(&path, OpenMode::ReadWrite).unwrap());
    let m = manifest(&path);
    assert_eq!(get(&m, "openspider_version").as_deref(), Some("0.0.9"));
    assert_eq!(get(&m, "last_writer_version").as_deref(), Some(OPENSPIDER_VERSION));
}

// spec: OUT-233
// spec: OUT-234
#[test]
fn an_unknown_read_api_version_is_reported_clearly() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "v.ospider", StorageProfile::Balanced);
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    file.check_read_api().unwrap();
    // Eine Datei, deren Manifest eine unbekannte Hauptversion nennt.
    file.conn()
        .execute_batch(
            "DROP VIEW q_manifest; \
             CREATE VIEW q_manifest AS SELECT 'format' AS key, 'ospider' AS value \
             UNION ALL SELECT 'read_api_version', '99';",
        )
        .unwrap();
    let err = file.check_read_api().unwrap_err().to_string();
    assert!(err.contains("unbekannte Version der Leseschnittstelle"), "{err}");
    assert!(err.contains("99"), "{err}");
    // Ein falsches Format wird ebenso abgewiesen.
    file.conn()
        .execute_batch(
            "DROP VIEW q_manifest; \
             CREATE VIEW q_manifest AS SELECT 'format' AS key, 'andere' AS value \
             UNION ALL SELECT 'read_api_version', '1';",
        )
        .unwrap();
    assert!(file
        .check_read_api()
        .unwrap_err()
        .to_string()
        .contains("keine OpenSpider-Crawl-Datei"));
    assert_eq!(read_api::READ_API_VERSION, 1);
}

// spec: OUT-233
// spec: OUT-234
#[test]
fn no_secret_appears_in_any_view_output() {
    // Geheimnisse gehören in den Schlüsselbund (C13); die Konfiguration verweist nur darauf. Die Volltextsuche über
    // alle Sichtausgaben findet keine Zugangsdaten, auch wenn die Konfiguration Verweise auf Konten trägt.
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "g.ospider", StorageProfile::Balanced);
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    let mut all = String::new();
    for view in read_api::VIEWS {
        let mut stmt = file.conn().prepare(&format!("SELECT * FROM {view}")).unwrap();
        let n = stmt.column_count();
        let mut rows = stmt.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            for i in 0..n {
                if let Ok(Some(t)) = row.get::<_, Option<String>>(i) {
                    all.push_str(&t);
                    all.push('\n');
                }
            }
        }
    }
    let lower = all.to_ascii_lowercase();
    for needle in [
        "refresh_token",
        "access_token",
        "client_secret",
        "bearer ",
        "password =",
        "api_key =",
        "-----begin",
    ] {
        assert!(!lower.contains(needle), "gefunden: {needle}");
    }
}
