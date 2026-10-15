//! Hilfen für die Writer-, Abfrage- und Dateitests (P07).
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use os_model::time::{IdSource, SeededIds};
use os_model::{
    ContentClass, CrawlConfig, CrawlMode, DiscoveryKind, FetchFacts, Indexability, LinkOrigin,
    LinkRow, LinkType, NewUrl, PageWrite, Timestamp, UrlId, UrlState, WriteBatch,
};
use os_store::{CrawlFile, CrawlInit, CrawlSink, SqliteSink, StorageProfile};

pub fn created_at() -> Timestamp {
    Timestamp::from_db("2026-10-13T07:35:00.000Z").unwrap()
}

pub fn init(seed: u64) -> CrawlInit {
    CrawlInit {
        crawl_id: SeededIds::new(seed).new_crawl_id(created_at()),
        mode: CrawlMode::Spider,
        start_url: Some("https://site.test/".into()),
        list_source: None,
        project: "Projekt".into(),
        name: "Testcrawl".into(),
        config: CrawlConfig::default(),
        created_at: created_at(),
    }
}

/// Legt eine Crawl-Datei an und gibt ihren Pfad zurück (die Anlegeverbindung wird geschlossen).
pub fn new_crawl(dir: &Path, name: &str, profile: StorageProfile) -> PathBuf {
    let path = dir.join(name);
    CrawlFile::create(&path, &init(42), profile).unwrap();
    path
}

pub fn sink(path: &Path, profile: StorageProfile) -> SqliteSink {
    SqliteSink::open(path, profile).unwrap()
}

/// Neue URL `https://site.test/<id>`; intern, Zustand `queued`.
pub fn new_url(id: i64, internal: bool) -> NewUrl {
    let host = if internal { "site.test" } else { "other.test" };
    let url = format!("https://{host}/p{id}");
    NewUrl {
        id: UrlId(id),
        address: url.clone(),
        origin: format!("https://{host}"),
        url_length: u32::try_from(url.len()).unwrap(),
        url,
        is_internal: internal,
        in_tables: true,
        discovered_via: DiscoveryKind::Link,
        crawl_depth: Some(1),
        folder_depth: 0,
        state: UrlState::Queued,
        not_crawled_reason: None,
    }
}

pub fn url_of(id: i64, internal: bool) -> String {
    new_url(id, internal).url
}

pub fn link(target: Option<i64>, link_type: LinkType, position: u32) -> LinkRow {
    LinkRow {
        target_id: target.map(UrlId),
        target_raw: target.map_or("mailto:x@y.test".to_string(), |t| format!("/p{t}")),
        link_type,
        position,
        anchor: Some(format!("Anker {position}")),
        alt_text: None,
        follow: true,
        rel: None,
        target_attr: None,
        path_type: None,
        link_path: None,
        link_position: None,
        link_origin: LinkOrigin::Html,
        in_head: Some(false),
    }
}

pub fn hyperlink(target: i64, position: u32) -> LinkRow {
    link(Some(target), LinkType::Hyperlink, position)
}

pub fn page(id: i64, links: Vec<LinkRow>) -> PageWrite {
    PageWrite {
        url_id: UrlId(id),
        replace: false,
        fetch: FetchFacts {
            crawl_timestamp: created_at(),
            status_code: 200,
            status: "OK".into(),
            content_type: Some("text/html".into()),
            content_class: ContentClass::Html,
            size_bytes: 1000,
            ..FetchFacts::default()
        },
        page: None,
        indexability: Indexability::Indexable,
        indexability_status: None,
        links,
        images: vec![],
        canonicals: vec![],
        directives: vec![],
        refresh: vec![],
        hreflang: vec![],
        pagination: vec![],
        headers: vec![],
    }
}

pub fn batch_with_urls(ids: impl IntoIterator<Item = (i64, bool)>) -> WriteBatch {
    WriteBatch {
        new_urls: ids.into_iter().map(|(id, i)| new_url(id, i)).collect(),
        ..WriteBatch::default()
    }
}

pub fn count(file: &CrawlFile, sql: &str) -> i64 {
    file.conn().query_row(sql, [], |r| r.get(0)).unwrap()
}

pub fn change_seq_of(file: &CrawlFile, id: i64) -> i64 {
    file.conn()
        .query_row("SELECT change_seq FROM urls WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .unwrap()
}

pub fn meta_change_seq(file: &CrawlFile) -> i64 {
    file.get_meta("change_seq")
        .unwrap()
        .unwrap()
        .parse()
        .unwrap()
}

// ------------------------------------------------------------------------------------------------------
// Fixtures `tests/fixtures/crawl-files/` (CRAWL-127, OUT-230)
// ------------------------------------------------------------------------------------------------------

/// Verzeichnis der Fixtures.
pub fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join("crawl-files")
}

/// Kopiert eine Fixture in ein Arbeitsverzeichnis (die Tests öffnen nie die eingecheckte Datei selbst).
pub fn copy_fixture(name: &str, dir: &Path) -> PathBuf {
    let dest = dir.join(name);
    std::fs::copy(fixture_dir().join(name), &dest).unwrap();
    dest
}

/// Ein pausierter Crawl: 40 interne URLs (1–25 abgerufen, 26–40 mit 15 Einträgen in der Warteschlange),
/// zwei externe Ziele (41, 42). Seite `i` verlinkt `i+1`, `i+2` und die Startseite.
pub fn build_paused_crawl(path: &Path) {
    let mut init = init(7);
    init.name = "Pausierter Crawl".into();
    CrawlFile::create(path, &init, StorageProfile::Balanced).unwrap();
    let mut sink = sink(path, StorageProfile::Balanced);

    let mut batch = batch_with_urls((1..=42).map(|i| (i, i <= 40)));
    for i in 1..=25 {
        let mut links = vec![hyperlink(i + 1, 0), hyperlink(i + 2, 1), hyperlink(1, 2)];
        if i % 5 == 0 {
            links.push(hyperlink(41 + (i / 5) % 2, 3));
        }
        batch.pages.push(page(i, links));
    }
    for i in 26..=40 {
        batch.queue.push(os_model::QueueOp::Push {
            id: UrlId(i),
            seq: u64::try_from(i).unwrap(),
            depth: Some(2),
            priority: 0,
        });
    }
    batch.meta = vec![
        ("state".into(), "Paused".into()),
        ("started_at".into(), "2026-10-13T07:36:00.000Z".into()),
        ("paused_at".into(), "2026-10-13T07:40:00.000Z".into()),
        ("heartbeat_at".into(), "2026-10-13T07:39:59.000Z".into()),
    ];
    sink.write(batch).unwrap();
    sink.finish().unwrap();
}

/// Die 70 Bytes eines gültigen 1×1-PNG (für `not-a-crawl.ospider`).
pub const PNG_BYTES: [u8; 70] = [
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0x64, 0x60, 0xf8, 0x5f,
    0x0f, 0x00, 0x02, 0x87, 0x01, 0x80, 0xeb, 0x47, 0xba, 0x92, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

/// Erzeugt die Fixtures in `out` aus `build_paused_crawl`: `schema-v2` (aktuelles Schema), davon abgeleitet `schema-v99`
/// und `corrupt`, dazu `not-a-crawl`. `schema-v1.ospider` (echte Datei der Version vor M1-P32, Eingabe des
/// Migrationstests) wird nicht neu erzeugt.
pub fn write_crawl_file_fixtures(out: &Path) {
    std::fs::create_dir_all(out).unwrap();
    let work = tempfile::tempdir().unwrap();
    let source = work.path().join("source.ospider");
    build_paused_crawl(&source);

    // schema-v2: eigenständige Kopie (VACUUM INTO), ohne Begleitdateien.
    let v2 = out.join("schema-v2.ospider");
    let _ = std::fs::remove_file(&v2);
    CrawlFile::open(&source, os_store::OpenMode::ReadOnly)
        .unwrap()
        .save_as(&v2)
        .unwrap();

    // schema-v99: künstlich hohe Version.
    let v99 = out.join("schema-v99.ospider");
    std::fs::copy(&v2, &v99).unwrap();
    {
        let conn = rusqlite::Connection::open(&v99).unwrap();
        conn.pragma_update(None, "user_version", 99).unwrap();
        conn.execute(
            "UPDATE crawl_meta SET value = '99' WHERE key = 'schema_version'",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE crawl_meta SET value = 'OpenSpider 9.9.0' WHERE key = 'generator'",
            [],
        )
        .unwrap();
        conn.pragma_update_and_check(None, "journal_mode", "DELETE", |_| Ok(()))
            .unwrap();
    }

    // corrupt: nach 50 % abgeschnitten.
    let bytes = std::fs::read(&v2).unwrap();
    std::fs::write(out.join("corrupt.ospider"), &bytes[..bytes.len() / 2]).unwrap();

    // not-a-crawl: PNG-Inhalt unter der Endung .ospider.
    std::fs::write(out.join("not-a-crawl.ospider"), PNG_BYTES).unwrap();
}

/// Inhalt aller Tabellen als Text (sortiert), für den Vergleich zweier Dateien.
pub fn dump(file: &CrawlFile) -> String {
    let conn = file.conn();
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let mut out = String::new();
    for table in tables {
        let mut stmt = conn.prepare(&format!("SELECT * FROM {table}")).unwrap();
        let columns = stmt.column_count();
        let mut lines: Vec<String> = stmt
            .query_map([], |r| {
                let mut cells = Vec::new();
                for i in 0..columns {
                    let v: rusqlite::types::Value = r.get(i)?;
                    cells.push(format!("{v:?}"));
                }
                Ok(cells.join("|"))
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        lines.sort();
        out.push_str(&format!("## {table}\n{}\n", lines.join("\n")));
    }
    out
}
