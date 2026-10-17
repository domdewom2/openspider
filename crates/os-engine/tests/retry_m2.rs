use os_engine::retry::{decide, retry_after, RetryDecision};
use os_fetch::{Fetch, FetchConfig, FetchOutcome, FetchRequest, HttpFetcher, MemorySecretStore};
use os_model::{CrawlConfig, NoResponseReason, SystemClock, Timestamp};
use os_testsite::TestSite;
use std::{sync::Arc, time::Duration};

// spec: CRAWL-096
#[tokio::test]
async fn fixture_retries_keep_only_the_final_result_and_count_attempts() {
    for (path, retries, status, attempts) in [
        ("/flaky", 2, 200, 3),
        ("/always500", 2, 500, 3),
        ("/flaky", 0, 503, 1),
    ] {
        let site = TestSite::start("crawl-m2-http").await.unwrap();
        let mut cfg = CrawlConfig::default();
        cfg.http.retries_5xx = retries;
        let f = HttpFetcher::new(
            FetchConfig::from_crawl_config(&cfg, &MemorySecretStore::new(), Arc::new(SystemClock))
                .unwrap(),
        )
        .unwrap();
        let mut attempt = 0;
        loop {
            let res = f
                .fetch(FetchRequest {
                    url: site.url(path).parse().unwrap(),
                    purpose: os_fetch::FetchPurpose::Page,
                    cookies: None,
                    max_body_bytes: 1024,
                    body_limit: Default::default(),
                    attempt,
                })
                .await;
            match decide(&res, attempt, &cfg) {
                RetryDecision::RetryAfter(d) => {
                    assert_eq!(d, Duration::from_secs(1 << attempt));
                    attempt += 1;
                }
                RetryDecision::Done => {
                    let FetchOutcome::Response(r) = res.outcome else {
                        panic!("response")
                    };
                    assert_eq!((r.status, r.attempts), (status, attempts));
                    break;
                }
            }
        }
    }
}

// spec: CRAWL-096
// spec: CRAWL-097
#[test]
fn retry_after_dates_caps_and_network_failure_policy() {
    let now = Timestamp::from_db("2026-10-17T12:00:00Z").unwrap();
    assert_eq!(
        retry_after("Sat, 17 Oct 2026 12:00:30 GMT", now, 60),
        Some(Duration::from_secs(30))
    );
    assert_eq!(
        retry_after("999999999999999999999999999", now, 60),
        Some(Duration::from_secs(60))
    );
    assert_eq!(retry_after("-1", now, 60), None);
    assert_eq!(
        retry_after("Sat, 17 Oct 2026 11:00:30 GMT", now, 60),
        Some(Duration::ZERO)
    );
    let cfg = CrawlConfig::default();
    for reason in [
        NoResponseReason::ConnectionError,
        NoResponseReason::ResponseTimeout,
        NoResponseReason::ConnectionTimeout,
        NoResponseReason::DnsLookupFailed,
        NoResponseReason::TlsError,
    ] {
        let r = os_fetch::FetchResult {
            url: "http://site.test/".parse().unwrap(),
            started_at: now,
            outcome: FetchOutcome::NoResponse {
                reason,
                detail: String::new(),
                elapsed_ms: 0,
            },
        };
        let retry = matches!(
            reason,
            NoResponseReason::ConnectionError
                | NoResponseReason::ResponseTimeout
                | NoResponseReason::ConnectionTimeout
        );
        assert_eq!(
            decide(&r, 0, &cfg),
            if retry {
                RetryDecision::RetryAfter(Duration::from_secs(1))
            } else {
                RetryDecision::Done
            }
        );
        assert_eq!(decide(&r, 1, &cfg), RetryDecision::Done);
    }
}

// spec: CRAWL-096
#[tokio::test]
async fn frontier_backoff_releases_worker_and_persists_final_attempts() {
    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = 1;
    cfg.speed.adaptive_throttle = false;
    cfg.http.retries_5xx = 2;
    let mut deps = os_engine::testing::test_deps();
    deps.clock = Arc::new(SystemClock);
    let fx = os_engine::testing::crawl_fixture_with("crawl-m2-http", "/frontier", cfg, deps).await;
    let requests = fx.site.requests();
    let paths: Vec<_> = requests.iter().map(|r| r.path_and_query.as_str()).collect();
    let retries: Vec<_> = paths
        .iter()
        .enumerate()
        .filter_map(|(i, p)| (*p == "/flaky").then_some(i))
        .collect();
    assert_eq!(retries.len(), 3);
    assert!(paths.iter().position(|p| *p == "/steady").unwrap() < retries[1]);
    let (status,attempts):(i64,i64)=fx.file.conn().query_row("SELECT u.status_code,e.v_int FROM urls u JOIN url_ext e ON e.url_id=u.id WHERE u.url LIKE '%/flaky' AND e.key='http.attempts'",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
    assert_eq!((status, attempts), (200, 3));
    assert_eq!(fx.summary.crawled, 3);
}

// spec: CRAWL-096
#[test]
fn exponential_backoff_is_not_capped_by_the_retry_after_header_limit() {
    let mut cfg = CrawlConfig::default();
    cfg.http.retries_network = 10;
    let r = os_fetch::FetchResult {
        url: "http://site.test/".parse().unwrap(),
        started_at: Timestamp::UNIX_EPOCH,
        outcome: FetchOutcome::NoResponse {
            reason: NoResponseReason::ConnectionError,
            detail: String::new(),
            elapsed_ms: 0,
        },
    };
    assert_eq!(
        decide(&r, 7, &cfg),
        RetryDecision::RetryAfter(Duration::from_secs(128))
    );
}
