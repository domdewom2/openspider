use os_model::{CancelToken, CrawlConfig};
// spec: ONPAGE-140 [M2]
#[tokio::test]
async fn changed_area_recomputes_word_count_from_stored_html_without_requests() {
    let mut cfg = CrawlConfig::default();
    cfg.extraction.store_html = true;
    cfg.duplicates.near_enabled = true;
    let mut fx = os_engine::testing::crawl_fixture("onpage-content", cfg).await;
    let old = fx.file.config().unwrap();
    let mut new = old.clone();
    new.content.area.include_elements = vec!["main".into()];
    let before = fx.site.total_requests();
    let count = |file: &os_store::CrawlFile| {
        file.conn()
            .query_row(
                "SELECT word_count FROM urls WHERE address LIKE '%/header-aside.html'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
    };
    let id: i64 = fx
        .file
        .conn()
        .query_row(
            "SELECT id FROM urls WHERE address LIKE '%/header-aside.html'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let html = include_str!("../../../tests/fixtures/onpage-m2-content/area.html")
        .replace("</body>", "<aside>Some extra words</aside></body>");
    fx.file.conn().execute("INSERT OR REPLACE INTO page_html(url_id,encoding,body,compression) VALUES (?1,'UTF-8',?2,NULL)",(id,html.as_bytes())).unwrap();
    fx.file
        .conn()
        .execute("UPDATE urls SET word_count=17 WHERE id=?1", [id])
        .unwrap();
    assert_eq!(count(&fx.file), 17);
    let changed =
        os_engine::recompute::d02::run(&mut fx.file, &old, &new, &CancelToken::new()).unwrap();
    assert!(!changed.is_empty());
    assert_eq!(count(&fx.file), 14);
    assert_eq!(fx.site.total_requests(), before);
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(matches!(
        os_engine::recompute::d02::run(&mut fx.file, &old, &new, &cancel),
        Err(os_engine::EngineError::Cancelled)
    ));
}
