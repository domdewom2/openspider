//! Lokale Store-Fixture für K03 (kein HTTP, kein Referenzmaterial).
#![allow(dead_code)]
use os_model::{CrawlConfig, CrawlMode, IdSource, SeededIds, Timestamp};
use os_store::{CrawlFile, CrawlInit, StorageProfile};
use rusqlite::params;

pub fn create(dir: &std::path::Path) -> CrawlFile {
    let at = Timestamp::from_unix_millis(1_760_000_000_000);
    CrawlFile::create(
        &dir.join("aggregates.ospider"),
        &CrawlInit {
            crawl_id: SeededIds::new(3).new_crawl_id(at),
            mode: CrawlMode::Spider,
            start_url: Some("https://site.test/".into()),
            list_source: None,
            project: "K03".into(),
            name: "K03".into(),
            config: CrawlConfig::default(),
            created_at: at,
        },
        StorageProfile::Balanced,
    )
    .unwrap()
}

#[allow(clippy::too_many_arguments)]
pub fn insert(
    file: &CrawlFile,
    id: i64,
    url: &str,
    depth: Option<u32>,
    html: bool,
    internal: bool,
    status: Option<u16>,
    ms: Option<u32>,
) {
    file.conn()
        .execute(
            "INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,crawl_depth,
        folder_depth,url_length,content_class,indexability,status_code,response_time_ms,change_seq)
        VALUES(?1,?2,?2,'https://site.test',?3,'done','hyperlink',?4,0,?5,?6,'Indexable',?7,?8,1)",
            params![
                id,
                url,
                internal,
                depth,
                url.len() as i64,
                if html { "HTML" } else { "Images" },
                status,
                ms
            ],
        )
        .unwrap();
    file.set_meta("change_seq", "1").unwrap();
}

pub fn depth_fixture(file: &CrawlFile) {
    for d in 0..=12 {
        insert(
            file,
            d + 1,
            &format!("https://site.test/a/b/{d}.html?x=1#part"),
            Some(d as u32),
            true,
            true,
            Some(if d == 4 { 404 } else { 200 }),
            Some(200),
        );
    }
    file.conn()
        .execute(
            "UPDATE urls SET indexability='Non-Indexable' WHERE crawl_depth=3",
            [],
        )
        .unwrap();
}
