#[path = "sidebar_support.rs"]
mod support;
use os_export::{export_bulk, Format};
use os_model::CrawlConfig;
// spec: OUT-030
#[test]
fn queue_excludes_completed_external_and_never_admitted_urls() {
    let dir = tempfile::tempdir().unwrap();
    let file = support::file(dir.path(), "paths.sql");
    let cfg = CrawlConfig::default();
    let info = export_bulk(
        &file,
        &cfg,
        "Queued URLs",
        &dir.path().join("out"),
        Format::Csv,
    )
    .unwrap();
    assert_eq!(info[0].rows, 1);
    support::golden(&info[0].path, "queued.csv");
    file.conn().execute_batch("DELETE FROM queue").unwrap();
    let info = export_bulk(
        &file,
        &cfg,
        "Queued URLs",
        &dir.path().join("complete"),
        Format::Csv,
    )
    .unwrap();
    assert_eq!(info[0].rows, 0);
    assert_eq!(support::csv(&info[0].path).len(), 1);
}

// spec: OUT-030
#[tokio::test]
async fn limit_five_retains_the_sixteen_unfetched_discoveries() {
    let mut cfg = CrawlConfig::default();
    cfg.limits.max_urls = 5;
    cfg.speed.threads = 1;
    let fixture = os_engine::testing::crawl_dir_with(
        &support::root().join("tests/fixtures/out-m2-reports/queue-site"),
        "/",
        cfg.clone(),
        os_engine::testing::test_deps(),
    )
    .await;
    let out = tempfile::tempdir().unwrap();
    let info = export_bulk(&fixture.file, &cfg, "Queued URLs", out.path(), Format::Csv).unwrap();
    assert_eq!(info[0].rows, 16);
    for row in support::csv(&info[0].path).iter().skip(1) {
        assert_eq!(row[1], "1");
        assert!(row[2].ends_with('/'));
        assert_eq!(row[3], "Hyperlink");
    }
}
