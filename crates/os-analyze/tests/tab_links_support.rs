#![allow(dead_code)]
use os_model::{CrawlConfig, CrawlMode, IdSource, SeededIds, Timestamp};
use os_store::{CrawlFile, CrawlInit, StorageProfile};

pub fn file() -> (tempfile::TempDir, CrawlFile) {
    let dir = tempfile::tempdir().unwrap();
    let now = Timestamp::from_db("2026-10-18T12:00:00.000Z").unwrap();
    let init = CrawlInit {
        crawl_id: SeededIds::new(8).new_crawl_id(now),
        mode: CrawlMode::Spider,
        start_url: Some("https://site.test/1".into()),
        list_source: None,
        project: String::new(),
        name: String::new(),
        config: CrawlConfig::default(),
        created_at: now,
    };
    let file = CrawlFile::create(
        &dir.path().join("links.ospider"),
        &init,
        StorageProfile::Balanced,
    )
    .unwrap();
    (dir, file)
}
pub fn sql(file: &CrawlFile, sql: &str) {
    file.conn().execute_batch(sql).unwrap();
}
pub fn nodes(file: &CrawlFile, count: usize) {
    for id in 1..=count {
        file.conn().execute("INSERT INTO urls (id,url,address,origin,is_internal,state,discovered_via,folder_depth,url_length,content_class,status_code,indexability) VALUES (?1,?2,?2,'https://site.test',1,'done','link',0,1,'HTML',200,'Indexable')", rusqlite::params![id, format!("https://site.test/{id}")]).unwrap();
    }
}
pub fn fixture(file: &CrawlFile) {
    nodes(file, 12);
    sql(
        file,
        include_str!("../../../tests/fixtures/tech-m2-links/filters.sql"),
    );
}
pub fn ids(file: &CrawlFile, cfg: &CrawlConfig, filter: &str) -> Vec<i64> {
    os_analyze::views::query_view(
        file,
        cfg,
        &os_analyze::views::ViewQuery::new("Links", filter),
    )
    .unwrap()
    .rows
    .iter()
    .map(|r| {
        if let os_model::Value::Text(s) = &r[0] {
            s.rsplit('/').next().unwrap().parse().unwrap()
        } else {
            panic!()
        }
    })
    .collect()
}
