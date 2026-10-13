//! `RobotsGate` gegen den Testserver (CRAWL-098/099/100).
//!
//! Ein Test-Fetcher (`Loopback`) spricht über den Mini-HTTP-Client von `os-testsite` direkt mit dem
//! Server auf Loopback; so laufen die Tests unabhängig vom echten `HttpFetcher` (M1-P02). Wartezeiten
//! zwischen Wiederholungen laufen mit `start_paused` in simulierter Zeit (M1-Plan 2.8).
//!
//! Tests mit virtuellen Hosts (`*.site.test`) nutzen die Testsite-Erweiterung aus M1-P09 (hostgebundene
//! Routen im Wurzel-`scenario.toml` des Szenarios) und den echten `HttpFetcher`. Weil dessen echte
//! Netzwerk-E/A mit angehaltener Tokio-Uhr nicht zuverlässig ist (die Uhr springt, sobald die Laufzeit auf
//! Socket-Ereignisse wartet, und löst Scheinzeitüberschreitungen aus), laufen sie in Echtzeit mit auf 1 s
//! verkürzten Wartezeiten; die 5-s-Abstände selbst prüfen die `start_paused`-Tests mit `Loopback`.

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use os_fetch::robots::{RobotsDecision, RobotsGate, RobotsGateConfig};
use os_fetch::{
    Fetch, FetchConfig, FetchOutcome, FetchPurpose, FetchRequest, FetchResult, HttpFetcher,
    HttpResponse, MemorySecretStore,
};
use os_model::{
    CrawlConfig, ManualClock, NoResponseReason, RobotsMode, RobotsUnreachablePolicy, Timestamp,
};
use os_testsite::client::get_once;
use os_testsite::{fixtures_root, SiteOptions, TestSite};
use tokio::time::Instant;
use url::Url;

/// Spricht direkt mit dem Testserver auf Loopback (Host und Schema der URL werden ignoriert).
struct Loopback;

impl Fetch for Loopback {
    fn fetch(&self, req: FetchRequest) -> Pin<Box<dyn Future<Output = FetchResult> + Send + '_>> {
        Box::pin(async move {
            assert_eq!(req.purpose, FetchPurpose::RobotsTxt);
            let port = req.url.port_or_known_default().expect("Port");
            let addr: SocketAddr = format!("127.0.0.1:{port}").parse().expect("Adresse");
            let mut target = req.url.path().to_string();
            if let Some(q) = req.url.query() {
                target = format!("{target}?{q}");
            }
            let outcome = match get_once(addr, &target).await {
                Ok(r) => {
                    let size = r.body.len() as u64;
                    FetchOutcome::Response(HttpResponse {
                        status: r.status,
                        reason_phrase: None,
                        status_text: String::new(),
                        http_version: "HTTP/1.1".to_string(),
                        headers: r.headers.clone(),
                        body: Bytes::from(r.body),
                        size_bytes: size,
                        transferred_bytes: size,
                        response_time_ms: 0,
                        truncated: false,
                        decode_error: None,
                        md5_hex: String::new(),
                    })
                }
                Err(e) => FetchOutcome::NoResponse {
                    reason: NoResponseReason::ConnectionError,
                    detail: e.to_string(),
                    elapsed_ms: 0,
                },
            };
            FetchResult {
                url: req.url,
                started_at: Timestamp::UNIX_EPOCH,
                outcome,
            }
        })
    }
}

/// Protokolliert jeden Abruf (URL und simulierte Zeit) und reicht ihn weiter.
struct Recording<F> {
    inner: F,
    log: Mutex<Vec<(String, Instant)>>,
}

impl<F: Fetch> Recording<F> {
    fn new(inner: F) -> Self {
        Recording {
            inner,
            log: Mutex::new(Vec::new()),
        }
    }

    fn requests(&self, path: &str) -> Vec<Instant> {
        self.log
            .lock()
            .unwrap()
            .iter()
            .filter(|(u, _)| Url::parse(u).is_ok_and(|u| u.path() == path))
            .map(|(_, t)| *t)
            .collect()
    }

    fn total(&self) -> usize {
        self.log.lock().unwrap().len()
    }
}

impl<F: Fetch> Fetch for Recording<F> {
    fn fetch(&self, req: FetchRequest) -> Pin<Box<dyn Future<Output = FetchResult> + Send + '_>> {
        self.log
            .lock()
            .unwrap()
            .push((req.url.to_string(), Instant::now()));
        self.inner.fetch(req)
    }
}

fn gate(mode: RobotsMode, policy: RobotsUnreachablePolicy) -> RobotsGate {
    RobotsGate::new(RobotsGateConfig {
        token: "OpenSpider".to_string(),
        mode,
        unreachable_policy: policy,
        retries: 2,
        retry_delay_s: 5,
        clock: Arc::new(ManualClock::new(Timestamp::UNIX_EPOCH)),
    })
}

/// Wie [`respect`], aber mit kurzer Wartezeit für Echtzeit-Tests (siehe Moduldoku).
fn respect_fast_retry() -> RobotsGate {
    RobotsGate::new(RobotsGateConfig {
        token: "OpenSpider".to_string(),
        mode: RobotsMode::Respect,
        unreachable_policy: RobotsUnreachablePolicy::BlockHost,
        retries: 2,
        retry_delay_s: 1,
        clock: Arc::new(ManualClock::new(Timestamp::UNIX_EPOCH)),
    })
}

fn respect() -> RobotsGate {
    gate(RobotsMode::Respect, RobotsUnreachablePolicy::BlockHost)
}

async fn site(dir: &str) -> TestSite {
    TestSite::start_dir_with(
        fixtures_root().join(dir),
        SiteOptions {
            port: 0,
            log_requests: true,
        },
    )
    .await
    .expect("Testserver")
}

fn url(site: &TestSite, path: &str) -> Url {
    Url::parse(&site.url(path)).expect("URL")
}

fn blocked(line: &str, site: &TestSite) -> RobotsDecision {
    RobotsDecision::Blocked {
        matched_line: Some(line.to_string()),
        robots_url: site.url("/robots.txt"),
        unreachable: false,
    }
}

// spec: CRAWL-098, CRAWL-099
#[tokio::test]
async fn respect_fetches_robots_txt_once_and_blocks_with_matched_line() {
    let site = site("crawl-robots").await;
    let fetcher = Recording::new(Loopback);
    let gate = respect();

    let first = gate.check(&fetcher, &url(&site, "/privat/x")).await;
    assert_eq!(
        first.decision,
        blocked("Zeile 10: Disallow: /privat/", &site)
    );
    let (res, write) = first
        .robots_fetch
        .expect("Erstabruf liefert die robots.txt");
    assert_eq!(res.url, url(&site, "/robots.txt"));
    assert_eq!(write.origin, site.base_url());
    assert_eq!(write.robots_url, site.url("/robots.txt"));
    assert_eq!(write.fetch_status, "ok");
    assert_eq!(write.status_code, Some(200));
    assert_eq!(write.attempts, 1);
    assert!(write
        .body
        .as_deref()
        .unwrap_or("")
        .contains("Disallow: /privat/"));

    let table = [
        ("/privat/oeffentlich", RobotsDecision::Allowed),
        ("/frei/a", RobotsDecision::Allowed),
        ("/seite", RobotsDecision::Allowed),
        ("/seite2", blocked("Zeile 15: Disallow: /seite", &site)),
        (
            "/dokument.pdf",
            blocked("Zeile 12: Disallow: /*.pdf$", &site),
        ),
        ("/dokument.pdf?download=1", RobotsDecision::Allowed),
        (
            "/liste?sessionid=abc",
            blocked("Zeile 13: Disallow: /*?sessionid=", &site),
        ),
        ("/liste?seite=2&sessionid=abc", RobotsDecision::Allowed),
        (
            "/zweite-gruppe/a",
            blocked("Zeile 23: Disallow: /zweite-gruppe/", &site),
        ),
    ];
    for (path, expected) in table {
        let check = gate.check(&fetcher, &url(&site, path)).await;
        assert_eq!(check.decision, expected, "{path}");
        assert!(
            check.robots_fetch.is_none(),
            "{path}: nur der Erstabruf liefert die Datei"
        );
    }
    // Genau ein Abruf, und die gesperrten Seiten selbst werden vom Gate nie angefragt.
    assert_eq!(fetcher.total(), 1);
    let counts = site.request_counts();
    assert_eq!(counts.len(), 1);
    assert_eq!(counts.get("/robots.txt"), Some(&1));
}

// spec: CRAWL-098
#[tokio::test]
async fn other_port_or_scheme_means_own_robots_txt() {
    let a = site("crawl-robots").await;
    let b = site("crawl-robots").await;
    let fetcher = Recording::new(Loopback);
    let gate = respect();
    for s in [&a, &b] {
        let check = gate.check(&fetcher, &url(s, "/privat/x")).await;
        assert!(matches!(check.decision, RobotsDecision::Blocked { .. }));
        assert!(check.robots_fetch.is_some());
        // Zweite URL desselben Origins: kein neuer Abruf.
        assert!(gate
            .check(&fetcher, &url(s, "/frei/a"))
            .await
            .robots_fetch
            .is_none());
    }
    assert_eq!(a.request_counts().get("/robots.txt"), Some(&1));
    assert_eq!(b.request_counts().get("/robots.txt"), Some(&1));
    // Dasselbe Host-Port-Paar mit anderem Schema ist ein anderer Origin.
    let https = Url::parse(&format!("https://127.0.0.1:{}/frei/a", a.addr().port())).unwrap();
    let check = gate.check(&fetcher, &https).await;
    assert!(check.robots_fetch.is_some());
    assert_eq!(a.request_counts().get("/robots.txt"), Some(&2));
}

// spec: CRAWL-098
#[tokio::test]
async fn redirect_variant_follows_to_the_real_file() {
    let site = site("crawl-robots/hosts/redirect.site.test").await;
    let fetcher = Recording::new(Loopback);
    let check = respect().check(&fetcher, &url(&site, "/privat/x")).await;
    assert_eq!(
        check.decision,
        blocked("Zeile 2: Disallow: /privat/", &site)
    );
    let (first, write) = check.robots_fetch.expect("Erstabruf");
    // Die URL-Zeile der robots.txt zeigt die Weiterleitung, das RobotsWrite den Endstatus samt Inhalt.
    assert!(matches!(first.outcome, FetchOutcome::Response(ref r) if r.status == 301));
    assert_eq!(write.status_code, Some(200));
    assert_eq!(write.fetch_status, "ok");
    assert!(write
        .body
        .as_deref()
        .unwrap_or("")
        .contains("Disallow: /privat/"));
    let counts = site.request_counts();
    assert_eq!(counts.get("/robots.txt"), Some(&1));
    assert_eq!(counts.get("/robots-real.txt"), Some(&1));
}

// spec: CRAWL-098
#[tokio::test]
async fn bom_and_crlf_variant_is_applied() {
    let site = site("crawl-robots/hosts/bom.site.test").await;
    let check = respect().check(&Loopback, &url(&site, "/privat/x")).await;
    assert_eq!(
        check.decision,
        blocked("Zeile 2: Disallow: /privat/", &site)
    );
    let ok = respect()
        .check(&Loopback, &url(&site, "/privat/oeffentlich"))
        .await;
    assert_eq!(ok.decision, RobotsDecision::Allowed);
}

/// Schreibt ein Szenario, in dem `/robots.txt` über `hops` Weiterleitungen zu einer Datei mit
/// `Disallow: /seite` führt.
fn redirect_chain_dir(hops: usize) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("Verzeichnis");
    let mut toml = String::new();
    for i in 0..hops {
        let from = if i == 0 {
            "/robots.txt".to_string()
        } else {
            format!("/hop{i}")
        };
        toml.push_str(&format!(
            "[[route]]\npath = \"{from}\"\nstatus = 301\nlocation = \"/hop{}\"\n\n",
            i + 1
        ));
    }
    let last = if hops == 0 {
        "/robots.txt".to_string()
    } else {
        format!("/hop{hops}")
    };
    toml.push_str(&format!(
        "[[route]]\npath = \"{last}\"\nstatus = 200\ncontent_type = \"text/plain\"\n\
         body = \"User-agent: *\\nDisallow: /seite\\n\"\n"
    ));
    std::fs::write(dir.path().join("scenario.toml"), toml).expect("scenario.toml");
    dir
}

// spec: CRAWL-098, CRAWL-100
#[tokio::test(start_paused = true)]
async fn five_redirects_are_followed_six_count_as_not_found() {
    for (hops, expect_blocked) in [(1, true), (5, true), (6, false), (8, false)] {
        let dir = redirect_chain_dir(hops);
        let site = TestSite::start_dir_with(
            dir.path(),
            SiteOptions {
                port: 0,
                log_requests: true,
            },
        )
        .await
        .expect("Testserver");
        let fetcher = Recording::new(Loopback);
        let check = respect().check(&fetcher, &url(&site, "/seite")).await;
        let is_blocked = matches!(
            check.decision,
            RobotsDecision::Blocked {
                unreachable: false,
                ..
            }
        );
        assert_eq!(
            is_blocked, expect_blocked,
            "{hops} Weiterleitungen: {check:?}"
        );
        if !expect_blocked {
            assert_eq!(check.decision, RobotsDecision::Allowed);
            // Wie 404: kein erneuter Versuch.
            assert_eq!(fetcher.total(), 6, "{hops}");
            assert_eq!(
                check.robots_fetch.expect("Erstabruf").1.fetch_status,
                "not_found"
            );
        }
    }
}

// spec: CRAWL-099
#[tokio::test]
async fn mode_respect_does_not_request_blocked_page() {
    let site = site("crawl-robots").await;
    let fetcher = Recording::new(Loopback);
    let check = respect().check(&fetcher, &url(&site, "/privat/x")).await;
    assert!(matches!(check.decision, RobotsDecision::Blocked { .. }));
    // Im Request-Log steht nur die robots.txt, `/privat/x` nicht.
    assert_eq!(
        site.request_counts().keys().collect::<Vec<_>>(),
        ["/robots.txt"]
    );
}

// spec: CRAWL-099
#[tokio::test]
async fn mode_ignore_does_not_fetch_robots_txt() {
    let site = site("crawl-robots").await;
    let fetcher = Recording::new(Loopback);
    let gate = gate(RobotsMode::Ignore, RobotsUnreachablePolicy::BlockHost);
    for path in ["/privat/x", "/seite2", "/robots.txt"] {
        let check = gate.check(&fetcher, &url(&site, path)).await;
        assert_eq!(check.decision, RobotsDecision::Allowed, "{path}");
        assert!(check.robots_fetch.is_none());
    }
    assert_eq!(fetcher.total(), 0);
    assert!(
        site.request_counts().is_empty(),
        "kein Abruf von /robots.txt im Request-Log"
    );
}

// spec: CRAWL-099
#[tokio::test]
async fn mode_ignore_but_report_fetches_and_reports() {
    let site = site("crawl-robots").await;
    let fetcher = Recording::new(Loopback);
    let gate = gate(
        RobotsMode::IgnoreButReport,
        RobotsUnreachablePolicy::BlockHost,
    );
    let first = gate.check(&fetcher, &url(&site, "/privat/x")).await;
    assert_eq!(
        first.decision,
        RobotsDecision::ReportOnly {
            would_block: true,
            matched_line: Some("Zeile 10: Disallow: /privat/".to_string()),
            robots_url: site.url("/robots.txt"),
        }
    );
    assert!(
        first.robots_fetch.is_some(),
        "robots.txt wird abgerufen und gespeichert"
    );
    let free = gate.check(&fetcher, &url(&site, "/frei/a")).await;
    assert_eq!(
        free.decision,
        RobotsDecision::ReportOnly {
            would_block: false,
            matched_line: None,
            robots_url: site.url("/robots.txt"),
        }
    );
    assert_eq!(site.request_counts().get("/robots.txt"), Some(&1));
}

// spec: CRAWL-100
#[tokio::test(start_paused = true)]
async fn client_errors_empty_and_html_mean_no_rules() {
    let hosts = [
        ("r404", "not_found", Some(404)),
        ("r403", "not_found", Some(403)),
        ("rredir404", "not_found", Some(404)),
        ("rempty", "ok", Some(200)),
        ("rhtml", "ok", Some(200)),
    ];
    for (host, status, code) in hosts {
        for policy in [
            RobotsUnreachablePolicy::BlockHost,
            RobotsUnreachablePolicy::AllowAll,
        ] {
            let site = site(&format!("crawl-robots-errors/hosts/{host}.site.test")).await;
            let fetcher = Recording::new(Loopback);
            let gate = gate(RobotsMode::Respect, policy);
            let check = gate.check(&fetcher, &url(&site, "/seite")).await;
            assert_eq!(check.decision, RobotsDecision::Allowed, "{host}");
            let (_, write) = check.robots_fetch.expect("Erstabruf");
            assert_eq!(write.fetch_status, status, "{host}");
            assert_eq!(write.status_code, code, "{host}");
            assert_eq!(write.attempts, 1, "{host}: keine Wiederholung");
            assert!(
                write
                    .note
                    .as_deref()
                    .unwrap_or("")
                    .ends_with("keine Regeln"),
                "{host}: {write:?}"
            );
            assert_eq!(fetcher.requests("/robots.txt").len(), 1, "{host}");
        }
    }
}

// spec: CRAWL-100
#[tokio::test(start_paused = true)]
async fn server_errors_block_host_after_three_requests_five_seconds_apart() {
    // (Host, abgerufene Pfade je Versuch)
    let hosts = [
        ("r500", vec!["/robots.txt"]),
        ("r429", vec!["/robots.txt"]),
        ("rredir503", vec!["/robots.txt", "/unavailable"]),
    ];
    for (host, per_attempt) in hosts {
        let site = site(&format!("crawl-robots-errors/hosts/{host}.site.test")).await;
        let fetcher = Recording::new(Loopback);
        let gate = respect();
        let t0 = Instant::now();
        let check = gate.check(&fetcher, &url(&site, "/seite")).await;
        assert_eq!(
            check.decision,
            RobotsDecision::Blocked {
                matched_line: None,
                robots_url: site.url("/robots.txt"),
                unreachable: true,
            },
            "{host}"
        );
        for path in &per_attempt {
            assert_eq!(fetcher.requests(path).len(), 3, "{host}: {path}");
        }
        assert_eq!(
            fetcher.total(),
            3 * per_attempt.len(),
            "{host}: /seite wird nie angefragt"
        );
        let times = fetcher.requests("/robots.txt");
        assert!(times[1] - times[0] >= Duration::from_secs(5), "{host}");
        assert!(times[2] - times[1] >= Duration::from_secs(5), "{host}");
        assert!(Instant::now() - t0 >= Duration::from_secs(10), "{host}");
        let (_, write) = check.robots_fetch.expect("Erstabruf");
        assert_eq!(
            (write.fetch_status.as_str(), write.attempts),
            ("unreachable", 3),
            "{host}"
        );
        assert!(
            write
                .note
                .as_deref()
                .unwrap_or("")
                .ends_with("Host gesperrt"),
            "{host}"
        );
        // Der Host bleibt für alle weiteren URLs gesperrt, ohne neuen Abruf.
        let again = gate.check(&fetcher, &url(&site, "/andere")).await;
        assert!(matches!(
            again.decision,
            RobotsDecision::Blocked {
                unreachable: true,
                ..
            }
        ));
        assert_eq!(fetcher.requests("/robots.txt").len(), 3, "{host}");
    }
}

// spec: CRAWL-100
#[tokio::test(start_paused = true)]
async fn allow_all_policy_crawls_after_server_errors() {
    for host in ["r500", "r429", "rredir503"] {
        let site = site(&format!("crawl-robots-errors/hosts/{host}.site.test")).await;
        let fetcher = Recording::new(Loopback);
        let gate = gate(RobotsMode::Respect, RobotsUnreachablePolicy::AllowAll);
        let check = gate.check(&fetcher, &url(&site, "/seite")).await;
        assert_eq!(check.decision, RobotsDecision::Allowed, "{host}");
        assert_eq!(fetcher.requests("/robots.txt").len(), 3, "{host}");
        let (_, write) = check.robots_fetch.expect("Erstabruf");
        assert_eq!(
            write.note.as_deref(),
            Some("robots.txt nicht erreichbar – ohne Regeln gecrawlt"),
            "{host}"
        );
    }
}

/// Echter `HttpFetcher` mit Auflösung aller `*.site.test`-Hosts auf Loopback (virtuelle Hosts, M1-P09).
fn http_fetcher_for_virtual_hosts(response_timeout_s: u32) -> HttpFetcher {
    let mut crawl = CrawlConfig::default();
    crawl.advanced.response_timeout_s = response_timeout_s;
    let clock = Arc::new(ManualClock::new(Timestamp::UNIX_EPOCH));
    let mut cfg = FetchConfig::from_crawl_config(&crawl, &MemorySecretStore::new(), clock)
        .expect("Abruf-Konfiguration");
    cfg.resolve = vec!["*.site.test=127.0.0.1".to_string()];
    HttpFetcher::new(cfg).expect("HttpFetcher")
}

fn virtual_url(site: &TestSite, host: &str, path: &str) -> Url {
    Url::parse(&format!("http://{host}:{}{path}", site.addr().port())).expect("URL")
}

// spec: CRAWL-100
#[tokio::test]
async fn virtual_host_r503_once_recovers_on_second_attempt() {
    let site = TestSite::start("crawl-robots-errors")
        .await
        .expect("Testserver");
    let fetcher = Recording::new(http_fetcher_for_virtual_hosts(2));
    let host = "r503-once.site.test";
    // Echtzeit (siehe Moduldoku): Wartezeit zwischen den Versuchen auf 1 s verkürzt.
    let check = respect_fast_retry()
        .check(&fetcher, &virtual_url(&site, host, "/seite"))
        .await;
    assert!(
        matches!(
            check.decision,
            RobotsDecision::Blocked {
                unreachable: false,
                ..
            }
        ),
        "{check:?}"
    );
    assert_eq!(fetcher.requests("/robots.txt").len(), 2);
    let (_, write) = check.robots_fetch.expect("Erstabruf");
    assert_eq!((write.fetch_status.as_str(), write.attempts), ("ok", 2));
}

// spec: CRAWL-100
#[tokio::test]
async fn virtual_host_rtimeout_blocks_host_after_three_requests() {
    let site = TestSite::start("crawl-robots-errors")
        .await
        .expect("Testserver");
    let fetcher = Recording::new(http_fetcher_for_virtual_hosts(1));
    let check = respect_fast_retry()
        .check(
            &fetcher,
            &virtual_url(&site, "rtimeout.site.test", "/seite"),
        )
        .await;
    assert!(
        matches!(
            check.decision,
            RobotsDecision::Blocked {
                unreachable: true,
                ..
            }
        ),
        "{check:?}"
    );
    let times = fetcher.requests("/robots.txt");
    assert_eq!(times.len(), 3);
    assert!(times[1] - times[0] >= Duration::from_secs(1));
    assert!(times[2] - times[1] >= Duration::from_secs(1));
    let (first, write) = check.robots_fetch.expect("Erstabruf");
    assert!(
        matches!(
            first.outcome,
            FetchOutcome::NoResponse {
                reason: NoResponseReason::ResponseTimeout,
                ..
            }
        ),
        "{first:?}"
    );
    assert_eq!(
        (write.fetch_status.as_str(), write.attempts),
        ("unreachable", 3)
    );
}

// spec: CRAWL-100
#[tokio::test]
async fn virtual_hosts_client_errors_allow_everything() {
    let site = TestSite::start("crawl-robots-errors")
        .await
        .expect("Testserver");
    let fetcher = Recording::new(http_fetcher_for_virtual_hosts(2));
    let gate = respect();
    for host in ["r404", "r403", "rempty", "rhtml", "rredir404"] {
        let u = virtual_url(&site, &format!("{host}.site.test"), "/seite");
        let check = gate.check(&fetcher, &u).await;
        assert_eq!(check.decision, RobotsDecision::Allowed, "{host}");
    }
}

// spec: CRAWL-098
#[tokio::test]
async fn virtual_hosts_bom_redirect_and_big_variants() {
    let site = TestSite::start("crawl-robots").await.expect("Testserver");
    let fetcher = Recording::new(http_fetcher_for_virtual_hosts(2));
    let gate = respect();
    for host in ["bom.site.test", "redirect.site.test"] {
        let check = gate
            .check(&fetcher, &virtual_url(&site, host, "/privat/x"))
            .await;
        assert!(
            matches!(check.decision, RobotsDecision::Blocked { .. }),
            "{host}: {check:?}"
        );
    }
    // 3 MB: kein Absturz, höchstens 500 KiB werden gespeichert.
    let check = gate
        .check(&fetcher, &virtual_url(&site, "big.site.test", "/privat/x"))
        .await;
    let (_, write) = check.robots_fetch.expect("Erstabruf");
    assert!(write.body.map_or(0, |b| b.len()) <= 500 * 1024);
}
