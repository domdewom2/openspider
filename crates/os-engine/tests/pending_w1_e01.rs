#[path = "core_support.rs"]
mod support;
use os_engine::testing::crawl_fixture_from;
use os_model::{CancelToken, CrawlConfig};
use support::*;

// spec: CRAWL-075
#[tokio::test]
#[ignore = "Klärung K2-68: N01-C15, Besitzer M2-E05 (worker.rs) und M2-N01b (Bodylimit os-fetch), max_page_kb = 0 unbegrenzt"]
async fn unlimited_page_size_does_not_truncate_the_body_at_zero_bytes() {
    let mut cfg = CrawlConfig::default();
    cfg.limits.max_page_kb = 0;
    let fx = crawl_fixture_from("crawl-m2-switches/crawl-resources", "/large", cfg).await;
    assert!(requested_paths(&fx, None).contains(&"/early".into()));
    assert_eq!(
        scalar(&fx, "SELECT size_bytes FROM urls WHERE url LIKE '%/large'"),
        8192
    );
    assert_eq!(scalar(&fx,"SELECT v_int FROM url_ext WHERE key = 'crawl.truncated' AND url_id = (SELECT id FROM urls WHERE url LIKE '%/large')"),0);
}

// spec: CRAWL-042 [P1]
#[tokio::test]
async fn media_transport_stops_after_64_kib_and_keeps_content_length() {
    let fx = crawl_fixture_from(
        "crawl-m2-switches/crawl-resources",
        "/video.mp4",
        CrawlConfig::default(),
    )
    .await;
    let request = fx
        .site
        .requests()
        .into_iter()
        .find(|r| r.path() == "/video.mp4")
        .unwrap();
    assert!(request.sent_bytes > 0);
    assert!(
        scalar(
            &fx,
            "SELECT transferred_bytes FROM urls WHERE url LIKE '%/video.mp4'"
        ) <= 128 * 1024
    );
    assert_eq!(
        scalar(
            &fx,
            "SELECT size_bytes FROM urls WHERE url LIKE '%/video.mp4'"
        ),
        5 * 1024 * 1024
    );
    assert_eq!(scalar(&fx,"SELECT v_int FROM url_ext WHERE key = 'crawl.truncated' AND url_id = (SELECT id FROM urls WHERE url LIKE '%/video.mp4')"),1);
}

// spec: CRAWL-094
#[tokio::test]
async fn self_refresh_recomputes_without_requests_when_the_respect_switch_changes() {
    let cfg = CrawlConfig::default();
    let mut fx = crawl_fixture_from(
        "crawl-m2-respect/crawl-indexability",
        "/self-refresh",
        cfg.clone(),
    )
    .await;
    let requests = fx.site.requests().len();
    let mut new = cfg.clone();
    new.advanced.respect_self_ref_meta_refresh = true;
    let outcome =
        os_engine::recompute::run_triggered(&mut fx.file, &cfg, &new, &CancelToken::default())
            .unwrap();
    assert!(!outcome.changed.is_empty());
    assert_eq!(
        scalar(
            &fx,
            "SELECT count(*) FROM urls WHERE indexability_status = 'Self Referencing Meta Refresh'"
        ),
        1
    );
    os_engine::recompute::run_triggered(&mut fx.file, &new, &cfg, &CancelToken::default()).unwrap();
    assert_eq!(
        scalar(
            &fx,
            "SELECT count(*) FROM urls WHERE indexability_status = 'Self Referencing Meta Refresh'"
        ),
        0
    );
    assert_eq!(fx.site.requests().len(), requests);
}
