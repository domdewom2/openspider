#[path = "core_support.rs"]
mod support;

use os_engine::testing::{crawl_fixture_from, FixtureCrawl};
use os_model::RobotsMode;
use support::*;

async fn crawl(internal: bool, external: bool, mode: RobotsMode) -> FixtureCrawl {
    let mut cfg = one_thread();
    cfg.robots.mode = mode;
    cfg.robots.show_blocked_internal = internal;
    cfg.robots.show_blocked_external = external;
    crawl_fixture_from("crawl-m2-robots", "http://site.test/", cfg).await
}

// spec: CRAWL-101
#[tokio::test]
async fn blocked_visibility_options_preserve_links_and_never_fetch_blocked_pages() {
    for internal in [true, false] {
        for external in [false, true] {
            let fx = crawl(internal, external, RobotsMode::Respect).await;
            for (host, visible, is_internal) in [
                ("site.test", internal, true),
                ("other.test", external, false),
            ] {
                let blocked = row_of(&fx, &url(host, "/blocked"));
                assert_eq!(blocked.is_internal, is_internal);
                assert_eq!(blocked.in_tables, visible, "{host}");
                assert_eq!(blocked.status_code, Some(0));
                assert_eq!(blocked.status.as_deref(), Some("Blocked by Robots.txt"));
                assert_eq!(blocked.state, "done");
                assert_eq!(count_requests(&fx, host, "/blocked"), 0);
                assert_eq!(count_requests(&fx, host, "/robots.txt"), 1);
                let inlinks: i64 = fx
                    .file
                    .conn()
                    .query_row(
                        "SELECT COUNT(*) FROM links WHERE target_id = ?1",
                        [blocked.id],
                        |r| r.get(0),
                    )
                    .unwrap();
                assert_eq!(inlinks, 2, "Outlinks auf {host} bleiben erhalten");
            }
            assert_eq!(scalar(&fx, "SELECT COUNT(*) FROM queue"), 0);
            for counter in ["robots_blocked_internal", "robots_blocked_external"] {
                let count: i64 = fx
                    .file
                    .conn()
                    .query_row(
                        "SELECT value FROM crawl_counters WHERE key = ?1",
                        [counter],
                        |r| r.get(0),
                    )
                    .unwrap();
                assert_eq!(count, 1);
            }
            let allowed = row_of(&fx, "http://site.test/allowed");
            assert!(allowed.in_tables);
            assert_eq!(allowed.status_code, Some(200));
        }
    }
}

// spec: CRAWL-101, CRAWL-099
#[tokio::test]
async fn visibility_options_only_apply_when_respecting_robots() {
    for mode in [RobotsMode::Ignore, RobotsMode::IgnoreButReport] {
        let fx = crawl(false, false, mode).await;
        for host in ["site.test", "other.test"] {
            let page = row_of(&fx, &url(host, "/blocked"));
            assert!(page.in_tables);
            assert_eq!(page.status_code, Some(200));
            assert_eq!(count_requests(&fx, host, "/blocked"), 1);
        }
    }
}

// spec: CRAWL-101
#[tokio::test]
async fn default_visibility_shows_internal_blocks_and_hides_external_blocks() {
    let fx = crawl_fixture_from("crawl-m2-robots", "http://site.test/", one_thread()).await;
    assert!(row_of(&fx, "http://site.test/blocked").in_tables);
    assert!(!row_of(&fx, "http://other.test/blocked").in_tables);
}

// spec: CRAWL-100, CRAWL-101
#[tokio::test]
async fn unreachable_robots_preserves_block_reason_regardless_of_visibility() {
    for visible in [false, true] {
        let mut cfg = one_thread();
        cfg.robots.show_blocked_external = visible;
        cfg.robots.retries = 0;
        let fx =
            crawl_fixture_from("crawl-m2-robots", "http://site.test/unreachable-start", cfg).await;
        let blocked = row_of(&fx, "http://unreachable.test/blocked");
        assert_eq!(blocked.in_tables, visible);
        assert_eq!(blocked.status_code, Some(0));
        assert_eq!(
            blocked.status.as_deref(),
            Some("Blocked by Robots.txt (robots.txt unreachable)")
        );
        assert_eq!(count_requests(&fx, "unreachable.test", "/robots.txt"), 1);
        assert_eq!(count_requests(&fx, "unreachable.test", "/blocked"), 0);
    }
}
