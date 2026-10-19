//! Echte RSS-Messung eines Renderer-Testprozesses; kein Browser/externes Netz erforderlich.
use os_engine::{
    memory,
    testing::{test_deps, SimFetcher, SimSite},
    CrawlRequest, CrawlSource,
};
use os_model::{CrawlConfig, CrawlState};
use std::io::{BufRead, Read, Write};
use std::process::{Child, Command, Stdio};

const ENV: &str = "OPENSPIDER_E02_MEMORY_CHILD";
struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
        memory::set_limit_for_test(None);
    }
}

// Fixture-Prozess für die Messung; normaler Testaufruf erzeugt keinen Speicher.
#[test]
fn renderer_fixture_process() {
    if std::env::var_os(ENV).is_none() {
        return;
    }
    let bytes: usize = include_str!("../../../tests/fixtures/crawl-m2-lifecycle/memory-bytes.txt")
        .trim()
        .parse()
        .unwrap();
    let mut allocation = vec![0_u8; bytes];
    for i in (0..bytes).step_by(4096) {
        allocation[i] = 1;
    }
    std::hint::black_box(&allocation);
    println!("E02_MEMORY_READY");
    std::io::stdout().flush().unwrap();
    let _ = std::io::stdin().read(&mut [0]);
    std::hint::black_box(&allocation);
}

// spec: CRAWL-129
#[tokio::test]
async fn renderer_child_rss_triggers_pause_when_engine_alone_is_below_limit() {
    let baseline = memory::sample();
    let limit = baseline.engine_bytes + (150 << 20);
    memory::set_limit_for_test(Some(limit));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("process.ospider");
    let fetcher = SimFetcher::new(
        SimSite::tree("site.test", 100).delay(std::time::Duration::from_millis(100)),
    );
    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = 1;
    let mut deps = test_deps();
    deps.clock = std::sync::Arc::new(os_model::SystemClock);
    let handle = os_engine::start_crawl(
        fetcher.clone(),
        CrawlRequest {
            config: cfg,
            source: CrawlSource::Spider {
                start_url: "https://site.test/".into(),
            },
            file: path,
            project: "fixture".into(),
            name: "process".into(),
        },
        deps,
    )
    .await
    .unwrap();
    let child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "renderer_fixture_process", "--nocapture"])
        .env(ENV, "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut child = ChildGuard(child);
    let stdout = child.0.stdout.take().unwrap();
    let mut lines = std::io::BufReader::new(stdout).lines();
    assert!(lines.any(|line| line.unwrap().contains("E02_MEMORY_READY")));
    let summary = tokio::time::timeout(std::time::Duration::from_secs(5), handle.wait())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(summary.state, CrawlState::Paused);
    let sample = memory::sample();
    assert!(sample.renderer_bytes >= 190 << 20, "{sample:?}");
    assert!(sample.engine_bytes < limit * 9 / 10, "{sample:?}");
    assert_eq!(sample.stage(), os_model::MemoryStage::Paused);
    let count = fetcher.requests().len();
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    assert_eq!(fetcher.requests().len(), count);
}
