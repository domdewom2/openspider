//! Der Engine-Adapter (`EngineCrawler`) gegen kleine synthetische Sites: PERF-001, PERF-003, PERF-005, PERF-009.
//!
//! Die Größen sind klein gehalten (Regel 2.8, DoD „cargo test < 60 s“); die Varianten mit 10 000 bzw. 1 000 000
//! Seiten laufen über `os-loadtest run` (CI-Job `loadtest-small`, Großläufe per Befehl).

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use os_loadtest::engine::{EngineCrawler, BACKPRESSURE_RSS_BYTES, MAX_RSS_BYTES};
use os_loadtest::runner::{self, ProbeParams};
use os_loadtest::{CrawlerUnderTest, ScenarioDef, Size};
use os_testsite::SyntheticConfig;
use serde_json::Value;

const TINY: ProbeParams = ProbeParams {
    requests: 200,
    connections: 4,
};

fn exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_os-loadtest"))
}

fn crawler() -> EngineCrawler {
    let mut c = EngineCrawler::new(exe());
    c.threads = 4;
    c.stall_timeout = Duration::from_secs(120);
    c
}

fn plain(c: &mut SyntheticConfig) {
    c.links_per_page = 10;
}

fn slow_with_errors(c: &mut SyntheticConfig) {
    c.links_per_page = 10;
    c.status_301_share = 0.02;
    c.status_404_share = 0.02;
    c.status_5xx_share = 0.01;
    c.delay_ms = 10;
    c.slow_share = 1.0;
}

fn def(
    name: &'static str,
    perf_id: &'static str,
    pages: u64,
    tune: fn(&mut SyntheticConfig),
) -> ScenarioDef {
    ScenarioDef {
        name,
        perf_id,
        description: "Test",
        pages: (pages, pages),
        tune,
    }
}

fn metric<'a>(rep: &'a os_loadtest::Report, key: &str) -> &'a Value {
    rep.metrics
        .get(key)
        .unwrap_or_else(|| panic!("Messwert {key} fehlt: {:?}", rep.metrics.keys()))
}

// spec: PERF-001
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_engine_adapter_measures_peak_rss_and_the_scaling_ratio() {
    let d = def("perf-001", "PERF-001", 1_000, plain);
    let rep = runner::run(&d, Size::Small, TINY, &crawler())
        .await
        .unwrap();
    assert!(rep.passed, "{:?}", rep.metrics);
    assert_eq!(metric(&rep, "crawler"), "os-engine");
    assert!(metric(&rep, "crawler_urls_crawled").as_u64().unwrap() >= 1_000);
    let rss = metric(&rep, "crawler_peak_rss_bytes").as_u64().unwrap();
    assert!(
        (5_000_000..MAX_RSS_BYTES).contains(&rss),
        "ein Crawl von 1 000 Seiten braucht Megabytes, nicht {rss} Bytes"
    );
    assert_eq!(metric(&rep, "crawler_reference_pages"), 100);
    let ratio = metric(&rep, "crawler_rss_ratio").as_f64().unwrap();
    assert!(ratio > 0.5 && ratio < 2.0, "{ratio}");
    assert_eq!(
        metric(&rep, "crawler_violations").as_array().unwrap().len(),
        0
    );
    assert!(
        metric(&rep, "crawler_max_speed_cache_kib")
            .as_i64()
            .unwrap()
            < 0
    );
    // K-73: der Stillstand wird auch in perf-001 gemessen und beurteilt
    let stall = metric(&rep, "crawler_max_stall_s").as_f64().unwrap();
    assert!(
        (0.0..os_loadtest::engine::MAX_STALL_SECS).contains(&stall),
        "{stall}"
    );
    assert_eq!(metric(&rep, "crawler_rss_limits"), "bewertet");
    assert_segments_reported(&rep);
}

/// K-72: die zehn Abschnittswerte und ihre Abweichungen stehen im Bericht.
fn assert_segments_reported(rep: &os_loadtest::Report) {
    let segs = metric(rep, "crawler_segment_urls_per_s")
        .as_array()
        .unwrap();
    assert_eq!(segs.len(), 10, "{segs:?}");
    assert!(segs.iter().all(|v| v.as_f64().unwrap() > 0.0), "{segs:?}");
    let (lo, hi) = (
        metric(rep, "crawler_segment_dev_min_pct").as_f64().unwrap(),
        metric(rep, "crawler_segment_dev_max_pct").as_f64().unwrap(),
    );
    // der Mittelwert nach der Spec enthält Start und Abschluss des Prozesses: alle Abschnitte können darüber liegen
    assert!(lo > -100.0 && lo <= hi, "{lo} {hi}");
    assert!(metric(rep, "crawler_urls_per_s").as_f64().unwrap() > 0.0);
}

// spec: PERF-002
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_throughput_sections_are_reported_but_small_runs_are_not_judged() {
    // 1 000 Seiten sind weit unter 1 000 000: höchstens ein Bericht, auch bei kleinem Durchsatz keine Verletzung
    let d = def("perf-001", "PERF-002", 1_000, plain);
    let rep = runner::run(&d, Size::Small, TINY, &crawler())
        .await
        .unwrap();
    assert!(rep.passed, "{:?}", rep.metrics);
    assert_segments_reported(&rep);
    let violations = metric(&rep, "crawler_violations").as_array().unwrap();
    assert!(
        violations
            .iter()
            .all(|v| !v.as_str().unwrap().contains("PERF-002")),
        "{violations:?}"
    );
}

// spec: PERF-001
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_max_speed_profile_skips_the_rss_limits_and_keeps_the_cache_limit() {
    let d = def("perf-001", "PERF-001", 1_000, plain);
    let mut c = crawler();
    c.profile = os_store::StorageProfile::MaxSpeed;
    let rep = runner::run(&d, Size::Small, TINY, &c).await.unwrap();
    assert!(rep.passed, "{:?}", rep.metrics);
    assert_eq!(
        metric(&rep, "crawler_rss_limits"),
        "nicht bewertet (Profil max-speed, K-16)"
    );
    assert_eq!(metric(&rep, "crawler_profile"), "max-speed");
    // die Cache-Grenze (K-16) wird weiter geprüft und gemeldet
    assert!(
        metric(&rep, "crawler_max_speed_cache_kib")
            .as_i64()
            .unwrap()
            < 0
    );
    // der Spitzen-RSS wird weiter gemessen und berichtet, nur nicht bewertet
    assert!(metric(&rep, "crawler_peak_rss_bytes").as_u64().unwrap() > 0);
}

// spec: PERF-003
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_slow_store_keeps_the_crawl_going_under_the_memory_limit() {
    let d = def("perf-003", "PERF-003", 800, plain);
    let rep = runner::run(&d, Size::Small, TINY, &crawler())
        .await
        .unwrap();
    assert!(rep.passed, "{:?}", rep.metrics);
    assert_eq!(metric(&rep, "crawler_sink_delay_ms"), 100);
    assert_eq!(metric(&rep, "crawler_end_reason"), "queue_empty");
    assert!(metric(&rep, "crawler_urls_crawled").as_u64().unwrap() >= 800);
    let rss = metric(&rep, "crawler_peak_rss_bytes").as_u64().unwrap();
    assert!(rss < BACKPRESSURE_RSS_BYTES, "{rss}");
    // 800 Seiten in Batches zu höchstens 200: mindestens 4 Batches à 100 ms
    assert!(metric(&rep, "crawler_seconds").as_f64().unwrap() >= 0.3);
    // PERF-002 im Bericht, Stillstand gemessen
    assert_segments_reported(&rep);
    assert!(metric(&rep, "crawler_max_stall_s").as_f64().unwrap() < 30.0);
    // SlowSink: Wartezeit und Aufrufe als DIAG-Zähler der Gruppe `loadtest`
    let diag = metric(&rep, "crawler_diag").as_object().unwrap();
    let calls = diag["loadtest_slow_calls"].as_u64().unwrap();
    let wait_ns = diag["loadtest_slow_wait_ns"].as_u64().unwrap();
    // wie viele Batches entstehen, hängt von der Batch-Größe der Engine ab: mindestens einer
    assert!(calls >= 1, "{calls}");
    assert!(
        wait_ns >= calls * 100_000_000,
        "{calls} Aufrufe à 100 ms warten mindestens {} ns, gemessen {wait_ns}",
        calls * 100_000_000
    );
}

// spec: PERF-005
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn seeded_hard_kills_leave_a_consistent_file_that_resumes_to_the_reference_result() {
    let d = def("perf-005", "PERF-005", 900, slow_with_errors);
    let mut c = crawler();
    c.kills = Some(3);
    c.seed = 11;
    let rep = runner::run(&d, Size::Small, TINY, &c).await.unwrap();
    let violations = metric(&rep, "crawler_violations");
    assert!(rep.passed, "{violations} {:?}", rep.metrics);
    assert_eq!(metric(&rep, "crawler_kills_done"), 3);
    assert_eq!(metric(&rep, "crawler_status_identical"), true);
    assert_eq!(metric(&rep, "crawler_duplicate_urls"), 0);
    assert_eq!(metric(&rep, "crawler_urls_missing"), 0);
    for kill in metric(&rep, "crawler_kills").as_array().unwrap() {
        assert_eq!(kill["integrity_ok"], true, "{kill}");
        assert_eq!(kill["duplicate_urls"], 0, "{kill}");
    }
    let repeated = metric(&rep, "crawler_repeated_requests").as_u64().unwrap();
    let bound = metric(&rep, "crawler_repeated_requests_bound")
        .as_u64()
        .unwrap();
    // K-49: Threads + Kanalkapazität + größter Batch je Abbruch, aus den Konstanten der Writer-Stufe abgeleitet
    let per_kill = 4
        + os_engine::writer_thread::CHANNEL_CAPACITY as u64
        + os_engine::writer_thread::BATCH_PAGES_BACKLOG as u64;
    assert_eq!(bound, 3 * per_kill, "Gate nach K-49");
    assert_eq!(
        metric(&rep, "crawler_repeated_bound_per_kill").as_u64(),
        Some(per_kill)
    );
    assert!(repeated <= bound, "{repeated} > {bound}");
    // wiederholte Requests je Abbruch einzeln, in Abbruchreihenfolge, mit Mittel und Maximum
    let list: Vec<u64> = metric(&rep, "crawler_repeated_per_kill")
        .as_array()
        .expect("repeated_per_kill ist eine Liste")
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    assert_eq!(list.len(), 3, "{list:?}");
    assert!(list.iter().all(|&n| n <= per_kill), "{list:?} > {per_kill}");
    let mean = metric(&rep, "crawler_repeated_per_kill_mean")
        .as_f64()
        .unwrap();
    assert!(
        (mean - list.iter().sum::<u64>() as f64 / 3.0).abs() < 0.06,
        "{mean} {list:?}"
    );
    assert_eq!(
        metric(&rep, "crawler_repeated_per_kill_max").as_u64(),
        list.iter().copied().max()
    );
    // die Summe über die Abbrüche ist die Gesamtzahl (vor dem ersten Abbruch wird nichts wiederholt)
    assert_eq!(
        list.iter().sum::<u64>()
            + metric(&rep, "crawler_repeated_before_first_kill")
                .as_u64()
                .unwrap(),
        repeated
    );
    // die frühere Spec-Grenze (Threads + 200) und ihre Warnung entfallen
    for gone in [
        "crawler_repeated_requests_spec_bound",
        "crawler_within_spec_bound",
        "crawler_known_issue_open_after_kill",
    ] {
        assert!(!rep.metrics.contains_key(gone), "{gone}");
    }
    // die Statuswerte stehen im Bericht
    let counts = metric(&rep, "crawler_status_counts").as_object().unwrap();
    assert!(counts.contains_key("200"), "{counts:?}");
}

// spec: PERF-009
#[tokio::test]
async fn other_scenarios_have_no_engine_adapter_yet() {
    let d = def("perf-007", "PERF-007", 100, plain);
    let rep = runner::run(&d, Size::Small, TINY, &crawler())
        .await
        .unwrap();
    assert!(rep.passed);
    assert_eq!(metric(&rep, "crawler_available"), false);
    assert_eq!(crawler().name(), "os-engine");
}

// spec: PERF-009
#[test]
fn several_scenarios_write_a_suite_report_and_one_report_each() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("loadtest").join("report.json");
    let status = Command::new(exe())
        .args(["run", "--scenario", "perf-007", "--scenario", "perf-008"])
        .args(["--size", "small", "--crawler", "none"])
        .arg("--out")
        .arg(&out)
        .stdout(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success());
    let suite: Value = serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    assert_eq!(suite["size"], "small");
    assert_eq!(suite["passed"], true);
    let names: Vec<&str> = suite["reports"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["scenario"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["perf-007", "perf-008"]);
    for name in names {
        let single: Value = serde_json::from_str(
            &std::fs::read_to_string(out.with_file_name(format!("report-{name}-small.json")))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(single["scenario"], name);
        assert_eq!(single["passed"], true);
    }
}

// spec: PERF-001
#[test]
fn a_child_process_reports_its_peak_rss_to_the_parent() {
    let mut child = Command::new(exe())
        .arg("list")
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let exit = os_loadtest::rss::wait_with_peak_rss(&mut child).unwrap();
    assert!(exit.success, "{exit:?}");
    let rss = exit.peak_rss_bytes.expect("Spitzen-RSS des Kindes");
    assert!((1_000_000..2_000_000_000).contains(&rss), "{rss}");
}
