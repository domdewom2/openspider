use std::time::Duration;

use os_loadtest::crawler::CrawlFuture;
use os_loadtest::runner::{self, ProbeParams};
use os_loadtest::{scenario, CrawlJob, CrawlOutcome, CrawlerUnderTest, NoCrawler, Size};

const TINY: ProbeParams = ProbeParams {
    requests: 300,
    connections: 4,
};

#[test]
fn scenarios_resolve_by_name_and_spec_id() {
    assert_eq!(scenario::find("perf-001").unwrap().perf_id, "PERF-001");
    assert_eq!(scenario::find("PERF-003").unwrap().name, "perf-003");
    assert!(scenario::find("perf-004").is_none());
    assert_eq!(
        scenario::find("perf-001")
            .unwrap()
            .config(Size::Large)
            .pages,
        1_000_000
    );
    assert_eq!(
        scenario::find("perf-001")
            .unwrap()
            .config(Size::Small)
            .pages,
        10_000
    );
}

#[tokio::test]
async fn report_schema_without_crawler() {
    // spec: PERF-009
    let def = scenario::find("perf-006").unwrap();
    let rep = runner::run(def, Size::Small, TINY, &NoCrawler)
        .await
        .unwrap();
    assert!(rep.passed);

    let path = std::env::temp_dir().join(format!("os-loadtest-test-{}", std::process::id()));
    let file = path.join("loadtest").join("report.json");
    rep.write(&file).unwrap();
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    let _ = std::fs::remove_dir_all(&path);

    assert_eq!(v["scenario"], "perf-006");
    assert_eq!(v["size"], "small");
    assert_eq!(v["passed"], true);
    let started = v["started_at"].as_str().unwrap();
    assert!(started.ends_with('Z') && started.len() == 20, "{started}");
    let m = &v["metrics"];
    assert_eq!(m["server_requests"], 300);
    assert_eq!(m["server_errors"], 0);
    assert_eq!(m["server_deterministic"], true);
    assert_eq!(m["crawler_available"], false);
    assert!(m["server_requests_per_s"].as_f64().unwrap() > 0.0);
}

struct Fake(u64);

impl CrawlerUnderTest for Fake {
    fn name(&self) -> &str {
        "fake"
    }

    fn crawl(&self, job: CrawlJob) -> CrawlFuture<'_> {
        let crawled = self.0.min(job.expected_urls);
        Box::pin(async move {
            Ok(CrawlOutcome {
                urls_crawled: crawled,
                duration: Duration::from_millis(5),
                peak_rss_bytes: Some(1234),
                ..Default::default()
            })
        })
    }
}

#[tokio::test]
async fn crawler_metrics_are_merged_and_judged() {
    let def = scenario::find("perf-006").unwrap();
    let pages = def.config(Size::Small).pages;
    let ok = runner::run(def, Size::Small, TINY, &Fake(pages))
        .await
        .unwrap();
    assert!(ok.passed);
    assert_eq!(ok.metrics["crawler_available"], true);
    assert_eq!(ok.metrics["crawler_peak_rss_bytes"], 1234);
    let missing = runner::run(def, Size::Small, TINY, &Fake(pages - 1))
        .await
        .unwrap();
    assert!(!missing.passed, "fehlende URLs lassen den Lauf scheitern");
}
