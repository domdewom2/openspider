use os_model::{CancelToken, CrawlConfig};
// spec: ONPAGE-140 [M2]
#[tokio::test]
#[ignore = "wartet auf M2-E01"]
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
    let previous = count(&fx.file);
    let changed =
        os_engine::recompute::d02::run(&mut fx.file, &old, &new, &CancelToken::new()).unwrap();
    assert!(!changed.is_empty());
    assert!(count(&fx.file) < previous);
    assert_eq!(fx.site.total_requests(), before);
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(matches!(
        os_engine::recompute::d02::run(&mut fx.file, &old, &new, &cancel),
        Err(os_engine::EngineError::Cancelled)
    ));
}
