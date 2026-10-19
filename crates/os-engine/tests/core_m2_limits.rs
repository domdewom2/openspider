#[path = "core_support.rs"]
mod support;
use os_engine::testing::{crawl_fixture, crawl_fixture_from};
use os_model::CrawlConfig;
use support::*;

// spec: CRAWL-069
// spec: CRAWL-070
// spec: CRAWL-073
#[tokio::test]
async fn limits_keep_outlinks_hide_targets_and_count_skips_once() {
    let mut cfg = CrawlConfig::default();
    cfg.limits.max_folder_depth = Some(1);
    cfg.limits.max_query_params = Some(2);
    cfg.limits.max_url_length = 100;
    let fx = crawl_fixture("crawl-m2-limits/crawl-folders", cfg).await;
    let reqs = requested_paths(&fx, None);
    for p in ["/a/", "/query?a=1&&b=2", "/query?a", "/query?a=1;b=2"] {
        assert!(reqs.contains(&p.into()), "{p}");
    }
    for p in ["/a/b/", "/query?a=1&b=2&c=3"] {
        assert!(!reqs.contains(&p.into()));
    }
    for key in [
        "skipped_folder_depth",
        "skipped_query_params",
        "skipped_url_length",
    ] {
        assert_eq!(
            scalar(
                &fx,
                &format!("SELECT value FROM crawl_counters WHERE key = '{key}'")
            ),
            1
        );
    }
    assert_eq!(scalar(&fx,"SELECT count(*) FROM links l JOIN urls u ON u.id = l.target_id WHERE u.not_crawled_reason = 'url_limit' AND u.in_tables = 0 AND u.status_code IS NULL"),3);
    let mut cfg = CrawlConfig::default();
    cfg.limits.max_query_params = Some(0);
    let fx = crawl_fixture("crawl-m2-limits/crawl-folders", cfg).await;
    assert!(!requested_paths(&fx, None).iter().any(|p| p.contains('?')));
}

// spec: CRAWL-074
#[tokio::test]
async fn link_cap_counts_metadata_and_resources_in_document_order_and_zero_is_unlimited() {
    for limit in [0, 2, 3] {
        let mut cfg = CrawlConfig::default();
        cfg.limits.max_links_per_url = limit;
        let fx = crawl_fixture_from("crawl-m2-limits/crawl-folders", "/many.html", cfg).await;
        assert_eq!(
            scalar(&fx, "SELECT count(*) FROM links"),
            if limit == 0 { 4 } else { limit as i64 }
        );
        assert_eq!(scalar(&fx,"SELECT v_int FROM url_ext WHERE key = 'crawl.links_truncated' AND url_id = (SELECT id FROM urls WHERE url LIKE '%/many.html')"),i64::from(limit != 0));
        assert_eq!(
            scalar(
                &fx,
                "SELECT count(*) FROM links WHERE target_raw = '/fourth'"
            ),
            i64::from(limit == 0)
        );
        assert_eq!(
            scalar(
                &fx,
                "SELECT count(*) FROM links WHERE target_raw = '/first'"
            ),
            1
        );
    }
}

// spec: CRAWL-075
#[tokio::test]
async fn partial_html_keeps_early_links_marks_truncation_and_uses_declared_size() {
    let mut cfg = CrawlConfig::default();
    cfg.limits.max_page_kb = 1;
    let fx = crawl_fixture_from("crawl-m2-switches/crawl-resources", "/large", cfg).await;
    assert!(requested_paths(&fx, None).contains(&"/early".into()));
    assert_eq!(
        scalar(&fx, "SELECT size_bytes FROM urls WHERE url LIKE '%/large'"),
        8192
    );
    assert_eq!(scalar(&fx,"SELECT v_int FROM url_ext WHERE key = 'crawl.truncated' AND url_id = (SELECT id FROM urls WHERE url LIKE '%/large')"),1);
}
