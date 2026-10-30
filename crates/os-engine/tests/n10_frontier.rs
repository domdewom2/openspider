#[path = "lifecycle_support.rs"]
mod support;

use os_engine::testing::crawl_fixture_from;
use os_model::{CrawlConfig, EndReason};
use os_store::{CrawlFile, OpenMode};
use support::*;

fn snapshot_n10(file: &CrawlFile) -> Vec<(String, bool, Option<u32>, Option<u16>)> {
    file.conn().prepare("SELECT url,in_tables,crawl_depth,status_code FROM urls WHERE discovered_via <> 'robots_txt' ORDER BY url").unwrap()
        .query_map([], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap()
        .map(Result::unwrap).collect()
}
fn check(file: &CrawlFile) {
    for name in [
        "nofollow-only.html",
        "pagination-only.html",
        "alternate-only.html",
    ] {
        let row = snapshot_n10(file)
            .into_iter()
            .find(|r| r.0.ends_with(name))
            .unwrap();
        assert_eq!((row.1, row.2, row.3), (false, None, None), "{name}");
    }
    for (name, depth) in [("three.html", 3), ("four.html", 4)] {
        let row = snapshot_n10(file)
            .into_iter()
            .find(|r| r.0.ends_with(name))
            .unwrap();
        assert_eq!(
            (row.1, row.2, row.3),
            (true, Some(depth), Some(200)),
            "{name}"
        );
    }
    let resource = snapshot_n10(file)
        .into_iter()
        .find(|r| r.0.ends_with("stored.svg"))
        .unwrap();
    assert!(resource.1);
    assert_eq!(resource.3, None);
}
fn config() -> CrawlConfig {
    let mut cfg = CrawlConfig::default();
    cfg.spider.images.crawl = false;
    cfg.speed.threads = 1;
    cfg
}

// spec: CRAWL-038
// spec: CRAWL-056
// spec: CRAWL-047
#[tokio::test]
async fn hidden_relations_do_not_create_table_rows_or_shorten_followed_paths() {
    let fx = crawl_fixture_from("n10-links", "http://site.test/", config()).await;
    check(&fx.file);
    let links: u64 = fx.file.conn().query_row("SELECT COUNT(*) FROM links l JOIN urls u ON u.id=l.target_id WHERE u.url LIKE '%nofollow-only.html'", [], |r| r.get(0)).unwrap();
    assert_eq!(links, 1, "relation retained");
    let mut cfg = config();
    cfg.spider.pagination.crawl = true;
    cfg.spider.follow_internal_nofollow = true;
    let fx = crawl_fixture_from("n10-links", "http://site.test/", cfg).await;
    let row = snapshot_n10(&fx.file)
        .into_iter()
        .find(|r| r.0.ends_with("three.html"))
        .unwrap();
    assert_eq!((row.1, row.2, row.3), (true, Some(1), Some(200)));
}

// spec: CRAWL-126
// spec: CRAWL-038
// spec: CRAWL-056
#[tokio::test]
async fn checkpoint_resume_keeps_hidden_relations_and_later_promotes_followed_targets() {
    let site = start_site(&os_testsite::fixtures_root().join("n10-links")).await;
    let work = tempfile::tempdir().unwrap();
    let full_path = work.path().join("full.ospider");
    let path = work.path().join("resumed.ospider");
    let mut cfg = config();
    cfg.network.resolve = config_for(&site, 1).network.resolve;
    run_to_end(start(cfg.clone(), &full_path, deps()).await).await;
    cfg.limits.max_urls = 2;
    let paused = run_to_end(start(cfg, &path, deps()).await).await;
    assert_eq!(paused.end_reason, Some(EndReason::LimitReached));
    assert_eq!(paused.crawled, 2);
    let limited = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    assert!(scalar(&limited, "SELECT COUNT(*) FROM queue") > 0);
    let hidden = snapshot_n10(&limited)
        .into_iter()
        .find(|r| r.0.ends_with("three.html"))
        .unwrap();
    assert_eq!((hidden.1, hidden.2, hidden.3), (false, None, None));
    drop(limited);
    let cfg = resume_config(&path, &site, |c| c.limits.max_urls = 100);
    run_to_end(resume(cfg, &path, deps()).await).await;
    let full = CrawlFile::open(&full_path, OpenMode::ReadOnly).unwrap();
    let resumed = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    check(&resumed);
    assert_eq!(snapshot_n10(&full), snapshot_n10(&resumed));
}
