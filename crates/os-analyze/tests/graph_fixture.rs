//! In-memory store fixtures for K16. No external site or reference data.
#![allow(dead_code)]
use os_model::{CrawlConfig, CrawlId, CrawlMode, Timestamp};
use os_store::{CrawlFile, CrawlInit, StorageProfile};

pub fn file() -> (tempfile::TempDir, CrawlFile) {
    let dir = tempfile::tempdir().unwrap();
    let file = CrawlFile::create(
        &dir.path().join("fx-ui-basic.ospider"),
        &CrawlInit {
            crawl_id: "01900000-0000-7000-8000-000000000001"
                .parse::<CrawlId>()
                .unwrap(),
            mode: CrawlMode::Spider,
            start_url: Some("https://site.test/".into()),
            list_source: None,
            project: "Fixture".into(),
            name: "fx-ui-basic".into(),
            config: CrawlConfig::default(),
            created_at: Timestamp::from_unix_millis(0),
        },
        StorageProfile::Balanced,
    )
    .unwrap();
    (dir, file)
}
pub fn url(file: &CrawlFile, id: i64, address: &str, html: bool) {
    file.conn().execute(
        "INSERT INTO urls(id, url, address, origin, is_internal, state, discovered_via, folder_depth, url_length,
         content_class, status_code, indexability, crawl_depth, word_count, response_time_ms)
         VALUES (?1, ?2, ?2, 'https://site.test', 1, 'done', 'Hyperlink', 0, length(?2), ?3, 200, 'Indexable', 0, ?4, 125)",
        rusqlite::params![id, address, if html { "HTML" } else { "Images" }, if id == 1 {50} else {2000}],
    ).unwrap();
}
pub fn edge(file: &CrawlFile, seq: i64, source: Option<i64>, target: i64) {
    file.conn().execute("INSERT INTO discovery_edges(seq, source_id, target_id, kind, depth) VALUES (?1, ?2, ?3, 'link', 0)",
        rusqlite::params![seq, source, target]).unwrap();
}
pub fn basic() -> (tempfile::TempDir, CrawlFile) {
    let (dir, file) = file();
    for (id, address, html) in [
        (1, "https://site.test/", true),
        (2, "https://site.test/a", true),
        (3, "https://site.test/b", true),
        (4, "https://site.test/c", true),
        (5, "https://site.test/d", true),
        (6, "https://site.test/logo.png", false),
    ] {
        url(&file, id, address, html);
    }
    edge(&file, 1, None, 1);
    edge(&file, 2, Some(1), 2);
    edge(&file, 3, Some(1), 3);
    edge(&file, 4, Some(2), 4);
    edge(&file, 5, Some(4), 5);
    edge(&file, 6, Some(3), 5); // Later discovery improves the path.
    edge(&file, 7, Some(3), 4); // Same depth: earlier seq 4 wins.
    edge(&file, 8, Some(5), 1); // Cycles never duplicate a node.
    edge(&file, 9, Some(1), 6);
    (dir, file)
}
pub fn link(
    file: &CrawlFile,
    source: i64,
    target: i64,
    anchor: &str,
    alt: &str,
    follow: bool,
    kind: &str,
) {
    file.conn().execute("INSERT INTO links(source_id, target_id, target_raw, link_type, position, anchor, alt_text, follow)
        VALUES (?1, ?2, '/target', ?3, 0, ?4, ?5, ?6)", rusqlite::params![source, target, kind, anchor, alt, follow]).unwrap();
}
