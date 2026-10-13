//! Anlegen und Wiederöffnen einer Crawl-Datei (OUT-230, CRAWL-127, CRAWL-132).

mod schema_common;

use os_model::{CrawlConfig, CrawlMode, CrawlState, RobotsMode};
use os_store::{CrawlFile, OpenMode, StorageProfile, StoreError, APPLICATION_ID, SCHEMA_VERSION};
use schema_common::*;

// spec: OUT-230
#[test]
fn create_sets_identity_wal_and_all_tables() {
    let dir = tempfile::tempdir().unwrap();
    let (path, file) = create(dir.path(), "a.ospider");
    assert_eq!(APPLICATION_ID, 1_330_860_100);
    let app_id: i64 = file
        .conn()
        .pragma_query_value(None, "application_id", |r| r.get(0))
        .unwrap();
    assert_eq!(app_id, 0x4F53_5044);
    let uv: i64 = file
        .conn()
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(uv, i64::from(SCHEMA_VERSION));
    assert_eq!(SCHEMA_VERSION, 1);
    let jm: String = file
        .conn()
        .pragma_query_value(None, "journal_mode", |r| r.get(0))
        .unwrap();
    assert_eq!(jm, "wal");
    let fk: i64 = file
        .conn()
        .pragma_query_value(None, "foreign_keys", |r| r.get(0))
        .unwrap();
    assert_eq!(fk, 0);
    assert_eq!(tables(&file), ALL_TABLES);
    assert_eq!(file.path(), path.as_path());
}

// spec: OUT-230
#[test]
fn schema_has_expected_indexes_and_columns() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = create(dir.path(), "a.ospider");
    let idx: Vec<String> = file
        .conn()
        .prepare("SELECT name FROM sqlite_master WHERE type = 'index' AND name NOT LIKE 'sqlite_%'")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    for name in [
        "urls_status",
        "urls_hash",
        "urls_canon",
        "links_source",
        "links_target",
        "img_page",
        "hreflang_target",
        "sitemap_entries_url",
        "queue_order",
        "url_flags_flag",
        "urls_change_seq",
    ] {
        assert!(idx.iter().any(|i| i == name), "Index {name} fehlt");
    }
    let cols: Vec<String> = file
        .conn()
        .prepare("SELECT name FROM pragma_table_info('urls')")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    for c in [
        "id",
        "url",
        "inlinks",
        "unique_inlinks",
        "self_inlinks",
        "inlinks_all",
        "img_inlinks",
        "title_1_px",
        "meta_desc_2_in_head",
        "canonical_self",
        "indexability_status",
        "change_seq",
    ] {
        assert!(cols.iter().any(|x| x == c), "Spalte urls.{c} fehlt");
    }
    // Eindeutigkeit der URL ist die Wahrheit für das Seen-Set.
    file.conn()
        .execute(
            "INSERT INTO urls (id, url, address, origin, is_internal, state, discovered_via, folder_depth, url_length) \
             VALUES (1, 'https://a.test/', 'https://a.test/', 'https://a.test', 1, 'queued', 'start', 0, 15)",
            [],
        )
        .unwrap();
    let dup = file.conn().execute(
        "INSERT INTO urls (id, url, address, origin, is_internal, state, discovered_via, folder_depth, url_length) \
         VALUES (2, 'https://a.test/', 'x', 'y', 1, 'queued', 'link', 0, 15)",
        [],
    );
    assert!(dup.is_err());
}

// spec: OUT-230, CRAWL-127
#[test]
fn reopen_reads_meta_and_config() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.ospider");
    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = 9;
    cfg.robots.mode = RobotsMode::Ignore;
    cfg.scope.exclude = vec!["\\?utm_".into()];
    let created = init(cfg.clone());
    {
        let f = CrawlFile::create(&path, &created, StorageProfile::Balanced).unwrap();
        let m = f.meta().unwrap();
        assert_eq!(m.crawl_id, created.crawl_id);
        assert_eq!(m.state, CrawlState::Running);
        assert_eq!(m.schema_version, 1);
    }
    for mode in [OpenMode::ReadOnly, OpenMode::ReadWrite] {
        let f = CrawlFile::open(&path, mode).unwrap();
        let m = f.meta().unwrap();
        assert_eq!(m.crawl_id, created.crawl_id);
        assert_eq!(m.mode, CrawlMode::Spider);
        assert_eq!(m.start_url.as_deref(), Some("https://site.test/"));
        assert_eq!(m.project, "Projekt");
        assert_eq!(m.name, "Testcrawl");
        assert_eq!(m.state, CrawlState::Running);
        assert_eq!(
            (m.end_reason, m.started_at, m.ended_at, m.heartbeat_at),
            (None, None, None, None)
        );
        assert_eq!(f.config().unwrap(), cfg);
        assert_eq!(
            f.get_meta("created_at").unwrap().as_deref(),
            Some("2026-10-13T07:35:00.000Z")
        );
        assert_eq!(f.get_meta("next_url_id").unwrap().as_deref(), Some("1"));
    }
}

// spec: CRAWL-132
#[test]
fn embedded_config_is_osconfig_text_without_secrets() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.auth.basic.push(os_model::BasicAuth {
        prefix: "https://a.test/".into(),
        username: "u".into(),
        secret_ref: "auth/a".into(),
    });
    let path = dir.path().join("c.ospider");
    let f = CrawlFile::create(&path, &init(cfg.clone()), StorageProfile::Balanced).unwrap();
    let text: String = f
        .conn()
        .query_row("SELECT osconfig FROM crawl_config WHERE id = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(CrawlConfig::from_toml_str(&text).unwrap(), cfg);
    assert!(text.contains("secret_ref = \"auth/a\""));
    // zweite Zeile ist nicht erlaubt (id = 1)
    assert!(f
        .conn()
        .execute(
            "INSERT INTO crawl_config (id, osconfig, updated_at) VALUES (2, '', '')",
            []
        )
        .is_err());
}

// spec: CRAWL-127
#[test]
fn set_config_replaces_embedded_config() {
    let dir = tempfile::tempdir().unwrap();
    let (path, f) = create(dir.path(), "a.ospider");
    let mut cfg = f.config().unwrap();
    assert_eq!(cfg, CrawlConfig::default());
    cfg.limits.max_urls = 77;
    cfg.title.max_chars = 50;
    f.set_config(&cfg).unwrap();
    assert_eq!(f.config().unwrap(), cfg);
    drop(f);
    let again = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    assert_eq!(again.config().unwrap(), cfg);
}

// spec: CRAWL-128
#[test]
fn set_meta_upserts_and_read_only_rejects_writes() {
    let dir = tempfile::tempdir().unwrap();
    let (path, f) = create(dir.path(), "a.ospider");
    f.set_meta("state", "Paused").unwrap();
    f.set_meta("end_reason", "stopped").unwrap();
    f.set_meta("heartbeat_at", "2026-10-13T07:36:00.500Z")
        .unwrap();
    let m = f.meta().unwrap();
    assert_eq!(m.state, CrawlState::Paused);
    assert_eq!(m.end_reason, Some(os_model::EndReason::Stopped));
    assert_eq!(m.heartbeat_at.unwrap().to_db(), "2026-10-13T07:36:00.500Z");
    drop(f);
    let ro = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    assert!(matches!(
        ro.set_meta("name", "x"),
        Err(StoreError::Sqlite(_))
    ));
    assert_eq!(ro.meta().unwrap().name, "Testcrawl");
}

// spec: CRAWL-128
#[test]
fn invalid_meta_is_reported_with_key() {
    let dir = tempfile::tempdir().unwrap();
    let (_, f) = create(dir.path(), "a.ospider");
    f.set_meta("state", "Zombie").unwrap();
    assert!(matches!(f.meta(), Err(StoreError::InvalidMeta { key, .. }) if key == "state"));
    f.set_meta("state", "Running").unwrap();
    f.conn()
        .execute("DELETE FROM crawl_meta WHERE key = 'project'", [])
        .unwrap();
    assert!(matches!(f.meta(), Err(StoreError::MissingMeta(k)) if k == "project"));
}

// spec: OUT-230
#[test]
fn create_never_overwrites_and_creates_missing_directories() {
    let dir = tempfile::tempdir().unwrap();
    let (path, f) = create(dir.path(), "a.ospider");
    drop(f);
    let err = CrawlFile::create(
        &path,
        &init(CrawlConfig::default()),
        StorageProfile::Balanced,
    );
    assert!(matches!(err, Err(StoreError::AlreadyExists(_))));
    let nested = dir.path().join("x").join("y").join("n.ospider");
    CrawlFile::create(
        &nested,
        &init(CrawlConfig::default()),
        StorageProfile::MinMemory,
    )
    .unwrap();
    assert!(nested.exists());
}

// spec: OUT-230
#[test]
fn list_mode_records_list_source() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("l.ospider");
    let mut i = init(CrawlConfig::default());
    i.mode = CrawlMode::List;
    i.start_url = None;
    i.list_source = Some("urls.txt".into());
    let f = CrawlFile::create(&path, &i, StorageProfile::Balanced).unwrap();
    let m = f.meta().unwrap();
    assert_eq!(m.mode, CrawlMode::List);
    assert_eq!(m.start_url, None);
    assert_eq!(
        f.get_meta("list_source").unwrap().as_deref(),
        Some("urls.txt")
    );
}

// spec: OUT-230
#[test]
fn change_seq_column_index_and_meta_start_at_zero() {
    let dir = tempfile::tempdir().unwrap();
    let (path, file) = create(dir.path(), "a.ospider");
    // urls.change_seq: INTEGER NOT NULL DEFAULT 0, indiziert über urls_change_seq.
    let (ty, notnull, default): (String, i64, Option<String>) = file
        .conn()
        .query_row(
            "SELECT type, \"notnull\", dflt_value FROM pragma_table_info('urls') WHERE name = 'change_seq'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        (ty.as_str(), notnull, default.as_deref()),
        ("INTEGER", 1, Some("0"))
    );
    let idx_cols: Vec<String> = file
        .conn()
        .prepare("SELECT name FROM pragma_index_info('urls_change_seq')")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(idx_cols, ["change_seq"]);
    file.conn()
        .execute(
            "INSERT INTO urls (id, url, address, origin, is_internal, state, discovered_via, folder_depth, url_length) \
             VALUES (1, 'https://a.test/', 'https://a.test/', 'https://a.test', 1, 'queued', 'start', 0, 15)",
            [],
        )
        .unwrap();
    let seq: i64 = file
        .conn()
        .query_row("SELECT change_seq FROM urls WHERE id = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(seq, 0);
    // crawl_meta.change_seq wird bei create mit 0 angelegt und von queries::change_seq gelesen.
    assert_eq!(file.get_meta("change_seq").unwrap().as_deref(), Some("0"));
    assert_eq!(os_store::queries::change_seq(file.conn()).unwrap(), 0);
    file.set_meta("change_seq", "7").unwrap();
    drop(file);
    let again = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    assert_eq!(os_store::queries::change_seq(again.conn()).unwrap(), 7);
    // Fehlt der Schlüssel, ist die Sequenz 0.
    let (_, f2) = create(dir.path(), "b.ospider");
    f2.conn()
        .execute("DELETE FROM crawl_meta WHERE key = 'change_seq'", [])
        .unwrap();
    assert_eq!(os_store::queries::change_seq(f2.conn()).unwrap(), 0);
    assert!(os_store::queries::changed_since(f2.conn(), 0, 10)
        .unwrap()
        .is_empty());
}
