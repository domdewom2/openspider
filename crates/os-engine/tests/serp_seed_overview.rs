use os_engine::{serp_input, start_crawl, testing, CrawlRequest, CrawlSource};
use os_model::{CrawlConfig, CrawlMode};
use os_store::{CrawlFile, OpenMode};

// spec: CRAWL-031
// spec: OUT-098 [M2]
#[tokio::test]
async fn serp_writer_preserves_drafts_populates_counters_and_overview() {
    let rows = serp_input::parse(include_str!(
        "../../../tests/fixtures/crawl-m2-list/serp.csv"
    ))
    .unwrap();
    assert_eq!(rows[4].url, "Zeile 6");
    let site = os_testsite::TestSite::start("crawl-m2-list").await.unwrap();
    let mut cfg = CrawlConfig::default();
    // Even sitemap discovery and JavaScript settings must not cause any network/render work.
    cfg.rendering.mode = os_model::RenderingMode::JavaScript;
    cfg.sitemaps.urls = vec!["http://site.test/sitemap.xml".into()];
    let deps = testing::test_deps();
    let fetcher = testing::site_fetcher(&site, &cfg, &deps);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("serp.ospider");
    let summary = start_crawl(
        fetcher,
        CrawlRequest {
            config: cfg,
            source: CrawlSource::Serp {
                rows: rows.clone(),
                label: "serp.csv".into(),
            },
            file: path.clone(),
            project: "test".into(),
            name: "serp".into(),
        },
        deps,
    )
    .await
    .unwrap()
    .wait()
    .await
    .unwrap();
    assert_eq!(summary.crawled, 0);
    assert!(site.requests().is_empty());
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    assert_eq!(file.meta().unwrap().mode, CrawlMode::Serp);
    let mut stmt = file.conn().prepare("SELECT address,title_1,title_1_len,title_1_px,meta_desc_1,meta_desc_1_len,meta_desc_1_px FROM urls ORDER BY id").unwrap();
    let data = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, u32>(2)?,
                r.get::<_, u32>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, u32>(5)?,
                r.get::<_, u32>(6)?,
            ))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    for (source, row) in rows.iter().zip(&data) {
        assert_eq!(row.1, source.title);
        assert_eq!(row.4, source.description);
        assert_eq!(row.2, os_parse::text::char_len(&source.title));
        assert_eq!(row.3, os_parse::pixel::width_px(&source.title, 20.0, false));
        assert_eq!(row.5, os_parse::text::char_len(&source.description));
        assert_eq!(
            row.6,
            os_parse::pixel::width_px(&source.description, 14.0, false)
        );
    }
    assert_eq!(data.len(), rows.len());
    for (source, row) in rows.iter().zip(&data) {
        assert_eq!(row.0, source.url);
    }
    let count: u64 = file
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM urls u JOIN url_counters c ON c.url_id=u.id
         WHERE c.change_seq=u.change_seq AND c.change_seq>0 AND c.inlinks=0 AND c.outlinks=0
         AND u.crawl_timestamp IS NULL AND u.size_bytes IS NULL
         AND u.transferred_bytes IS NULL AND u.total_transferred_bytes IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, rows.len() as u64);
    let overview = os_store::queries::app_support::overview_summary(file.conn()).unwrap();
    for key in [
        "Total URLs Encountered",
        "Total URLs Displayed",
        "Total Internal URLs",
        "Total Internal Indexable URLs",
    ] {
        assert_eq!(
            overview.iter().find(|(name, _)| name == key).unwrap().1,
            rows.len() as u64
        );
    }
    assert_eq!(
        file.get_meta("next_url_id").unwrap().unwrap(),
        (rows.len() + 1).to_string()
    );
}
