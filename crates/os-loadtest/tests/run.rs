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

struct Violating;

impl CrawlerUnderTest for Violating {
    fn name(&self) -> &str {
        "violating"
    }

    fn crawl(&self, job: CrawlJob) -> CrawlFuture<'_> {
        let pages = job.expected_urls;
        Box::pin(async move {
            Ok(CrawlOutcome {
                urls_crawled: pages,
                duration: Duration::from_millis(5),
                violations: vec!["PERF-001: Beispiel".to_string()],
                ..Default::default()
            })
        })
    }
}

// spec: PERF-009
#[tokio::test]
async fn a_violated_criterion_fails_the_run_and_is_listed_in_the_report() {
    let def = scenario::find("perf-006").unwrap();
    let rep = runner::run(def, Size::Small, TINY, &Violating)
        .await
        .unwrap();
    assert!(!rep.passed);
    assert_eq!(
        rep.metrics["crawler_violations"],
        serde_json::json!(["PERF-001: Beispiel"])
    );
}

/// Crawler mit festem Endbatch (`diag`) und gleichmäßigen Abschnitten, wie ihn `EngineCrawler` liefert.
struct EndBatch {
    end_batch_ns: Option<u64>,
    /// Crawl-Dauer in Sekunden.
    seconds: f64,
}

impl CrawlerUnderTest for EndBatch {
    fn name(&self) -> &str {
        "end-batch"
    }

    fn crawl(&self, job: CrawlJob) -> CrawlFuture<'_> {
        let pages = job.expected_urls;
        let (end_batch_ns, seconds) = (self.end_batch_ns, self.seconds);
        Box::pin(async move {
            let mut extra = std::collections::BTreeMap::new();
            if let Some(ns) = end_batch_ns {
                extra.insert(
                    "diag".to_string(),
                    serde_json::json!({ "engine_w_end_batch_ns": ns }),
                );
            }
            // zehn gleiche Abschnitte mit 4 000 URLs/s: 250 s bis zur letzten URL bei 1 Mio. URLs
            extra.insert(
                "segment_urls_per_s".to_string(),
                serde_json::json!(vec![4_000.0; 10]),
            );
            Ok(CrawlOutcome {
                urls_crawled: pages,
                duration: Duration::from_secs_f64(seconds),
                extra,
                ..Default::default()
            })
        })
    }
}

// spec: PERF-002
#[tokio::test]
async fn k80_the_end_batch_over_60_s_fails_perf_001_with_one_million_pages() {
    let def = scenario::find("perf-001").unwrap();
    // 1 Mio. URLs bei 4 000 URLs/s = 250 s bis zur letzten URL; Dauer 330 s, davon 80 s Endbatch
    let rep = runner::run(
        def,
        Size::Large,
        TINY,
        &EndBatch {
            end_batch_ns: Some(80_000_000_000),
            seconds: 330.0,
        },
    )
    .await
    .unwrap();
    assert!(!rep.passed);
    assert_eq!(
        rep.metrics["crawler_violations"],
        serde_json::json!(["K-80: Endbatch 80.0 s > 60 s"])
    );
    assert_eq!(rep.metrics["crawler_end_batch_s"], 80.0);
    assert_eq!(rep.metrics["crawler_tail_s"], 80.0);
}

// spec: PERF-002
#[tokio::test]
async fn k80_an_end_batch_within_the_limit_passes_and_both_values_are_reported() {
    let def = scenario::find("perf-001").unwrap();
    let rep = runner::run(
        def,
        Size::Large,
        TINY,
        &EndBatch {
            end_batch_ns: Some(44_500_000_000),
            seconds: 294.5,
        },
    )
    .await
    .unwrap();
    assert!(rep.passed, "{:?}", rep.metrics["crawler_violations"]);
    assert_eq!(rep.metrics["crawler_violations"], serde_json::json!([]));
    assert_eq!(rep.metrics["crawler_end_batch_s"], 44.5);
    assert_eq!(rep.metrics["crawler_tail_s"], 44.5);
    // ohne Zähler bei einem beurteilten Lauf: nicht messbar ist eine Verletzung
    let rep = runner::run(
        def,
        Size::Large,
        TINY,
        &EndBatch {
            end_batch_ns: None,
            seconds: 294.5,
        },
    )
    .await
    .unwrap();
    assert!(!rep.passed);
    assert_eq!(rep.metrics["crawler_end_batch_s"], serde_json::Value::Null);
    assert_eq!(
        rep.metrics["crawler_violations"],
        serde_json::json!(["K-80: Endbatch nicht messbar"])
    );
}

// spec: PERF-002
#[tokio::test]
async fn k80_applies_neither_to_small_runs_nor_to_other_scenarios_but_both_values_are_always_reported(
) {
    // kleiner Lauf: berichtet, nicht beurteilt
    let def = scenario::find("perf-001").unwrap();
    let rep = runner::run(
        def,
        Size::Small,
        TINY,
        &EndBatch {
            end_batch_ns: Some(500_000_000_000),
            seconds: 600.0,
        },
    )
    .await
    .unwrap();
    assert!(rep.passed);
    assert_eq!(rep.metrics["crawler_end_batch_s"], 500.0);
    // anderes Szenario mit 1 Mio. Seiten: ebenfalls nicht beurteilt
    let def = scenario::find("perf-003").unwrap();
    let rep = runner::run(
        def,
        Size::Large,
        TINY,
        &EndBatch {
            end_batch_ns: Some(500_000_000_000),
            seconds: 800.0,
        },
    )
    .await
    .unwrap();
    assert!(rep.passed);
    assert_eq!(rep.metrics["crawler_end_batch_s"], 500.0);
    // Crawler ohne Messwerte: beide Felder stehen als null im Bericht
    let def = scenario::find("perf-006").unwrap();
    let pages = def.config(Size::Small).pages;
    let rep = runner::run(def, Size::Small, TINY, &Fake(pages))
        .await
        .unwrap();
    assert_eq!(rep.metrics["crawler_end_batch_s"], serde_json::Value::Null);
    assert_eq!(rep.metrics["crawler_tail_s"], serde_json::Value::Null);
}
