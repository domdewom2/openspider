//! Testhelfer für alle Pakete, die gegen einen echten Crawl der Fixtures testen (Feature `testing`).
//!
//! **Stabile API für Tab-, Export-, MCP- und CLI-Pakete:**
//! * [`test_deps`] – deterministische Abhängigkeiten (`FixedClock`, `SeededIds(42)`, Profil „Ausgewogen“,
//!   `MemorySecretStore`, keine eigene Senke, kein Hook, `owner_kind = "cli"`).
//! * [`crawl_fixture`], [`crawl_fixture_from`], [`crawl_fixture_with`] – starten die Testsite zum Szenario,
//!   setzen `network.resolve` für `*.test`, crawlen vollständig in eine temporäre Crawl-Datei und liefern sie
//!   geöffnet zurück ([`FixtureCrawl`]). `start_path` darf auch eine vollständige URL sein
//!   (`http://www.site.test/`), die dann über die Resolve-Regel auf die Testsite zeigt.
//! * [`crawl_dir_with`] – wie `crawl_fixture_with` für ein beliebiges Verzeichnis statt eines Szenarionamens.
//!
//! **Hilfen für Zeit- und Ablauftests ohne Sockets** (Regel 2.8, simulierte Zeit mit
//! `#[tokio::test(start_paused = true)]`):
//! * [`TokioClock`] – `Clock` auf Basis der tokio-Zeit.
//! * [`SimSite`] / [`SimFetcher`] – ein `Fetch` über eine Website im Speicher mit einstellbarer Verzögerung
//!   je Seite, Request-Log mit simulierten Zeiten und Zähler für gleichzeitige Abrufe.
//! * [`MemorySink`] – eine `CrawlSink`, die die Batches im Speicher sammelt (optional mit Verzögerung).

use std::collections::{BTreeMap, HashMap};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use os_fetch::{
    Fetch, FetchConfig, FetchOutcome, FetchPurpose, FetchRequest, FetchResult, HttpFetcher,
    HttpResponse, MemorySecretStore,
};
use os_model::{
    Clock, CrawlConfig, FixedClock, NewUrl, PageWrite, SeededIds, Timestamp, WriteBatch,
};
use os_store::{CrawlFile, CrawlSink, OpenMode, StorageProfile, StoreError};

use crate::crawl::start_crawl;
use crate::types::{CrawlRequest, CrawlSource, CrawlSummary, EngineDeps, SinkFactory};

/// Ergebnis von `crawl_fixture*`: laufende Testsite, geöffnete Crawl-Datei und Arbeitsverzeichnis.
pub struct FixtureCrawl {
    /// Laufende Testsite (wird beim Drop beendet).
    pub site: os_testsite::TestSite,
    /// Die Crawl-Datei (lesend und schreibend geöffnet).
    pub file: CrawlFile,
    /// Arbeitsverzeichnis mit der Crawl-Datei (wird beim Drop gelöscht).
    pub dir: tempfile::TempDir,
    /// Zusammenfassung des beendeten Crawls.
    pub summary: CrawlSummary,
}

/// Deterministische Abhängigkeiten: `FixedClock` (ab 2026-01-01, 1 ms je Abfrage), `SeededIds(42)`,
/// Profil „Ausgewogen“, `MemorySecretStore`, keine eigene Senke, kein Hook.
pub fn test_deps() -> EngineDeps {
    let start = Timestamp::from_db("2026-01-01T00:00:00.000Z").expect("gültiger Startzeitpunkt");
    EngineDeps {
        clock: Arc::new(FixedClock::new(start, Duration::from_millis(1))),
        ids: Arc::new(SeededIds::new(42)),
        profile: StorageProfile::Balanced,
        secrets: Arc::new(MemorySecretStore::new()),
        sink_factory: None,
        on_finished: None,
        owner_kind: "cli",
    }
}

/// Crawlt das Szenario ab der Basis-URL der Testsite.
pub async fn crawl_fixture(scenario: &str, cfg: CrawlConfig) -> FixtureCrawl {
    crawl_fixture_from(scenario, "/", cfg).await
}

/// Crawlt das Szenario ab `start_path` (Pfad auf der Testsite oder vollständige URL).
pub async fn crawl_fixture_from(
    scenario: &str,
    start_path: &str,
    cfg: CrawlConfig,
) -> FixtureCrawl {
    crawl_fixture_with(scenario, start_path, cfg, test_deps()).await
}

/// Wie `crawl_fixture_from`, aber mit eigenen Abhängigkeiten (Hook, langsame Senke, `ManualClock`).
pub async fn crawl_fixture_with(
    scenario: &str,
    start_path: &str,
    cfg: CrawlConfig,
    deps: EngineDeps,
) -> FixtureCrawl {
    let site = os_testsite::TestSite::start(scenario)
        .await
        .unwrap_or_else(|e| panic!("Testsite {scenario:?} startet nicht: {e}"));
    crawl_site(site, scenario, start_path, cfg, deps).await
}

/// Wie `crawl_fixture_with` für ein Szenario-Verzeichnis außerhalb von `tests/fixtures` (z. B. ein
/// temporäres Verzeichnis mit eigener `scenario.toml`).
pub async fn crawl_dir_with(
    dir: &Path,
    start_path: &str,
    cfg: CrawlConfig,
    deps: EngineDeps,
) -> FixtureCrawl {
    let site = os_testsite::TestSite::start_dir(dir, 0)
        .await
        .unwrap_or_else(|e| panic!("Testsite {} startet nicht: {e}", dir.display()));
    let name = dir
        .file_name()
        .map_or_else(|| "dir".to_string(), |n| n.to_string_lossy().into_owned());
    crawl_site(site, &name, start_path, cfg, deps).await
}

async fn crawl_site(
    site: os_testsite::TestSite,
    name: &str,
    start_path: &str,
    mut cfg: CrawlConfig,
    deps: EngineDeps,
) -> FixtureCrawl {
    cfg.network.resolve.push(format!("*.test={}", site.addr()));
    let dir = tempfile::tempdir().expect("temporäres Verzeichnis");
    let path = dir.path().join("crawl.ospider");

    let mut fetch_cfg = FetchConfig::from_crawl_config(&cfg, &*deps.secrets, deps.clock.clone())
        .expect("Abruf-Konfiguration");
    fetch_cfg.extra_root_certs = vec![site.ca_cert_pem().into_bytes()];
    let fetcher = Arc::new(HttpFetcher::new(fetch_cfg).expect("HTTP-Abrufer"));

    let start_url = if start_path.starts_with("http://") || start_path.starts_with("https://") {
        start_path.to_string()
    } else {
        site.url(start_path)
    };
    let request = CrawlRequest {
        config: cfg,
        source: CrawlSource::Spider { start_url },
        file: path.clone(),
        project: "test".to_string(),
        name: name.to_string(),
    };
    let handle = start_crawl(fetcher, request, deps)
        .await
        .unwrap_or_else(|e| panic!("Crawl startet nicht: {e}"));
    let summary = handle
        .wait()
        .await
        .unwrap_or_else(|e| panic!("Crawl schlägt fehl: {e}"));
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).expect("Crawl-Datei öffnen");
    FixtureCrawl {
        site,
        file,
        dir,
        summary,
    }
}

// ---------------------------------------------------------------------------------------------------
// Simulierte Zeit

/// Uhr auf Basis der tokio-Zeit: läuft unter `#[tokio::test(start_paused = true)]` mit der simulierten
/// Zeit. `now()` ist die Wanduhr (für Zeitstempel), `monotonic()` die simulierte Zeit seit dem Anlegen.
pub struct TokioClock {
    start: tokio::time::Instant,
}

impl TokioClock {
    /// Beginnt jetzt (simulierte) Zeit 0.
    pub fn new() -> Arc<Self> {
        Arc::new(TokioClock {
            start: tokio::time::Instant::now(),
        })
    }
}

impl Clock for TokioClock {
    fn now(&self) -> Timestamp {
        Timestamp::now_utc()
    }

    fn monotonic(&self) -> Duration {
        self.start.elapsed()
    }
}

/// Eine Seite der simulierten Website.
#[derive(Debug, Clone)]
pub struct SimPage {
    /// Statuscode.
    pub status: u16,
    /// Antwort-Header (Namen klein).
    pub headers: Vec<(String, String)>,
    /// Body.
    pub body: String,
    /// Eigene Verzögerung (sonst die der Website).
    pub delay: Option<Duration>,
}

/// Eine Website im Speicher.
#[derive(Debug, Clone, Default)]
pub struct SimSite {
    pages: HashMap<String, SimPage>,
    delay: Duration,
}

impl SimSite {
    /// Leere Website (alle URLs antworten mit 404).
    pub fn new() -> Self {
        SimSite::default()
    }

    /// Verzögerung jeder Antwort.
    pub fn delay(mut self, d: Duration) -> Self {
        self.delay = d;
        self
    }

    /// Eine Seite beliebiger Art.
    pub fn with(mut self, url: &str, page: SimPage) -> Self {
        let key = url::Url::parse(url)
            .unwrap_or_else(|e| panic!("{url}: {e}"))
            .to_string();
        self.pages.insert(key, page);
        self
    }

    /// HTML-Seite (200).
    pub fn page(self, url: &str, html: &str) -> Self {
        self.asset(url, "text/html; charset=utf-8", html)
    }

    /// Datei mit Content-Type (200).
    pub fn asset(self, url: &str, content_type: &str, body: &str) -> Self {
        self.with(
            url,
            SimPage {
                status: 200,
                headers: vec![("content-type".to_string(), content_type.to_string())],
                body: body.to_string(),
                delay: None,
            },
        )
    }

    /// Weiterleitung.
    pub fn redirect(self, url: &str, status: u16, location: &str) -> Self {
        self.with(
            url,
            SimPage {
                status,
                headers: vec![("location".to_string(), location.to_string())],
                body: String::new(),
                delay: None,
            },
        )
    }

    /// Ternärer Baum aus `pages` Seiten (Wurzel `/`, danach `/page-<n>.html`; Seite *k* verlinkt auf die
    /// Seiten 3k+1 … 3k+3 und zurück auf `/`) plus 10 Bilder, 3 CSS- und 3 JS-Dateien an den Seiten 1 bis 16
    /// – derselbe Aufbau wie die Fixture `crawl-site-100`. Die Wurzel hat Tiefe 0, bei 100 Seiten ist die
    /// größte Tiefe 4.
    pub fn tree(host: &str, pages: usize) -> Self {
        let mut site = SimSite::new();
        let path = |n: usize| {
            if n == 0 {
                "/".to_string()
            } else {
                format!("/page-{n}.html")
            }
        };
        for n in 0..pages {
            let mut head = String::new();
            let mut body = String::new();
            if (1..=10).contains(&n) {
                body.push_str(&format!("<img src=\"/img/i{n}.png\" alt=\"Bild {n}\">"));
            }
            if (11..=13).contains(&n) {
                head.push_str(&format!(
                    "<link rel=\"stylesheet\" href=\"/css/c{}.css\">",
                    n - 10
                ));
            }
            if (14..=16).contains(&n) {
                head.push_str(&format!("<script src=\"/js/j{}.js\"></script>", n - 13));
            }
            body.push_str("<a href=\"/\">Start</a>");
            for c in (3 * n + 1)..=(3 * n + 3) {
                if c < pages {
                    body.push_str(&format!(" <a href=\"{}\">Seite {c}</a>", path(c)));
                }
            }
            let html = format!(
                "<!doctype html><html><head><title>Seite {n}</title>{head}</head>\
                 <body><h1>Seite {n}</h1>{body}</body></html>"
            );
            site = site.page(&format!("https://{host}{}", path(n)), &html);
        }
        for n in 1..=10 {
            site = site.asset(&format!("https://{host}/img/i{n}.png"), "image/png", "PNG");
        }
        for n in 1..=3 {
            site = site.asset(&format!("https://{host}/css/c{n}.css"), "text/css", "a{}");
            site = site.asset(
                &format!("https://{host}/js/j{n}.js"),
                "application/javascript",
                "var a=1;",
            );
        }
        site
    }
}

/// Ein protokollierter Abruf.
#[derive(Debug, Clone)]
pub struct SimRequest {
    /// Laufende Nummer ab 0 (Reihenfolge der Aufrufe).
    pub seq: usize,
    /// Abgerufene URL.
    pub url: String,
    /// Zweck.
    pub purpose: FetchPurpose,
    /// Simulierte Zeit des Beginns.
    pub start: Duration,
    /// Simulierte Zeit des Endes.
    pub end: Duration,
    /// Gleichzeitig laufende Abrufe beim Beginn (diesen eingeschlossen).
    pub concurrent: usize,
}

/// `Fetch` über eine [`SimSite`]: kein Netz, Verzögerung über `tokio::time::sleep` (simulierbar).
pub struct SimFetcher {
    site: SimSite,
    start: tokio::time::Instant,
    log: Mutex<Vec<SimRequest>>,
    inflight: AtomicUsize,
    max_inflight: AtomicUsize,
}

impl SimFetcher {
    /// Legt den Abrufer an; die Zeit der Protokolleinträge beginnt jetzt.
    pub fn new(site: SimSite) -> Arc<Self> {
        Arc::new(SimFetcher {
            site,
            start: tokio::time::Instant::now(),
            log: Mutex::new(Vec::new()),
            inflight: AtomicUsize::new(0),
            max_inflight: AtomicUsize::new(0),
        })
    }

    /// Alle Abrufe in Aufrufreihenfolge.
    pub fn requests(&self) -> Vec<SimRequest> {
        self.log.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// Abrufe ohne robots.txt.
    pub fn page_requests(&self) -> Vec<SimRequest> {
        self.requests()
            .into_iter()
            .filter(|r| r.purpose != FetchPurpose::RobotsTxt)
            .collect()
    }

    /// Wie oft die URL abgerufen wurde.
    pub fn count(&self, url: &str) -> usize {
        self.requests().iter().filter(|r| r.url == url).count()
    }

    /// Höchste Zahl gleichzeitiger Abrufe.
    pub fn max_concurrency(&self) -> usize {
        self.max_inflight.load(Ordering::SeqCst)
    }

    /// Gleichzeitige Abrufe jetzt.
    pub fn inflight(&self) -> usize {
        self.inflight.load(Ordering::SeqCst)
    }
}

impl Fetch for SimFetcher {
    fn fetch(&self, req: FetchRequest) -> Pin<Box<dyn Future<Output = FetchResult> + Send + '_>> {
        Box::pin(async move {
            let now = self.inflight.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_inflight.fetch_max(now, Ordering::SeqCst);
            let start = self.start.elapsed();
            let seq = {
                let mut log = self.log.lock().unwrap_or_else(|p| p.into_inner());
                let seq = log.len();
                log.push(SimRequest {
                    seq,
                    url: req.url.to_string(),
                    purpose: req.purpose,
                    start,
                    end: start,
                    concurrent: now,
                });
                seq
            };
            let page = self.site.pages.get(req.url.as_str()).cloned();
            let (status, headers, body, own_delay) = match page {
                Some(p) => (p.status, p.headers, p.body, p.delay),
                None => (
                    404,
                    vec![("content-type".to_string(), "text/html".to_string())],
                    "nicht gefunden".to_string(),
                    None,
                ),
            };
            // robots.txt antwortet sofort (der Zeitbedarf der Seiten soll nicht davon abhängen)
            let delay = if req.purpose == FetchPurpose::RobotsTxt {
                Duration::ZERO
            } else {
                own_delay.unwrap_or(self.site.delay)
            };
            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }
            let end = self.start.elapsed();
            self.log.lock().unwrap_or_else(|p| p.into_inner())[seq].end = end;
            self.inflight.fetch_sub(1, Ordering::SeqCst);
            let size = body.len() as u64;
            FetchResult {
                url: req.url,
                started_at: Timestamp::now_utc(),
                outcome: FetchOutcome::Response(HttpResponse {
                    status,
                    reason_phrase: None,
                    status_text: os_fetch::http::status_text(status, None),
                    http_version: "HTTP/1.1".to_string(),
                    headers,
                    body: Bytes::from(body),
                    size_bytes: size,
                    transferred_bytes: size + 100,
                    response_time_ms: delay.as_millis() as u32,
                    truncated: false,
                    decode_error: None,
                    md5_hex: "0".repeat(32),
                }),
            }
        })
    }
}

/// Senke, die die Batches im Speicher sammelt; optional mit künstlicher Verzögerung je Batch
/// (echte Zeit, der Writer-Thread ist ein Betriebssystem-Thread).
#[derive(Clone, Default)]
pub struct MemorySink {
    batches: Arc<Mutex<Vec<WriteBatch>>>,
    delay: Duration,
}

impl MemorySink {
    /// Sammelnde Senke ohne Verzögerung.
    pub fn new() -> Self {
        MemorySink::default()
    }

    /// Senke mit Verzögerung je Batch (PERF-003: 100 ms).
    pub fn slow(delay: Duration) -> Self {
        MemorySink {
            batches: Arc::default(),
            delay,
        }
    }

    /// Fabrik für `EngineDeps::sink_factory`.
    pub fn factory(&self) -> SinkFactory {
        let sink = self.clone();
        Arc::new(move |_path, _profile| Ok(Box::new(sink.clone()) as Box<dyn CrawlSink>))
    }

    /// Alle geschriebenen Batches.
    pub fn batches(&self) -> Vec<WriteBatch> {
        self.batches
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// Alle geschriebenen Seiten in Schreibreihenfolge.
    pub fn pages(&self) -> Vec<PageWrite> {
        self.batches()
            .into_iter()
            .flat_map(|b| b.pages.into_iter())
            .collect()
    }

    /// Alle neu angelegten URLs in Schreibreihenfolge.
    pub fn new_urls(&self) -> Vec<NewUrl> {
        self.batches()
            .into_iter()
            .flat_map(|b| b.new_urls.into_iter())
            .collect()
    }

    /// Alle Zähler-Deltas, aufsummiert.
    pub fn counters(&self) -> BTreeMap<String, i64> {
        let mut out = BTreeMap::new();
        for b in self.batches() {
            for (k, v) in b.counters {
                *out.entry(k).or_insert(0) += v;
            }
        }
        out
    }
}

impl CrawlSink for MemorySink {
    fn write(&mut self, batch: WriteBatch) -> Result<(), StoreError> {
        if !self.delay.is_zero() {
            std::thread::sleep(self.delay);
        }
        self.batches
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(batch);
        Ok(())
    }

    fn checkpoint(&mut self) -> Result<(), StoreError> {
        Ok(())
    }
}

/// Abhängigkeiten für Tests mit simulierter Zeit: `TokioClock`, Senke im Speicher.
pub fn sim_deps(sink: &MemorySink) -> EngineDeps {
    let mut deps = test_deps();
    deps.clock = TokioClock::new();
    deps.sink_factory = Some(sink.factory());
    deps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deps_are_deterministic() {
        let a = test_deps();
        let b = test_deps();
        let now = a.clock.now();
        assert_eq!(a.ids.new_crawl_id(now), b.ids.new_crawl_id(b.clock.now()));
        assert!(a.sink_factory.is_none() && a.on_finished.is_none());
        assert_eq!(a.owner_kind, "cli");
    }

    #[test]
    fn tree_has_the_documented_shape() {
        let site = SimSite::tree("site.test", 100);
        // 100 Seiten + 10 Bilder + 3 CSS + 3 JS
        assert_eq!(site.pages.len(), 116);
        let root = &site.pages["https://site.test/"].body;
        assert!(root.contains("/page-1.html") && root.contains("/page-3.html"));
        let leaf = &site.pages["https://site.test/page-99.html"].body;
        assert!(!leaf.contains("page-100"), "keine Kinder hinter Seite 99");
    }
}
