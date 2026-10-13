use std::collections::{HashSet, VecDeque};
use std::time::Instant;

use os_testsite::client::{get_once, HttpClient};
use os_testsite::{SiteOptions, SyntheticConfig, SyntheticSite, TestSite};

fn anchors(html: &str) -> Vec<String> {
    html.split("<a href=\"")
        .skip(1)
        .filter_map(|p| p.split('"').next())
        .map(str::to_string)
        .collect()
}

fn logging() -> SiteOptions {
    SiteOptions {
        log_requests: true,
        ..Default::default()
    }
}

#[tokio::test]
async fn crawl_1000_pages_over_http_without_duplicates() {
    // spec: PERF-001, PERF-003 (Testsite-Grundlage, Request-Log erkennt Duplikat-Abrufe)
    let site = TestSite::start_synthetic(
        SyntheticConfig {
            pages: 1000,
            status_301_share: 0.03,
            status_404_share: 0.03,
            status_5xx_share: 0.02,
            ..Default::default()
        },
        logging(),
    )
    .await
    .unwrap();
    let mut client = HttpClient::connect(site.addr()).await.unwrap();

    let mut seen: HashSet<String> = HashSet::from(["/".to_string()]);
    let mut queue = VecDeque::from(["/".to_string()]);
    let (mut ok, mut redirects, mut not_found, mut errors) = (0, 0, 0, 0);
    while let Some(path) = queue.pop_front() {
        let resp = client.get(&path).await.unwrap();
        let next = match resp.status {
            200 => {
                ok += 1;
                anchors(&resp.text())
            }
            301 => {
                redirects += 1;
                vec![resp.header("location").unwrap().to_string()]
            }
            404 => {
                not_found += 1;
                vec![]
            }
            500 | 503 => {
                errors += 1;
                vec![]
            }
            other => panic!("Status {other} für {path}"),
        };
        for link in next {
            if seen.insert(link.clone()) {
                queue.push_back(link);
            }
        }
    }
    assert_eq!(seen.len(), 1000, "alle Seiten von / erreichbar");
    assert_eq!(ok + redirects + not_found + errors, 1000);
    assert!(redirects > 0 && not_found > 0 && errors > 0);

    // Request-Log: jede Seite genau einmal, kein Duplikat-Abruf.
    assert_eq!(site.total_requests(), 1000);
    assert!(site.duplicate_requests().is_empty());

    // Ein zweiter Abruf wird als Duplikat erkannt.
    client.get("/").await.unwrap();
    client.get("/").await.unwrap();
    assert_eq!(site.duplicate_requests(), vec![("/".to_string(), 3)]);

    // JSON-Endpunkt liefert dieselben Zähler und zählt sich nicht selbst.
    let json = client.get("/__testsite/requests").await.unwrap();
    assert_eq!(json.status, 200);
    let v: serde_json::Value = serde_json::from_slice(&json.body).unwrap();
    assert_eq!(v["total"], 1002);
    assert_eq!(v["paths"]["/"], 3);
    site.reset_request_log();
    assert_eq!(site.total_requests(), 0);
}

#[tokio::test]
async fn responses_are_byte_identical_across_servers() {
    let cfg = SyntheticConfig {
        pages: 500,
        noindex_share: 0.2,
        duplicate_title_share: 0.2,
        ..Default::default()
    };
    let a = TestSite::start_synthetic(cfg.clone(), SiteOptions::default())
        .await
        .unwrap();
    let b = TestSite::start_synthetic(cfg, SiteOptions::default())
        .await
        .unwrap();
    for id in (0..500).step_by(7) {
        let path = SyntheticSite::page_path(id);
        let ra = get_once(a.addr(), &path).await.unwrap();
        let rb = get_once(b.addr(), &path).await.unwrap();
        let rc = get_once(a.addr(), &path).await.unwrap();
        assert_eq!(ra.body, rb.body, "{path}");
        assert_eq!(ra.body, rc.body, "{path}");
        assert_eq!(ra.status, rb.status);
    }
}

#[tokio::test]
async fn robots_and_sitemap_index_over_http() {
    let site = TestSite::start_synthetic(
        SyntheticConfig {
            pages: 100_001,
            ..Default::default()
        },
        SiteOptions::default(),
    )
    .await
    .unwrap();
    let robots = get_once(site.addr(), "/robots.txt").await.unwrap();
    assert_eq!(robots.status, 200);
    assert!(robots
        .text()
        .contains(&format!("Sitemap: {}/sitemap.xml", site.base_url())));
    let index = get_once(site.addr(), "/sitemap.xml").await.unwrap();
    assert_eq!(index.header("content-type"), Some("application/xml"));
    assert_eq!(index.text().matches("<sitemap>").count(), 3);
    let last = get_once(site.addr(), "/sitemap-3.xml").await.unwrap();
    assert_eq!(last.text().matches("<loc>").count(), 1);
    let missing = get_once(site.addr(), "/sitemap-4.xml").await.unwrap();
    assert_eq!(missing.status, 404);
}

#[tokio::test]
async fn scenario_toml_with_synthetic_section() {
    // spec: PERF-001 (Fixture-Szenario ohne Dateien auf Platte, nur scenario.toml)
    let site = TestSite::start("synthetic-small").await.unwrap();
    let synth = site.synthetic().expect("synthetic aktiv");
    assert_eq!(synth.pages(), 1000);
    assert_eq!(synth.level_sizes(), [1, 10, 100, 889]);
    let home = get_once(site.addr(), "/").await.unwrap();
    assert_eq!(home.status, 200);
    assert!(home.text().contains("<h1>Seite 0</h1>"));
}

#[tokio::test]
async fn short_form_spec_starts_site() {
    let site = TestSite::start_spec("synthetic:n=2000,links=12,seed=7", SiteOptions::default())
        .await
        .unwrap();
    assert_eq!(site.synthetic().unwrap().pages(), 2000);
    assert_eq!(site.synthetic().unwrap().config().seed, 7);
    let resp = get_once(site.addr(), "/s5/p1005.html").await.unwrap();
    assert_eq!(resp.status, 200);
    assert!(anchors(&resp.text()).len() >= 12);
    assert!(
        TestSite::start_spec("synthetic:n=oops", SiteOptions::default())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn slow_pages_respect_delay() {
    let site = TestSite::start_synthetic(
        SyntheticConfig {
            pages: 50,
            delay_ms: 150,
            slow_share: 1.0,
            ..Default::default()
        },
        SiteOptions::default(),
    )
    .await
    .unwrap();
    let t = Instant::now();
    get_once(site.addr(), "/").await.unwrap();
    assert!(t.elapsed().as_millis() >= 150);
    // robots.txt ist nie verzögert.
    let t = Instant::now();
    get_once(site.addr(), "/robots.txt").await.unwrap();
    assert!(t.elapsed().as_millis() < 100);
}

/// Benchmark: > 5.000 Requests/s lokal. Nur im Release-Build aussagekräftig:
/// `cargo test -p os-testsite --release --test synthetic -- --ignored --nocapture`
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Benchmark, mit --release und --ignored ausführen"]
async fn benchmark_20k_requests_over_5000_per_second() {
    // spec: PERF-002 (Testserver darf den Durchsatzmesswert nicht begrenzen)
    const TOTAL: usize = 20_000;
    const CONNECTIONS: usize = 32;
    let site = TestSite::start_synthetic(
        SyntheticConfig {
            pages: 1_000_000,
            ..Default::default()
        },
        SiteOptions::default(),
    )
    .await
    .unwrap();
    let addr = site.addr();
    let start = Instant::now();
    let mut tasks = Vec::new();
    for c in 0..CONNECTIONS {
        tasks.push(tokio::spawn(async move {
            let mut client = HttpClient::connect(addr).await.unwrap();
            let mut bytes = 0usize;
            for i in (c..TOTAL).step_by(CONNECTIONS) {
                // Verteilt über die gesamte Site (ca. 50.000er Schritte).
                let id = (i as u64 * 49_999 + 1) % 1_000_000;
                let resp = client.get(&SyntheticSite::page_path(id)).await.unwrap();
                assert_eq!(resp.status, 200);
                bytes += resp.body.len();
            }
            bytes
        }));
    }
    let mut bytes = 0;
    for t in tasks {
        bytes += t.await.unwrap();
    }
    let secs = start.elapsed().as_secs_f64();
    let rps = TOTAL as f64 / secs;
    println!(
        "benchmark: {TOTAL} Requests in {secs:.2} s = {rps:.0} Requests/s, {:.1} MB",
        bytes as f64 / 1e6
    );
    assert!(rps > 5000.0, "nur {rps:.0} Requests/s");
}
