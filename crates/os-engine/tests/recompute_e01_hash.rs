use os_engine::testing::crawl_fixture;
use os_model::{CancelToken, CrawlConfig};

// spec: ONPAGE-143 [M2]
#[tokio::test]
async fn missing_stored_html_requires_recrawl_and_never_triggers_requests() {
    let mut off = CrawlConfig::default();
    off.extraction.hash = false;
    let mut fx = crawl_fixture("crawl-m2-switches/crawl-resources", off.clone()).await;
    let requests = fx.site.requests().len();
    let mut on = off.clone();
    on.extraction.hash = true;
    let result =
        os_engine::recompute::run_triggered(&mut fx.file, &off, &on, &CancelToken::default())
            .unwrap();
    assert!(result.changed.is_empty());
    assert_eq!(result.ran, ["M2-E01"]);
    assert_eq!(fx.site.requests().len(), requests);
    assert_eq!(
        fx.file
            .conn()
            .query_row(
                "SELECT count(*) FROM urls WHERE hash_md5 IS NOT NULL",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
}

// spec: ONPAGE-143 [M2]
#[tokio::test]
async fn stored_html_restores_original_hashes_without_requests_and_stamps_the_change_feed() {
    let mut cfg = CrawlConfig::default();
    cfg.extraction.store_html = true;
    let mut fx = crawl_fixture("crawl-m2-switches/crawl-resources", cfg.clone()).await;
    let mut stmt = fx
        .file
        .conn()
        .prepare("SELECT id, hash_md5 FROM urls WHERE hash_md5 IS NOT NULL ORDER BY id")
        .unwrap();
    let hashes: Vec<(i64, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    drop(stmt);
    let requests = fx.site.requests().len();
    // Simuliert zuvor ohne Hash erfasste Seiten, deren Roh-HTML vorliegt.
    fx.file
        .conn()
        .execute("UPDATE urls SET hash_md5 = NULL", [])
        .unwrap();
    let mut off = cfg.clone();
    off.extraction.hash = false;
    let changed =
        os_engine::recompute::run_triggered(&mut fx.file, &off, &cfg, &CancelToken::default())
            .unwrap();
    assert!(!changed.changed.is_empty());
    let seq: String = fx.file.get_meta("change_seq").unwrap().unwrap();
    assert!(
        fx.file
            .conn()
            .query_row(
                "SELECT count(*) FROM urls WHERE change_seq = ?1",
                [seq.parse::<i64>().unwrap()],
                |r| r.get::<_, i64>(0)
            )
            .unwrap()
            > 0
    );
    // Abschalten lässt historische Erhebungen bestehen (CRAWL-061).
    assert!(
        os_engine::recompute::run_triggered(&mut fx.file, &cfg, &off, &CancelToken::default())
            .unwrap()
            .changed
            .is_empty()
    );
    for (id, hash) in hashes {
        let actual: String = fx
            .file
            .conn()
            .query_row("SELECT hash_md5 FROM urls WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(actual, hash);
    }
    assert_eq!(fx.site.requests().len(), requests);
    let cancel = CancelToken::default();
    cancel.cancel();
    assert!(matches!(
        os_engine::recompute::e01::run(&mut fx.file, &cfg, &off, &cancel),
        Err(os_engine::EngineError::Cancelled)
    ));
}
