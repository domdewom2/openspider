//! Ausführung eines Szenarios: misst den Testserver und (sobald vorhanden) den Crawler.

use std::collections::BTreeMap;
use std::io;
use std::time::Instant;

use os_testsite::client::HttpClient;
use os_testsite::{SiteOptions, SyntheticSite, TestSite};
use serde_json::{json, Value};

use crate::crawler::{CrawlJob, CrawlerError, CrawlerUnderTest};
use crate::report::{now_rfc3339, Report};
use crate::scenario::{ScenarioDef, Size};

/// Mindestdurchsatz des Testservers (Requests/s), nur im Release-Build bewertet.
pub const MIN_SERVER_RPS: f64 = 5000.0;

/// Parameter der Server-Messung.
#[derive(Debug, Clone, Copy)]
pub struct ProbeParams {
    /// Anzahl Requests.
    pub requests: usize,
    /// Parallele Keep-Alive-Verbindungen.
    pub connections: usize,
}

impl ProbeParams {
    /// Standardwerte je Größe.
    pub fn for_size(size: Size) -> Self {
        Self {
            requests: match size {
                Size::Small => 10_000,
                Size::Large => 100_000,
            },
            connections: 32,
        }
    }
}

/// Ergebnis der Server-Messung.
#[derive(Debug, Clone)]
pub struct ServerMetrics {
    /// Anzahl gesendeter Requests.
    pub requests: usize,
    /// Dauer in Sekunden.
    pub seconds: f64,
    /// Requests pro Sekunde.
    pub requests_per_s: f64,
    /// Latenz-Perzentile in Millisekunden.
    pub p50_ms: f64,
    /// 95. Perzentil.
    pub p95_ms: f64,
    /// 99. Perzentil.
    pub p99_ms: f64,
    /// Antworten mit unerwartetem Status oder I/O-Fehler.
    pub errors: usize,
    /// Übertragene Body-Bytes.
    pub bytes: u64,
    /// Zweiter Abruf derselben Seiten war byte-identisch.
    pub deterministic: bool,
}

fn percentile(sorted: &[u32], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    f64::from(sorted[idx]) / 1000.0
}

/// Startet die Site des Szenarios und misst Durchsatz und Latenz.
pub async fn probe_server(
    def: &ScenarioDef,
    size: Size,
    params: ProbeParams,
) -> io::Result<(ServerMetrics, SyntheticSite)> {
    let cfg = def.config(size);
    let site = TestSite::start_synthetic(cfg.clone(), SiteOptions::default()).await?;
    let model =
        SyntheticSite::new(cfg).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let pages = model.pages();
    let addr = site.addr();
    let connections = params.connections.max(1);

    let start = Instant::now();
    let mut tasks = Vec::with_capacity(connections);
    for c in 0..connections {
        let model = model.clone();
        let total = params.requests;
        tasks.push(tokio::spawn(async move {
            let mut client = HttpClient::connect(addr).await?;
            let mut latencies = Vec::new();
            let (mut errors, mut bytes) = (0usize, 0u64);
            for i in (c..total).step_by(connections) {
                // Gleichmäßig über die ganze Site verteilt (Primzahl-Schritt).
                let id = (i as u64).wrapping_mul(49_999) % pages;
                let expected = model.page_info(id).status.code();
                let t = Instant::now();
                match client.get(&SyntheticSite::page_path(id)).await {
                    Ok(resp) => {
                        bytes += resp.body.len() as u64;
                        if resp.status != expected {
                            errors += 1;
                        }
                    }
                    Err(_) => errors += 1,
                }
                latencies.push(t.elapsed().as_micros().min(u128::from(u32::MAX)) as u32);
            }
            Ok::<_, io::Error>((latencies, errors, bytes))
        }));
    }
    let mut latencies: Vec<u32> = Vec::with_capacity(params.requests);
    let (mut errors, mut bytes) = (0usize, 0u64);
    for t in tasks {
        let (l, e, b) = t.await.map_err(|e| io::Error::other(e.to_string()))??;
        latencies.extend(l);
        errors += e;
        bytes += b;
    }
    let seconds = start.elapsed().as_secs_f64();
    latencies.sort_unstable();

    // Determinismus: dieselben Seiten ein zweites Mal abrufen und vergleichen.
    let mut client = HttpClient::connect(addr).await?;
    let mut deterministic = true;
    for id in (0..pages).step_by((pages / 50).max(1) as usize).take(50) {
        let path = SyntheticSite::page_path(id);
        let a = client.get(&path).await?;
        let b = client.get(&path).await?;
        deterministic &= a.body == b.body && a.status == b.status;
    }

    Ok((
        ServerMetrics {
            requests: params.requests,
            seconds,
            requests_per_s: params.requests as f64 / seconds.max(f64::EPSILON),
            p50_ms: percentile(&latencies, 0.50),
            p95_ms: percentile(&latencies, 0.95),
            p99_ms: percentile(&latencies, 0.99),
            errors,
            bytes,
            deterministic,
        },
        model,
    ))
}

/// Führt ein Szenario aus und baut den Bericht.
pub async fn run(
    def: &ScenarioDef,
    size: Size,
    params: ProbeParams,
    crawler: &dyn CrawlerUnderTest,
) -> io::Result<Report> {
    let started_at = now_rfc3339();
    let (server, model) = probe_server(def, size, params).await?;

    let mut metrics: BTreeMap<String, Value> = BTreeMap::new();
    metrics.insert("perf_id".into(), json!(def.perf_id));
    metrics.insert(
        "build_profile".into(),
        json!(if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }),
    );
    metrics.insert("site_pages".into(), json!(model.pages()));
    metrics.insert(
        "site_links_per_page".into(),
        json!(model.config().links_per_page),
    );
    metrics.insert("site_depth_levels".into(), json!(model.level_sizes().len()));
    metrics.insert("server_requests".into(), json!(server.requests));
    metrics.insert("server_seconds".into(), json!(round(server.seconds, 3)));
    metrics.insert(
        "server_requests_per_s".into(),
        json!(round(server.requests_per_s, 1)),
    );
    metrics.insert(
        "server_latency_p50_ms".into(),
        json!(round(server.p50_ms, 3)),
    );
    metrics.insert(
        "server_latency_p95_ms".into(),
        json!(round(server.p95_ms, 3)),
    );
    metrics.insert(
        "server_latency_p99_ms".into(),
        json!(round(server.p99_ms, 3)),
    );
    metrics.insert("server_errors".into(), json!(server.errors));
    metrics.insert("server_body_bytes".into(), json!(server.bytes));
    metrics.insert("server_deterministic".into(), json!(server.deterministic));

    // Durchsatz nur im Release-Build bewerten; langsame Seiten (delay) verfälschen ihn bewusst.
    let rps_applies = !cfg!(debug_assertions) && model.config().delay_ms == 0;
    let server_ok = server.errors == 0
        && server.deterministic
        && (!rps_applies || server.requests_per_s >= MIN_SERVER_RPS);

    let mut crawler_ok = true;
    metrics.insert("crawler".into(), json!(crawler.name()));
    let site = TestSite::start_synthetic(model.config().clone(), SiteOptions::default()).await?;
    let job = CrawlJob {
        scenario: def.name.to_string(),
        size,
        start_url: site.url("/"),
        expected_urls: model.pages(),
        site: model.config().clone(),
    };
    match crawler.crawl(job).await {
        Ok(outcome) => {
            metrics.insert("crawler_available".into(), json!(true));
            metrics.insert("crawler_urls_crawled".into(), json!(outcome.urls_crawled));
            metrics.insert(
                "crawler_seconds".into(),
                json!(round(outcome.duration.as_secs_f64(), 3)),
            );
            if let Some(rss) = outcome.peak_rss_bytes {
                metrics.insert("crawler_peak_rss_bytes".into(), json!(rss));
            }
            for (k, v) in outcome.extra {
                metrics.insert(format!("crawler_{k}"), v);
            }
            metrics.insert("crawler_violations".into(), json!(outcome.violations));
            metrics.insert("crawler_warnings".into(), json!(outcome.warnings));
            crawler_ok = outcome.urls_crawled >= model.pages() && outcome.violations.is_empty();
        }
        Err(CrawlerError::NotImplemented) => {
            metrics.insert("crawler_available".into(), json!(false));
        }
        Err(CrawlerError::Failed(msg)) => {
            metrics.insert("crawler_available".into(), json!(true));
            metrics.insert("crawler_error".into(), json!(msg));
            crawler_ok = false;
        }
    }

    Ok(Report {
        scenario: def.name.to_string(),
        size,
        started_at,
        metrics,
        passed: server_ok && crawler_ok,
    })
}

fn round(v: f64, digits: i32) -> f64 {
    let f = 10f64.powi(digits);
    (v * f).round() / f
}
