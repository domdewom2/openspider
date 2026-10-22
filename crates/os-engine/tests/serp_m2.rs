use os_engine::{serp_input, start_crawl, testing, CrawlRequest, CrawlSource};
use os_model::{CrawlConfig, CrawlMode};
use os_store::{CrawlFile, OpenMode};

// spec: CRAWL-031
// spec: OUT-098 [M2]
#[tokio::test]
async fn offline_upload_preserves_unicode_measures_and_roundtrips_summary_columns() {
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
    let mut out = csv::Writer::from_writer(Vec::new());
    out.write_record([
        "Address",
        "Title 1",
        "Title 1 Length",
        "Title 1 Pixel Width",
        "Meta Description 1",
        "Meta Description 1 Length",
        "Meta Description 1 Pixel Width",
    ])
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
        out.write_record([
            row.0.clone(),
            row.1.clone(),
            row.2.to_string(),
            row.3.to_string(),
            row.4.clone(),
            row.5.to_string(),
            row.6.to_string(),
        ])
        .unwrap();
    }
    assert_eq!(
        serp_input::parse(&String::from_utf8(out.into_inner().unwrap()).unwrap()).unwrap(),
        rows
    );
}

// spec: CRAWL-031
#[test]
fn aliases_missing_headers_and_text_uploads() {
    let root = os_testsite::fixtures_root().join("crawl-m2-list");
    let de = serp_input::read(&root.join("serp-de.txt")).unwrap();
    assert_eq!(de[0].title, "Grüße 👩‍💻 中文");
    let err = serp_input::read(&root.join("serp-missing-header.csv")).unwrap_err();
    assert!(err.to_string().contains("Description"));
    let csv = serp_input::read(&root.join("serp.csv")).unwrap();
    assert_eq!(csv.len(), 5);
    for name in ["serp.xls", "serp.xlsx"] {
        assert_eq!(serp_input::read(&root.join(name)).unwrap(), csv, "{name}");
    }
}
