use os_engine::host_pace::HostPacer;
use os_fetch::{FetchOutcome, FetchResult, HttpResponse};
use os_model::{Clock, CrawlConfig, Timestamp};
use std::time::Duration;
struct TestClock(tokio::time::Instant);
impl Clock for TestClock {
    fn now(&self) -> Timestamp {
        Timestamp::UNIX_EPOCH.plus(self.monotonic())
    }
    fn monotonic(&self) -> Duration {
        self.0.elapsed()
    }
}
fn result(status: u16, ms: u32, after: Option<&str>) -> FetchResult {
    FetchResult {
        url: "http://site.test/".parse().unwrap(),
        started_at: Timestamp::UNIX_EPOCH,
        outcome: FetchOutcome::Response(HttpResponse {
            status,
            request_headers: vec![],
            set_cookies: vec![],
            content_length: None,
            untruncated_bytes: None,
            synthetic: None,
            attempts: 1,
            reason_phrase: None,
            status_text: String::new(),
            http_version: "HTTP/1.1".into(),
            headers: after
                .map(|v| vec![("retry-after".into(), v.into())])
                .unwrap_or_default(),
            body: Default::default(),
            size_bytes: 0,
            transferred_bytes: 0,
            response_time_ms: ms,
            truncated: false,
            decode_error: None,
            md5_hex: String::new(),
        }),
    }
}
// spec: CRAWL-145
#[tokio::test(start_paused = true)]
async fn crawl_delay_is_per_host_capped_and_can_be_disabled() {
    let clock = TestClock(tokio::time::Instant::now());
    let mut cfg = CrawlConfig::default();
    let p = HostPacer::new(Some(3));
    let u = "http://site.test/".parse().unwrap();
    p.before_fetch(&u, Some(0.5), &cfg, &clock).await;
    p.before_fetch(&u, Some(0.5), &cfg, &clock).await;
    assert_eq!(clock.monotonic(), Duration::from_millis(500));
    p.before_fetch(
        &"http://other.test/".parse().unwrap(),
        Some(5.0),
        &cfg,
        &clock,
    )
    .await;
    assert_eq!(clock.monotonic(), Duration::from_millis(500));
    p.before_fetch(&u, Some(999.0), &cfg, &clock).await;
    assert_eq!(clock.monotonic(), Duration::from_millis(60500));
    cfg.robots.respect_crawl_delay = false;
    p.before_fetch(&u, Some(20.0), &cfg, &clock).await;
    assert_eq!(clock.monotonic(), Duration::from_millis(60500));
}
// spec: CRAWL-146
// spec: CRAWL-097
#[tokio::test(start_paused = true)]
async fn overload_pauses_then_recovers_after_twenty_successes() {
    let clock = TestClock(tokio::time::Instant::now());
    let cfg = CrawlConfig::default();
    let p = HostPacer::new(Some(3));
    let u = "http://site.test/".parse().unwrap();
    p.before_fetch(&u, None, &cfg, &clock).await;
    p.on_result(&u, &result(503, 1, Some("999")), &cfg, &clock);
    assert!(p.status(&u, &clock).1);
    assert_eq!(p.notices().lock().unwrap().len(), 1);
    p.before_fetch(&u, None, &cfg, &clock).await;
    assert_eq!(clock.monotonic(), Duration::from_secs(300));
    p.on_result(&u, &result(503, 1, None), &cfg, &clock);
    p.on_result(&u, &result(503, 1, None), &cfg, &clock);
    p.before_fetch(&u, None, &cfg, &clock).await;
    assert_eq!(clock.monotonic(), Duration::from_secs(302));
    for _ in 0..40 {
        p.on_result(&u, &result(200, 1, None), &cfg, &clock);
    }
    assert!(!p.status(&u, &clock).1);
}
// spec: CRAWL-147
#[tokio::test(start_paused = true)]
async fn aimd_rises_halves_and_is_disabled_by_explicit_rate() {
    let clock = TestClock(tokio::time::Instant::now());
    let mut cfg = CrawlConfig::default();
    cfg.speed.auto = true;
    let p = HostPacer::new(Some(4));
    let u = "http://site.test/".parse().unwrap();
    p.before_fetch(&u, None, &cfg, &clock).await;
    assert_eq!(p.status(&u, &clock).0, 1);
    for _ in 0..30 {
        p.on_result(&u, &result(200, 100, None), &cfg, &clock);
    }
    assert_eq!(p.status(&u, &clock).0, 4);
    p.on_result(&u, &result(200, 400, None), &cfg, &clock);
    assert_eq!(p.status(&u, &clock).0, 2);
    p.on_result(&u, &result(429, 100, None), &cfg, &clock);
    assert_eq!(p.status(&u, &clock).0, 1);
    cfg.speed.max_urls_per_sec = Some(1.0);
    p.before_fetch(&u, None, &cfg, &clock).await;
    assert_eq!(p.status(&u, &clock).0, 4);
}

// spec: CRAWL-146
// spec: CRAWL-145
#[tokio::test(start_paused = true)]
async fn pause_cancels_host_wait_without_advancing_the_clock() {
    let clock = TestClock(tokio::time::Instant::now());
    let cfg = CrawlConfig::default();
    let p = HostPacer::new(Some(1));
    let u = "http://site.test/".parse().unwrap();
    let first = p.acquire(&u).await;
    p.before_fetch(&u, Some(60.0), &cfg, &clock).await;
    let waiting = p.before_fetch(&u, Some(60.0), &cfg, &clock);
    tokio::pin!(waiting);
    assert!(futures_util::poll!(waiting.as_mut()).is_pending());
    p.cancellation().cancel();
    waiting.await;
    let _cancelled = p.acquire(&u).await;
    drop(first);
    assert_eq!(clock.monotonic(), Duration::ZERO);
}

// spec: CRAWL-097
#[tokio::test(start_paused = true)]
async fn rate_limit_retry_after_applies_without_adaptive_spacing() {
    let clock = TestClock(tokio::time::Instant::now());
    let mut cfg = CrawlConfig::default();
    cfg.speed.adaptive_throttle = false;
    let p = HostPacer::new(Some(2));
    let u = "http://site.test/".parse().unwrap();
    p.before_fetch(&u, None, &cfg, &clock).await;
    p.on_result(&u, &result(429, 1, Some("15")), &cfg, &clock);
    p.before_fetch(&u, None, &cfg, &clock).await;
    assert_eq!(clock.monotonic(), Duration::from_secs(15));
    assert!(p.status(&u, &clock).1);
}
