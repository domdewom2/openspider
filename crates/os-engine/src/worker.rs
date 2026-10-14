//! Worker: Abruf, Auswertung und Schreibmodell je URL (Datenfluss M1-Plan 3.7).
//!
//! Ablauf je URL (`process`):
//! 1. robots.txt prüfen (`RobotsGate`); beim Erstabruf je Origin entsteht eine URL-Zeile für die robots.txt
//!    (`DiscoveryKind::RobotsTxt`, zählt nicht). Gesperrte URL → Zeile mit Status 0, kein Abruf.
//! 2. Abrufen (`Fetch`), Fakten (`FetchFacts`) aus der Antwort bilden.
//! 3. Interne HTML-Seiten parsen (`parse_html`), CSS nach `url()`/`@import` durchsuchen, bei allem anderen nur
//!    die Header auswerten. Externe URLs und 3xx-Bodys werden nie geparst.
//! 4. Weiterleitung bestimmen: HTTP-3xx vor Refresh-Header vor Meta-Refresh.
//! 5. Alle Ziele (Links, Bilder, Canonical, hreflang, Paginierung, Weiterleitung) entdecken: Entscheidung über
//!    `decide::decide`, Include/Exclude, Tiefe; neue URLs bekommen ihre Kennung und werden eingeplant.
//! 6. `PageWrite` samt Kindzeilen und Indexierbarkeit an den Writer.
//!
//! **Reihenfolge im Kanal:** Die neuen URLs (und ihre Queue-Einträge) gehen in einer eigenen Nachricht vor der
//! Seite in den Kanal, und zwar *unter der Sperre* über einen vorab reservierten Platz. So kann kein anderer
//! Worker eine neue URL abrufen und ihre `PageWrite` vor der `NewUrl`-Zeile senden.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use os_fetch::robots::{RobotsDecision, RobotsGate};
use os_fetch::{content_class, Fetch, FetchOutcome, FetchRequest, FetchResult, HttpResponse};
use os_model::{
    CanonicalRow, Clock, ContentClass, CrawlConfig, DirectiveRow, DiscoveryKind, ElementPair,
    ExtractedLink, ExtractedText, FetchFacts, HeaderHints, HeaderRow, HintSource, HreflangRef,
    HreflangRow, ImageRow, Indexability, IndexabilityInput, LinkOrigin, LinkPosition, LinkRow,
    LinkType, NoResponseReason, NotCrawledReason, PageFacts, PageWrite, PaginationRow, ParsedPage,
    PathType, PdfMeta, QueueOp, RedirectType, RefreshRow, RefreshSource, SitemapId, SitemapKind,
    SitemapWrite, Timestamp, UrlId, UrlState, WriteBatch,
};
use os_parse::{ParseConfig, ParseInput};
use os_url::{NormUrl, ScopeClass};
use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore};
use url::Url;

use crate::decide::{self, LinkDecision};
use crate::diag;
use crate::error::EngineError;
use crate::frontier::{
    Cand, Classify, Core, CrawlScope, Purpose, QueueItem, COUNTER_REDIRECT_LIMIT, F_QUEUED,
    F_RULE_OK,
};
use crate::progress::ProgressTracker;
use crate::redirect::{self, RedirectTarget};
use crate::sitemaps::{self, FetchedSitemap, SitemapSeed};
use crate::writer_thread::{WriterGone, WriterSender};

/// Ab dieser Body-Größe wird auf einem Blocking-Thread geparst.
const BLOCKING_PARSE_THRESHOLD: usize = 1024 * 1024;

/// Steuerung von außen (`CrawlHandle`) und zwischen Worker und Steuerschleife.
pub(crate) struct Control {
    /// Weckt die Steuerschleife (neue Arbeit, Worker fertig, Einstellung geändert).
    pub notify: Notify,
    /// Gewünschte Zahl paralleler Threads.
    pub threads: AtomicU32,
    /// `STOP_*`.
    pub stop: AtomicU8,
    /// Grund einer Pause, die die Engine selbst auslöst (Speicherplatz): `(Code, Meldung)`.
    pub reason: Mutex<Option<(&'static str, String)>>,
}

/// Kein Stopp angefordert.
pub(crate) const STOP_NONE: u8 = 0;
/// Beenden, Warteschlange bleibt erhalten.
pub(crate) const STOP_STOP: u8 = 1;
/// Pausieren (Zustand `Paused`, kein Abschluss-Hook).
pub(crate) const STOP_PAUSE: u8 = 2;

/// Zustand, den alle Worker und die Steuerschleife teilen.
pub(crate) struct Shared<F: Fetch> {
    pub fetcher: Arc<F>,
    pub cfg: Arc<CrawlConfig>,
    pub clock: Arc<dyn Clock>,
    pub scope: CrawlScope,
    pub rules: os_url::RuleSet,
    pub gate: RobotsGate,
    pub pcfg: ParseConfig,
    pub core: Mutex<Core>,
    pub ctl: Arc<Control>,
    pub writer: WriterSender,
    pub progress: Mutex<ProgressTracker>,
    /// Ein Worker hat einen nicht behebbaren Fehler gemeldet (oder der Writer ist ausgefallen).
    pub abort: AtomicBool,
    pub error: Mutex<Option<EngineError>>,
    pub host_gates: Mutex<HashMap<String, Arc<Semaphore>>>,
    pub max_per_host: Option<u32>,
    /// URL Encoded Address der Start-URL (von Include/Exclude ausgenommen; im Listenmodus leer).
    pub start_url: String,
    /// Origins, deren robots.txt aus der Crawl-Datei übernommen wurde (Fortsetzen): kein erneuter Abruf, kein
    /// neues Schreiben (P19).
    pub primed_origins: std::collections::HashSet<String>,
}

impl<F: Fetch> Shared<F> {
    /// Meldet einen Fehler; der erste gewinnt, der Crawl endet.
    pub fn fail(&self, e: EngineError) {
        let mut slot = self.error.lock().unwrap_or_else(|p| p.into_inner());
        if slot.is_none() {
            *slot = Some(e);
        }
        self.abort.store(true, Ordering::SeqCst);
        self.ctl.notify.notify_one();
    }

    fn lock_core(&self) -> std::sync::MutexGuard<'_, Core> {
        self.core.lock().unwrap_or_else(|p| p.into_inner())
    }

    async fn host_permit(&self, url: &Url) -> Option<OwnedSemaphorePermit> {
        let cap = self.max_per_host?;
        let key = format!(
            "{}:{}",
            url.host_str().unwrap_or(""),
            url.port_or_known_default().unwrap_or(0)
        );
        let sem = {
            let mut gates = self.host_gates.lock().unwrap_or_else(|p| p.into_inner());
            Arc::clone(
                gates
                    .entry(key)
                    .or_insert_with(|| Arc::new(Semaphore::new(cap.max(1) as usize))),
            )
        };
        sem.acquire_owned().await.ok()
    }

    pub(crate) fn clock_mono(&self) -> std::time::Duration {
        self.clock.monotonic()
    }
}

struct ActiveGuard<F: Fetch> {
    sh: Arc<Shared<F>>,
}

impl<F: Fetch> Drop for ActiveGuard<F> {
    fn drop(&mut self) {
        self.sh.lock_core().finish_worker();
        self.sh.ctl.notify.notify_one();
    }
}

/// Bearbeitet eine URL der Warteschlange (der Abruf ist bereits reserviert, `active` erhöht).
pub(crate) async fn process<F: Fetch>(sh: Arc<Shared<F>>, item: QueueItem) {
    let _guard = ActiveGuard {
        sh: Arc::clone(&sh),
    };
    let started = std::time::Instant::now();
    let outcome = run_item(&sh, &item).await;
    diag::inc(&diag::STATS.items);
    diag::add(&diag::STATS.item_ns, started);
    match outcome {
        Ok(()) => {}
        Err(Failure::Writer) => {
            sh.abort.store(true, Ordering::SeqCst);
        }
        Err(Failure::Engine(e)) => sh.fail(e),
    }
}

pub(crate) enum Failure {
    /// Der Writer ist ausgefallen; die Ursache liefert `WriterThread::finish`.
    Writer,
    Engine(EngineError),
}

impl From<WriterGone> for Failure {
    fn from(_: WriterGone) -> Self {
        Failure::Writer
    }
}

async fn run_item<F: Fetch>(sh: &Arc<Shared<F>>, item: &QueueItem) -> Result<(), Failure> {
    let url = Url::parse(&item.url)
        .map_err(|e| Failure::Engine(EngineError::Invalid(format!("{}: {e}", item.url))))?;
    let cfg = &*sh.cfg;
    let t = std::time::Instant::now();
    let _host = sh.host_permit(&url).await;
    diag::add(&diag::STATS.host_wait_ns, t);

    // 1. robots.txt
    let t = std::time::Instant::now();
    let check = sh.gate.check(&*sh.fetcher, &url).await;
    diag::add(&diag::STATS.robots_ns, t);
    let mut sitemap_seeds = Vec::new();
    if let Some((fetch_result, write)) = check.robots_fetch {
        sitemap_seeds = record_robots(sh, fetch_result, write).await?;
    }
    let internal = item.purpose != Purpose::External;
    match &check.decision {
        RobotsDecision::Blocked { .. } => {
            let facts = crate::robots_glue::blocked_fetch_facts(&check.decision, &*sh.clock);
            sh.lock_core().refund();
            let counter = if internal {
                "robots_blocked_internal"
            } else {
                "robots_blocked_external"
            };
            let mut batch = WriteBatch::default();
            batch.counters.push((counter.to_string(), 1));
            batch.pages.push(PageWrite {
                url_id: item.id,
                replace: item.replace,
                indexability: Indexability::NonIndexable,
                indexability_status: None,
                fetch: facts,
                page: None,
                links: vec![],
                images: vec![],
                canonicals: vec![],
                directives: vec![],
                refresh: vec![],
                hreflang: vec![],
                pagination: vec![],
                headers: vec![],
            });
            let page = &mut batch.pages[0];
            let (ind, status) = os_model::evaluate(&IndexabilityInput {
                fetch: &page.fetch,
                page: None,
                page_url: &item.url,
                refresh: &[],
                cfg,
                same_url: os_url::eq_for_compare,
            });
            page.indexability = ind;
            page.indexability_status = status;
            batch.queue.push(QueueOp::Pop { id: item.id });
            sh.writer.send(batch).await?;
            sh.progress
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .record_completion(sh.clock_mono(), None);
            sh.ctl.notify.notify_one();
            return Ok(());
        }
        RobotsDecision::Allowed | RobotsDecision::ReportOnly { .. } => {}
    }

    // 2. Abruf
    let cookies = sh.lock_core().session_jar(item.session);
    let t = std::time::Instant::now();
    let result = sh
        .fetcher
        .fetch(FetchRequest {
            url: url.clone(),
            purpose: item.purpose.fetch_purpose(),
            cookies,
            max_body_bytes: cfg.limits.max_page_kb.saturating_mul(1024),
        })
        .await;
    let fetched = diag::add_max(&diag::STATS.fetch_ns, &diag::STATS.fetch_max_ns, t);
    if fetched >= diag::SLOW_FETCH_NS {
        diag::inc(&diag::STATS.fetch_slow);
    }

    // 3. Auswertung
    let t = std::time::Instant::now();
    let mut analysis = analyze(sh, item, &url, &result).await;
    diag::add(&diag::STATS.analyze_ns, t);
    if let RobotsDecision::ReportOnly {
        would_block,
        matched_line,
        robots_url,
    } = &check.decision
    {
        analysis.facts.robots_blocked = *would_block;
        analysis.facts.robots_matched_line = matched_line.clone();
        analysis.facts.robots_txt_url = Some(robots_url.clone());
    }

    // 4./5. Ziele entdecken und Schreibmodell bilden
    let response_ms = analysis
        .facts
        .response_time_ms
        .filter(|_| analysis.facts.status_code != 0);
    let t = std::time::Instant::now();
    let batch = assemble(sh, item, analysis).await?;
    diag::add(&diag::STATS.assemble_ns, t);
    let t = std::time::Instant::now();
    sh.writer.send(batch).await?;
    diag::add_max(&diag::STATS.send_ns, &diag::STATS.send_max_ns, t);
    sh.progress
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .record_completion(sh.clock_mono(), response_ms);
    sh.ctl.notify.notify_one();

    // 6. Sitemaps aus der robots.txt – nicht mehr, wenn schon pausiert oder gestoppt wird: nach der Pause
    // keine neuen Abrufe (CRAWL-126). Beim Fortsetzen leitet das robots-Gate die Adressen erneut ab.
    if !sitemap_seeds.is_empty() && sh.ctl.stop.load(Ordering::SeqCst) == STOP_NONE {
        ingest_sitemaps(sh, sitemap_seeds).await?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------
// robots.txt

/// Legt die URL-Zeile der robots.txt an und gibt die `Sitemap:`-Ausgangspunkte zurück.
async fn record_robots<F: Fetch>(
    sh: &Arc<Shared<F>>,
    result: FetchResult,
    write: os_model::RobotsWrite,
) -> Result<Vec<SitemapSeed>, WriterGone> {
    let cfg = &*sh.cfg;
    let parsed_robots = Url::parse(&write.robots_url).ok();
    let norm_robots = parsed_robots.as_ref().map(os_url::normalize);
    let internal = norm_robots
        .as_ref()
        .is_some_and(|n| sh.scope.classify(n) == ScopeClass::Internal);
    // `Sitemap:`-Zeilen gelten nur für interne Hosts: die Sitemaps fremder Hosts gehören nicht zu diesem Crawl.
    let seeds = if internal {
        write
            .body
            .as_deref()
            .map(|body| {
                let listed: Vec<String> = os_fetch::robots::parse(body).sitemaps().to_vec();
                // relative Angaben (`Sitemap: /sitemap_index.xml`) löst die URL der robots.txt auf (CRAWL-058)
                match &parsed_robots {
                    Some(base) => crate::robots_glue::sitemap_seeds_at(base, &listed, cfg),
                    None => crate::robots_glue::sitemap_seeds(&listed, cfg),
                }
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    // Fortsetzen: die robots.txt dieses Origins stammt aus der Crawl-Datei; Zeile und Eintrag bleiben unverändert.
    if sh.primed_origins.contains(&write.origin) {
        return Ok(seeds);
    }

    let mut batch = WriteBatch::default();
    if let Some(norm) = norm_robots {
        let slot = sh.writer.reserve().await?;
        let mut core = sh.lock_core();
        if core.lookup(&norm.encoded).is_none() {
            let id = core.alloc_id();
            // K-54 (CRAWL-098): die robots.txt jedes Hosts, intern wie extern, wird als URL-Zeile ohne
            // Tabellen-Sichtbarkeit gespeichert; sie erscheint in keinem Tab, Filter, Zähler oder Export.
            let flags = F_QUEUED | F_RULE_OK;
            core.register(&norm.encoded, id, None, flags);
            batch.new_urls.push(os_model::NewUrl {
                id,
                url: norm.encoded.clone(),
                address: norm.display.clone(),
                origin: norm.origin.clone(),
                is_internal: internal,
                in_tables: false,
                discovered_via: DiscoveryKind::RobotsTxt,
                crawl_depth: None,
                folder_depth: os_url::folder_depth(&norm.url),
                url_length: os_url::url_length(&norm.display),
                state: UrlState::Done,
                not_crawled_reason: None,
            });
            let mut page = plain_page(id, &result, cfg);
            page.headers = header_rows(&result, cfg);
            batch.pages.push(page);
        }
        batch.robots.push(write);
        slot.send(batch);
        drop(core);
    } else {
        batch.robots.push(write);
        sh.writer.send(batch).await?;
    }
    Ok(seeds)
}

/// `PageWrite` ohne Auswertung (robots.txt, Sitemap-Datei).
fn plain_page(id: UrlId, result: &FetchResult, cfg: &CrawlConfig) -> PageWrite {
    let fetch = facts_from_result(result, cfg);
    let (indexability, indexability_status) = os_model::evaluate(&IndexabilityInput {
        fetch: &fetch,
        page: None,
        page_url: result.url.as_str(),
        refresh: &[],
        cfg,
        same_url: os_url::eq_for_compare,
    });
    PageWrite {
        url_id: id,
        replace: false,
        fetch,
        page: None,
        indexability,
        indexability_status,
        links: vec![],
        images: vec![],
        canonicals: vec![],
        directives: vec![],
        refresh: vec![],
        hreflang: vec![],
        pagination: vec![],
        headers: vec![],
    }
}

fn header_rows(result: &FetchResult, cfg: &CrawlConfig) -> Vec<HeaderRow> {
    if !cfg.extraction.http_headers {
        return Vec::new();
    }
    match &result.outcome {
        FetchOutcome::Response(r) => r
            .headers
            .iter()
            .enumerate()
            .map(|(i, (name, value))| HeaderRow {
                request: false,
                position: i as u32,
                name: name.clone(),
                value: value.clone(),
            })
            .collect(),
        FetchOutcome::NoResponse { .. } => Vec::new(),
    }
}

// ---------------------------------------------------------------------------------------------------
// Fakten aus der Antwort

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

/// Liest ein HTTP-Datum (`Wed, 21 Oct 2015 07:28:00 GMT`, RFC 9110).
pub(crate) fn parse_http_date(s: &str) -> Option<Timestamp> {
    use time::format_description::well_known::Rfc2822;
    time::OffsetDateTime::parse(s.trim(), &Rfc2822)
        .ok()
        .map(|t| Timestamp(t.to_offset(time::UtcOffset::UTC)))
}

/// Bildet die Fakten des Abrufs (CRAWL-011/012/022, K-02: `total_transferred = transferred`).
pub(crate) fn facts_from_result(res: &FetchResult, cfg: &CrawlConfig) -> FetchFacts {
    let mut f = FetchFacts {
        crawl_timestamp: res.started_at,
        ..FetchFacts::default()
    };
    match &res.outcome {
        FetchOutcome::NoResponse { reason, .. } => {
            f.status_code = 0;
            f.status = reason.status_text().to_string();
            f.no_response_reason = Some(*reason);
        }
        FetchOutcome::Response(r) => {
            f.status_code = r.status;
            if r.decode_error.is_some() {
                f.status = NoResponseReason::ConnectionError.status_text().to_string();
                f.no_response_reason = Some(NoResponseReason::ConnectionError);
            } else {
                f.status = r.status_text.clone();
            }
            let ct = header(&r.headers, "content-type").map(str::to_string);
            f.content_class = content_class(ct.as_deref(), cfg.advanced.assume_html);
            f.content_type = ct;
            f.http_version = Some(r.http_version.clone());
            f.size_bytes = r.size_bytes;
            f.transferred_bytes = r.transferred_bytes;
            f.total_transferred_bytes = r.transferred_bytes;
            if cfg.extraction.response_time {
                f.response_time_ms = Some(r.response_time_ms);
            }
            if cfg.extraction.last_modified {
                f.last_modified_raw = header(&r.headers, "last-modified").map(str::to_string);
                f.last_modified = f.last_modified_raw.as_deref().and_then(parse_http_date);
            }
            // ONPAGE-018 (K-60): MD5 nur für HTML-Antworten (Inhaltsklasse HTML) mit Status 2xx, sonst leer. Die
            // Referenz hasht außerdem 2xx-Antworten ohne Content-Type (z. B. 204: Hash des leeren Körpers);
            // Fehlerseiten, Weiterleitungen, Bilder, CSS, JS, PDF und Text bleiben leer.
            let ok = (200..300).contains(&r.status);
            if cfg.extraction.hash
                && ok
                && (f.content_class == ContentClass::Html || f.content_type.is_none())
            {
                f.hash_md5 = Some(r.md5_hex.clone());
            }
        }
    }
    f
}

// ---------------------------------------------------------------------------------------------------
// Auswertung

#[derive(Default)]
struct Analysis {
    facts: FetchFacts,
    headers: Vec<HeaderRow>,
    parsed: Option<ParsedPage>,
    hints: Option<HeaderHints>,
    css_links: Vec<ExtractedLink>,
    pdf: Option<PdfMeta>,
    redirect: Option<RedirectTarget>,
    refresh: Vec<os_model::RefreshRef>,
}

async fn analyze<F: Fetch>(
    sh: &Arc<Shared<F>>,
    item: &QueueItem,
    url: &Url,
    result: &FetchResult,
) -> Analysis {
    let cfg = &*sh.cfg;
    let mut a = Analysis {
        facts: facts_from_result(result, cfg),
        headers: header_rows(result, cfg),
        ..Analysis::default()
    };
    let FetchOutcome::Response(resp) = &result.outcome else {
        return a;
    };
    // 3xx: nur Location auswerten, der Body wird nie geparst (Q5)
    if redirect::is_redirect_status(resp.status) {
        a.redirect = redirect::http_target(url, &resp.headers);
        return a;
    }
    // defekte Kompression liefert keinen verlässlichen Inhalt
    if resp.decode_error.is_some() {
        return a;
    }
    // Externe URLs werden nicht ausgewertet, mit einer Ausnahme (ONPAGE-058/059): Refresh-Header und
    // Meta-Refresh externer HTML-Seiten. Daraus entstehen Redirect URL/Type wie bei internen Seiten; ob das
    // Ziel abgerufen wird, entscheidet `assemble` nach `decide` (K-40) und `spider.external.crawl`.
    if item.purpose == Purpose::External {
        if a.facts.content_class == ContentClass::Html {
            let refs = external_refresh(sh, url, resp).await;
            a.refresh = refs.clone();
            parsed_refresh(&mut a, url, &refs);
        }
        return a;
    }
    match a.facts.content_class {
        ContentClass::Html => {
            let parsed = parse_html_page(sh, url, resp).await;
            a.facts.charset = parsed.charset.clone();
            a.refresh = parsed.refresh.clone();
            parsed_refresh(&mut a, url, &parsed.refresh);
            a.parsed = Some(parsed);
        }
        other => {
            let hints = os_parse::parse_headers(url, &resp.headers, &sh.pcfg);
            a.refresh = hints.refresh.clone();
            parsed_refresh(&mut a, url, &hints.refresh);
            match other {
                ContentClass::Css => a.css_links = os_parse::parse_css(url, &resp.body),
                ContentClass::Pdf => a.pdf = Some(os_parse::parse_pdf_meta(&resp.body)),
                _ => {}
            }
            a.hints = Some(hints);
        }
    }
    a
}

fn parsed_refresh(a: &mut Analysis, url: &Url, refs: &[os_model::RefreshRef]) {
    let outcome = redirect::refresh_target(url, refs);
    a.facts.meta_refresh_delay = outcome.delay;
    a.redirect = outcome.redirect;
}

/// Refresh-Angaben einer externen HTML-Seite: der `Refresh`-Header vor den Meta-Tags (Quelltextreihenfolge).
/// Der Rest der Seite wird nicht ausgewertet.
async fn external_refresh<F: Fetch>(
    sh: &Arc<Shared<F>>,
    url: &Url,
    resp: &HttpResponse,
) -> Vec<os_model::RefreshRef> {
    let mut refs = os_parse::parse_headers(url, &resp.headers, &sh.pcfg).refresh;
    let content_type = header(&resp.headers, "content-type").map(str::to_string);
    let found = if resp.body.len() < BLOCKING_PARSE_THRESHOLD {
        meta_refresh(url, &resp.body, content_type.as_deref(), &resp.headers)
    } else {
        let url = url.clone();
        let body: Bytes = resp.body.clone();
        let headers = resp.headers.clone();
        tokio::task::spawn_blocking(move || {
            meta_refresh(&url, &body, content_type.as_deref(), &headers)
        })
        .await
        .unwrap_or_default()
    };
    let shift = refs.len() as u32;
    refs.extend(found.into_iter().map(|mut r| {
        r.order = r.order.saturating_add(shift);
        r
    }));
    refs
}

fn meta_refresh(
    url: &Url,
    body: &[u8],
    content_type: Option<&str>,
    headers: &[(String, String)],
) -> Vec<os_model::RefreshRef> {
    let (html, _) = os_parse::charset::decode(body, content_type, headers);
    let doc = os_parse::dom::Document::parse(&html);
    let base = os_parse::base::resolve_base(&doc, url);
    os_parse::refresh::extract(&doc, &base)
}

async fn parse_html_page<F: Fetch>(
    sh: &Arc<Shared<F>>,
    url: &Url,
    resp: &HttpResponse,
) -> ParsedPage {
    let content_type = header(&resp.headers, "content-type").map(str::to_string);
    if resp.body.len() < BLOCKING_PARSE_THRESHOLD {
        return parse_now(
            url,
            &resp.body,
            content_type.as_deref(),
            &resp.headers,
            &sh.pcfg,
        );
    }
    let url = url.clone();
    let body: Bytes = resp.body.clone();
    let headers = resp.headers.clone();
    let pcfg = sh.pcfg.clone();
    tokio::task::spawn_blocking(move || {
        parse_now(&url, &body, content_type.as_deref(), &headers, &pcfg)
    })
    .await
    .unwrap_or_default()
}

fn parse_now(
    url: &Url,
    body: &[u8],
    content_type: Option<&str>,
    headers: &[(String, String)],
    cfg: &ParseConfig,
) -> ParsedPage {
    os_parse::parse_html(&ParseInput {
        url,
        body,
        content_type,
        headers,
        cfg,
    })
}

// ---------------------------------------------------------------------------------------------------
// Ziele

/// Eine Referenz auf ein Ziel (Link, Bild, Canonical, hreflang, Paginierung).
struct Ref {
    ext: ExtractedLink,
    /// Zeile in `links` anlegen (Links aus dem Dokument und synthetische für Canonical, hreflang …).
    row: bool,
    norm: Option<usize>,
    decision: LinkDecision,
}

struct Targets {
    list: Vec<(NormUrl, bool)>,
    index: HashMap<String, usize>,
}

impl Targets {
    fn new() -> Self {
        Targets {
            list: Vec::new(),
            index: HashMap::new(),
        }
    }

    fn get<F: Fetch>(&mut self, sh: &Shared<F>, resolved: &str) -> Option<usize> {
        if let Some(i) = self.index.get(resolved) {
            return Some(*i);
        }
        let url = Url::parse(resolved).ok()?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return None;
        }
        let norm = os_url::normalize(&url);
        let internal = sh.scope.classify(&norm) == ScopeClass::Internal;
        self.list.push((norm, internal));
        let i = self.list.len() - 1;
        self.index.insert(resolved.to_string(), i);
        Some(i)
    }
}

fn path_type_of(raw: &str) -> PathType {
    let t = raw.trim();
    if t.starts_with("//") {
        PathType::ProtocolRelative
    } else if t.contains("://") {
        PathType::Absolute
    } else if t.starts_with('/') {
        PathType::RootRelative
    } else {
        PathType::PathRelative
    }
}

fn synth_link(link_type: LinkType, raw: &str, resolved: Option<&str>) -> ExtractedLink {
    ExtractedLink {
        link_type,
        href_raw: raw.to_string(),
        resolved: resolved.map(str::to_string),
        anchor: None,
        alt_text: None,
        rel: None,
        nofollow_rel: false,
        target: None,
        path_type: path_type_of(raw),
        xpath: String::new(),
        position: LinkPosition::Content,
        in_head: true,
        doc_order: 0,
    }
}

/// Maßgebliche Canonical-URL (TECH-017): erster gültiger HTTP-Header-Eintrag, sonst erstes gültiges
/// HTML-Element im Head, sonst erstes gültiges HTML-Element außerhalb des Heads.
pub(crate) fn pick_canonical(canonicals: &[os_model::CanonicalRef]) -> Option<usize> {
    let valid = |c: &os_model::CanonicalRef| c.resolved.is_some();
    canonicals
        .iter()
        .position(|c| c.source == HintSource::Http && valid(c))
        .or_else(|| {
            canonicals
                .iter()
                .position(|c| c.source == HintSource::Html && c.in_head && valid(c))
        })
        .or_else(|| {
            canonicals
                .iter()
                .position(|c| c.source == HintSource::Html && !c.in_head && valid(c))
        })
}

/// Wo in der Referenzliste welche Art von Verweis steht.
#[derive(Default)]
struct RefMap {
    links: Vec<usize>,
    canonicals: Vec<usize>,
    hreflang: Vec<usize>,
    pagination: Vec<usize>,
}

fn element_pair(texts: &[ExtractedText], enabled: bool) -> ElementPair {
    if !enabled {
        return ElementPair::default();
    }
    ElementPair {
        count: texts.len() as u32,
        first: texts.first().cloned(),
        second: texts.get(1).cloned(),
    }
}

fn pdf_pair(text: Option<&str>) -> ElementPair {
    match text.map(str::trim).filter(|t| !t.is_empty()) {
        Some(t) => ElementPair {
            count: 1,
            first: Some(ExtractedText {
                text: t.to_string(),
                len: t.chars().count() as u32,
                px: None,
                in_head: false,
            }),
            second: None,
        },
        None => ElementPair::default(),
    }
}

/// Entdeckt die Ziele und bildet das Schreibmodell der Seite.
async fn assemble<F: Fetch>(
    sh: &Arc<Shared<F>>,
    item: &QueueItem,
    a: Analysis,
) -> Result<WriteBatch, Failure> {
    let cfg = &*sh.cfg;
    let Analysis {
        mut facts,
        headers,
        parsed,
        hints,
        css_links,
        pdf,
        redirect,
        refresh,
    } = a;
    let page_follow = parsed.as_ref().is_none_or(|p| p.effective_follow);

    let mut targets = Targets::new();
    let mut refs: Vec<Ref> = Vec::new();
    let mut map = RefMap::default();
    let add_ref = |refs: &mut Vec<Ref>, targets: &mut Targets, ext: ExtractedLink, row: bool| {
        let norm = ext.resolved.as_deref().and_then(|r| targets.get(&**sh, r));
        let scope = match norm.map(|i| targets.list[i].1) {
            Some(true) => ScopeClass::Internal,
            _ => ScopeClass::External,
        };
        let decision = decide::decide(&ext, scope, cfg, page_follow);
        refs.push(Ref {
            ext,
            row,
            norm,
            decision,
        });
        refs.len() - 1
    };

    // Links aus dem Dokument (HTML) bzw. aus der CSS-Datei
    if let Some(p) = &parsed {
        for l in &p.links {
            let i = add_ref(&mut refs, &mut targets, l.clone(), true);
            map.links.push(i);
        }
    }
    for l in &css_links {
        let i = add_ref(&mut refs, &mut targets, l.clone(), true);
        map.links.push(i);
    }
    // Canonical, hreflang, Paginierung (Dokument und Header)
    let (canonicals, hreflang, pagination, directives) = match (&parsed, &hints) {
        (Some(p), _) => (
            p.canonicals.clone(),
            p.hreflang.clone(),
            p.pagination.clone(),
            p.directives.clone(),
        ),
        (None, Some(h)) => (
            h.canonicals.clone(),
            h.hreflang.clone(),
            h.pagination.clone(),
            h.directives.clone(),
        ),
        (None, None) => Default::default(),
    };
    for c in &canonicals {
        let lt = match c.source {
            HintSource::Http => LinkType::HttpCanonical,
            _ => LinkType::Canonical,
        };
        let ext = synth_link(lt, &c.raw, c.resolved.as_deref());
        let i = add_ref(&mut refs, &mut targets, ext, true);
        map.canonicals.push(i);
    }
    for h in &hreflang {
        let lt = match h.source {
            HintSource::Http => LinkType::HttpHreflang,
            _ => LinkType::Hreflang,
        };
        let ext = synth_link(lt, &h.href_raw, h.resolved.as_deref());
        let i = add_ref(&mut refs, &mut targets, ext, true);
        map.hreflang.push(i);
    }
    for p in &pagination {
        let lt = if p.rel_next {
            LinkType::RelNext
        } else {
            LinkType::RelPrev
        };
        let ext = synth_link(lt, &p.raw, p.resolved.as_deref());
        let i = add_ref(&mut refs, &mut targets, ext, true);
        map.pagination.push(i);
    }
    // Bildinstanzen: nur nachschlagen (die Bild-Links stehen schon in `links`)
    let image_targets: Vec<Option<usize>> = parsed
        .as_ref()
        .map(|p| {
            p.images
                .iter()
                .map(|im| im.resolved.as_deref().and_then(|r| targets.get(&**sh, r)))
                .collect()
        })
        .unwrap_or_default();

    // Weiterleitung: Entscheidung und Limit (CRAWL-072)
    let redirect_plan = redirect.as_ref().map(|r| {
        let internal = sh.scope.classify(&r.target) == ScopeClass::Internal;
        targets.list.push((r.target.clone(), internal));
        (r, targets.list.len() - 1)
    });

    // Discovery unter der Sperre, Nachricht mit den neuen URLs vor der Seite in den Kanal
    let depth = item.depth.map(|d| d + 1);
    let t = std::time::Instant::now();
    let slot = sh.writer.reserve().await?;
    diag::add_max(&diag::STATS.reserve_ns, &diag::STATS.reserve_max_ns, t);
    let mut disc = WriteBatch::default();
    let mut ids: Vec<Option<UrlId>> = Vec::with_capacity(refs.len());
    let mut image_ids: Vec<Option<UrlId>> = Vec::new();
    let mut redirect_id: Option<UrlId> = None;
    // Weiterleitungs-Flags (CRAWL-017, CRAWL-072) gehen mit der Seite in den Writer (M1.md 3.11).
    let mut url_flags: Vec<(UrlId, String, Option<String>)> = Vec::new();
    {
        let mut core = sh.lock_core();
        for r in &refs {
            let d = &r.decision;
            let Some(ti) = r.norm else {
                ids.push(None);
                continue;
            };
            if !d.store && !d.crawl {
                ids.push(None);
                continue;
            }
            let (norm, internal) = &targets.list[ti];
            let lt = r.ext.link_type;
            let resource_like = lt.is_resource() || lt == LinkType::Other;
            let purpose = if !*internal {
                Purpose::External
            } else if resource_like {
                Purpose::Resource
            } else {
                Purpose::Page
            };
            let cand = Cand {
                norm,
                internal: *internal,
                kind: DiscoveryKind::Link,
                store: d.store,
                crawl: d.crawl,
                reason: d.reason,
                resource_like,
                apply_rules: norm.encoded != sh.start_url,
                purpose,
            };
            let id = core.discover(&cand, depth, 0, item.session, &sh.rules, &mut disc);
            ids.push(Some(id));
        }
        for t in &image_targets {
            let id = t
                .and_then(|ti| core.lookup(&targets.list[ti].0.encoded))
                .map(|s| s.id);
            image_ids.push(id);
        }
        if let Some((r, ti)) = &redirect_plan {
            let (norm, internal) = &targets.list[*ti];
            let (store, mut crawl, mut reason) = match r.kind {
                RedirectType::Http => (true, true, None),
                kind => {
                    // Meta-/Header-Refresh hängen am Schalter „Meta Refresh“ (K-40); Grund und Außenwirkung
                    // wie bei jedem anderen Linktyp aus der Matrix.
                    let link_type = if kind == RedirectType::MetaRefresh {
                        LinkType::MetaRefresh
                    } else {
                        LinkType::HttpRefresh
                    };
                    let ext = synth_link(link_type, &r.raw, Some(&norm.encoded));
                    let class = if *internal {
                        ScopeClass::Internal
                    } else {
                        ScopeClass::External
                    };
                    let d = decide::decide(&ext, class, cfg, true);
                    (d.store, d.crawl, d.reason)
                }
            };
            let internal = *internal;
            if !internal && crawl && !cfg.spider.external.crawl {
                crawl = false;
                reason = Some(NotCrawledReason::ExternalNotChecked);
            }
            let limit_hit = item.hops >= cfg.limits.max_redirects;
            if limit_hit {
                crawl = false;
                reason = Some(NotCrawledReason::RedirectLimit);
                disc.counters.push((COUNTER_REDIRECT_LIMIT.to_string(), 1));
                core.redirects.flag_limit(item.id);
                url_flags.push((item.id, redirect::FLAG_LIMIT.to_string(), None));
            }
            if store || crawl || limit_hit {
                let cand = Cand {
                    norm,
                    internal,
                    kind: DiscoveryKind::Redirect,
                    store: store && !limit_hit,
                    crawl,
                    reason,
                    resource_like: false,
                    apply_rules: norm.encoded != sh.start_url,
                    purpose: if internal {
                        Purpose::Page
                    } else {
                        Purpose::External
                    },
                };
                let id = core.discover(
                    &cand,
                    depth,
                    item.hops + 1,
                    item.session,
                    &sh.rules,
                    &mut disc,
                );
                if let Some(members) = core.redirects.add_edge(item.id, id) {
                    url_flags.extend(
                        members
                            .into_iter()
                            .map(|m| (m, redirect::FLAG_LOOP.to_string(), None)),
                    );
                }
                redirect_id = Some(id);
            }
        }
    }
    slot.send(disc);
    sh.ctl.notify.notify_one();

    // Schreibmodell
    if let Some((r, ti)) = &redirect_plan {
        facts.redirect_url = Some(targets.list[*ti].0.encoded.clone());
        facts.redirect_type = Some(r.kind);
        facts.redirect_target = redirect_id;
    }
    let mut batch = WriteBatch::default();
    let own = item.url.as_ref();
    let mut links: Vec<LinkRow> = Vec::new();
    let mut position = 0u32;
    let mut push_row = |links: &mut Vec<LinkRow>, r: &Ref, id: Option<UrlId>, follow: bool| {
        let e = &r.ext;
        links.push(LinkRow {
            target_id: id,
            // K-37: der Rohwert wie im Markup; das aufgelöste Ziel steht über `target_id`.
            target_raw: e.href_raw.clone(),
            link_type: e.link_type,
            position,
            anchor: e.anchor.clone(),
            alt_text: e.alt_text.clone(),
            follow,
            rel: e.rel.clone(),
            target_attr: e.target.clone(),
            path_type: Some(e.path_type),
            link_path: (!e.xpath.is_empty()).then(|| e.xpath.clone()),
            link_position: (!e.xpath.is_empty()).then_some(e.position),
            link_origin: LinkOrigin::Html,
            in_head: Some(e.in_head),
        });
        position += 1;
    };
    for &i in &map.links {
        let r = &refs[i];
        if r.row && r.decision.store {
            push_row(&mut links, r, ids[i], page_follow && !r.ext.nofollow_rel);
        }
    }
    for &i in map
        .canonicals
        .iter()
        .chain(&map.hreflang)
        .chain(&map.pagination)
    {
        let r = &refs[i];
        if r.row && r.decision.store {
            push_row(&mut links, r, ids[i], true);
        }
    }
    if let (Some((r, _)), Some(id)) = (&redirect_plan, redirect_id) {
        let lt = match r.kind {
            RedirectType::Http => LinkType::HttpRedirect,
            RedirectType::MetaRefresh => LinkType::MetaRefresh,
            RedirectType::HttpRefresh => LinkType::HttpRefresh,
            RedirectType::HstsPolicy => LinkType::HstsPolicy,
            RedirectType::JavaScript => LinkType::JavaScriptRedirect,
        };
        links.push(LinkRow {
            target_id: Some(id),
            target_raw: r.raw.clone(),
            link_type: lt,
            position,
            anchor: None,
            alt_text: None,
            follow: true,
            rel: None,
            target_attr: None,
            path_type: Some(path_type_of(&r.raw)),
            link_path: None,
            link_position: None,
            link_origin: LinkOrigin::Html,
            in_head: None,
        });
    }

    let images: Vec<ImageRow> = parsed
        .as_ref()
        .map(|p| {
            p.images
                .iter()
                .zip(&image_ids)
                .map(|(im, id)| ImageRow {
                    image_id: *id,
                    instance: im.clone(),
                })
                .collect()
        })
        .unwrap_or_default();

    // „Speichern“ aus (CRAWL-038, CRAWL-046, K-40): keine Detailzeilen und keine Spaltenwerte. Das Crawlen der
    // Ziele ist davon unabhängig und hat oben schon stattgefunden.
    let keep_canonicals = cfg.spider.canonicals.store;
    let keep_hreflang = cfg.spider.hreflang.store;
    let keep_pagination = cfg.spider.pagination.store;
    let canon_rows: Vec<CanonicalRow> = if keep_canonicals {
        canonicals
            .iter()
            .enumerate()
            .map(|(n, c)| CanonicalRow {
                target_id: ids.get(map.canonicals[n]).copied().flatten(),
                r: c.clone(),
            })
            .collect()
    } else {
        Vec::new()
    };
    let hreflang_rows: Vec<HreflangRow> = if keep_hreflang {
        hreflang
            .iter()
            .enumerate()
            .map(|(n, h)| HreflangRow {
                target_id: ids.get(map.hreflang[n]).copied().flatten(),
                sitemap_id: None,
                r: h.clone(),
            })
            .collect()
    } else {
        Vec::new()
    };
    let pagination_rows: Vec<PaginationRow> = if keep_pagination {
        pagination
            .iter()
            .enumerate()
            .map(|(n, p)| PaginationRow {
                target_id: ids.get(map.pagination[n]).copied().flatten(),
                r: p.clone(),
            })
            .collect()
    } else {
        Vec::new()
    };
    let refresh_rows: Vec<RefreshRow> = refresh
        .iter()
        .map(|r| {
            let chosen = redirect_plan
                .as_ref()
                .is_some_and(|(t, _)| r.target_raw.as_deref() == Some(t.raw.as_str()))
                && matches!(
                    (r.source, redirect_plan.as_ref().map(|(t, _)| t.kind)),
                    (RefreshSource::Meta, Some(RedirectType::MetaRefresh))
                        | (RefreshSource::Header, Some(RedirectType::HttpRefresh))
                );
            RefreshRow {
                target_id: if chosen { redirect_id } else { None },
                r: r.clone(),
            }
        })
        .collect();

    // Seitenfakten
    let canon_pick = pick_canonical(&canonicals).filter(|_| keep_canonicals);
    let canonical_url = canon_pick.and_then(|i| canonicals[i].resolved.clone());
    let canonical_target = canon_pick.and_then(|i| canon_rows[i].target_id);
    let canonical_count = if keep_canonicals {
        canonicals.len() as u32
    } else {
        0
    };
    let has_rel_next = keep_pagination && pagination.iter().any(|x| x.rel_next);
    let has_rel_prev = keep_pagination && pagination.iter().any(|x| !x.rel_next);
    let hreflang_count = if keep_hreflang {
        hreflang.len() as u32
    } else {
        0
    };
    let canonical_self = canonical_url
        .as_deref()
        .map(|c| os_url::eq_for_compare(own, c));
    let (eff_index, eff_follow) = match &parsed {
        Some(p) => (p.effective_index, p.effective_follow),
        None => {
            let hdr: Vec<_> = directives
                .iter()
                .filter(|d| d.source == os_model::DirectiveSource::XRobotsTag)
                .cloned()
                .collect();
            os_parse::effective::compute(&[], &hdr)
        }
    };
    let has_hints = !(canonicals.is_empty()
        && directives.is_empty()
        && refresh.is_empty()
        && hreflang.is_empty()
        && pagination.is_empty());
    let page_facts = match (&parsed, &pdf) {
        (Some(p), _) => Some(PageFacts {
            title: element_pair(&p.titles, cfg.extraction.page_titles),
            meta_description: element_pair(&p.meta_descriptions, cfg.extraction.meta_description),
            meta_keywords: element_pair(&p.meta_keywords, cfg.extraction.meta_keywords),
            h1: element_pair(&p.h1, cfg.extraction.h1),
            h2: element_pair(&p.h2, cfg.extraction.h2),
            word_count: p.word_count,
            text_ratio: p.text_ratio,
            canonical_count,
            canonical_url,
            canonical_target,
            canonical_self,
            effective_index: eff_index,
            effective_follow: eff_follow,
            has_rel_next,
            has_rel_prev,
            hreflang_count,
        }),
        (None, pdf_meta) if pdf_meta.is_some() || (hints.is_some() && has_hints) => {
            let (title, keywords) = match pdf_meta {
                Some(m) => (
                    pdf_pair(m.title.as_deref()),
                    pdf_pair(m.keywords.as_deref()),
                ),
                None => (ElementPair::default(), ElementPair::default()),
            };
            Some(PageFacts {
                title,
                meta_description: ElementPair::default(),
                meta_keywords: keywords,
                h1: ElementPair::default(),
                h2: ElementPair::default(),
                word_count: None,
                text_ratio: None,
                canonical_count,
                canonical_url,
                canonical_target,
                canonical_self,
                effective_index: eff_index,
                effective_follow: eff_follow,
                has_rel_next,
                has_rel_prev,
                hreflang_count,
            })
        }
        _ => None,
    };
    let (indexability, indexability_status) = os_model::evaluate(&IndexabilityInput {
        fetch: &facts,
        page: page_facts.as_ref(),
        page_url: own,
        refresh: &refresh,
        cfg,
        same_url: os_url::eq_for_compare,
    });
    batch.pages.push(PageWrite {
        url_id: item.id,
        replace: item.replace,
        fetch: facts,
        page: page_facts,
        indexability,
        indexability_status,
        links,
        images,
        canonicals: canon_rows,
        directives: directives.into_iter().map(DirectiveRow).collect(),
        refresh: refresh_rows,
        hreflang: hreflang_rows,
        pagination: pagination_rows,
        headers,
    });
    batch.queue.push(QueueOp::Pop { id: item.id });
    batch.url_flags = url_flags;
    Ok(batch)
}

// ---------------------------------------------------------------------------------------------------
// Sitemaps

/// Ruft Sitemaps ab und überträgt ihre URLs in die Warteschlange.
///
/// K-41a: Sitemap-Dateien holt ausschließlich dieser Weg, und zwar je Crawl genau einmal. Die Schlüssel der
/// angefragten und der End-URL stehen in `Core::sitemap_keys`; Seeds, die dort schon stehen (z. B. dieselbe
/// `Sitemap:`-Zeile in der robots.txt eines zweiten Hosts), entfallen.
pub(crate) async fn ingest_sitemaps<F: Fetch>(
    sh: &Arc<Shared<F>>,
    seeds: Vec<SitemapSeed>,
) -> Result<(), Failure> {
    if seeds.is_empty() {
        return Ok(());
    }
    let (fresh, known) = {
        let mut core = sh.lock_core();
        let known = core.sitemap_keys.clone();
        let mut fresh = Vec::new();
        for seed in seeds {
            if core.sitemap_keys.insert(sitemaps::sitemap_key(&seed.url)) {
                fresh.push(seed);
            }
        }
        (fresh, known)
    };
    if fresh.is_empty() {
        return Ok(());
    }
    let out = sitemaps::ingest_with_known(&*sh.fetcher, fresh, 3, &known).await;
    let fetched = out.sitemaps;
    if fetched.is_empty() && out.warnings.is_empty() {
        return Ok(());
    }
    let events: Vec<os_model::CrawlEvent> = out
        .warnings
        .iter()
        .map(|w| os_model::CrawlEvent {
            at: sh.clock.now(),
            kind: "warn".to_string(),
            code: w.code.to_string(),
            message: w.message.clone(),
        })
        .collect();
    {
        let mut core = sh.lock_core();
        for fs in &fetched {
            if let Ok(u) = Url::parse(&fs.url) {
                core.sitemap_keys.insert(sitemaps::sitemap_key(&u));
            }
            core.sitemap_keys
                .insert(sitemaps::sitemap_key(&fs.fetch.url));
        }
    }
    let slot = sh.writer.reserve().await?;
    let batch = {
        let mut core = sh.lock_core();
        let mut batch = build_sitemap_batch(
            &mut core,
            &sh.cfg,
            &sh.scope,
            &sh.rules,
            &fetched,
            sh.clock.now(),
        );
        batch.events.extend(events);
        batch
    };
    slot.send(batch);
    sh.ctl.notify.notify_one();
    Ok(())
}

/// Bildet das Schreibmodell der abgerufenen Sitemaps: URL-Zeile der Datei, `SitemapWrite` und die URLs der
/// Einträge als Warteschlangeneinträge ohne Tiefe (Priorität 1, TECH-210/CRAWL-005).
///
/// K-41d (Sitemap-Protokoll): Ein `<loc>` gilt nur, wenn Schema, Host und Port mit der Sitemap-Datei
/// übereinstimmen. Andere Einträge zählen wie ungültige (`invalid_entries`), bekommen keine URL-Zeile und
/// keinen Warteschlangeneintrag; je Datei meldet ein `CrawlEvent` (warn) ihre Anzahl. Cross-Submission per
/// robots.txt ist M2.
pub(crate) fn build_sitemap_batch(
    core: &mut Core,
    cfg: &CrawlConfig,
    scope: &impl Classify,
    rules: &os_url::RuleSet,
    fetched: &[FetchedSitemap],
    now: Timestamp,
) -> WriteBatch {
    let mut batch = WriteBatch::default();
    let base = core.next_sitemap_id;
    core.next_sitemap_id += fetched.len() as i64;
    for (i, fs) in fetched.iter().enumerate() {
        let sid = SitemapId(base + i as i64);
        let parent = fs.parent.map(|p| SitemapId(base + p as i64));
        let mut url_id = None;
        let mut status_code = None;
        if let Ok(parsed) = Url::parse(&fs.url) {
            let norm = os_url::normalize(&parsed);
            let internal = scope.classify(&norm) == ScopeClass::Internal;
            match core.lookup(&norm.encoded) {
                Some(seen) => url_id = Some(seen.id),
                None => {
                    let id = core.alloc_id();
                    core.register(&norm.encoded, id, None, F_QUEUED | F_RULE_OK);
                    // Die Start-URL einer Sitemap-Datei ist Tiefe 0; alle anderen Dateien haben keine Tiefe.
                    let is_start = fs.found_via == "start" && fs.parent.is_none();
                    batch.new_urls.push(os_model::NewUrl {
                        id,
                        url: norm.encoded.clone(),
                        address: norm.display.clone(),
                        origin: norm.origin.clone(),
                        is_internal: internal,
                        in_tables: false,
                        discovered_via: if is_start {
                            DiscoveryKind::Start
                        } else {
                            DiscoveryKind::Sitemap
                        },
                        crawl_depth: is_start.then_some(0),
                        folder_depth: os_url::folder_depth(&norm.url),
                        url_length: os_url::url_length(&norm.display),
                        state: UrlState::Done,
                        not_crawled_reason: None,
                    });
                    let mut page = plain_page(id, &fs.fetch, cfg);
                    page.headers = header_rows(&fs.fetch, cfg);
                    batch.pages.push(page);
                    url_id = Some(id);
                }
            }
        }
        if let FetchOutcome::Response(r) = &fs.fetch.outcome {
            status_code = Some(r.status);
        }
        let sitemap_origins: Vec<Url> = [Url::parse(&fs.url).ok(), Some(fs.fetch.url.clone())]
            .into_iter()
            .flatten()
            .collect();
        let (kind, entries, invalid, size) = match &fs.doc {
            Some(d) => (d.kind, d.locs.len() as u32, d.invalid_entries, d.size_bytes),
            None => (SitemapKind::Invalid, 0, 0, 0),
        };
        let mut entry_urls = Vec::new();
        let mut hreflang_rows: Vec<(UrlId, HreflangRow)> = Vec::new();
        // Spider-Modus: Einträge ohne Tiefe (Priorität 1, nur über die Sitemap erreicht). Listenmodus: die
        // Einträge sind Listen-URLs der Tiefe 0 (CRAWL-027).
        let (entry_kind, entry_depth) = if core.list_mode {
            (DiscoveryKind::ListInput, Some(0))
        } else {
            (DiscoveryKind::Sitemap, None)
        };
        let mut foreign = 0u32;
        if let (Some(doc), SitemapKind::Urlset) = (&fs.doc, kind) {
            for loc in &doc.locs {
                let Some(u) = loc.url.as_deref() else {
                    continue;
                };
                let Ok(parsed) = Url::parse(u) else {
                    continue;
                };
                if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
                    continue;
                }
                if !same_origin(&parsed, &sitemap_origins) {
                    foreign += 1;
                    continue;
                }
                let norm = os_url::normalize(&parsed);
                if scope.classify(&norm) != ScopeClass::Internal {
                    continue;
                }
                let cand = Cand {
                    norm: &norm,
                    internal: true,
                    kind: entry_kind,
                    store: true,
                    crawl: true,
                    reason: None,
                    resource_like: false,
                    apply_rules: true,
                    purpose: Purpose::Page,
                };
                let id = core.discover(&cand, entry_depth, 0, 0, rules, &mut batch);
                entry_urls.push((id, loc.loc_raw.clone(), loc.lastmod.clone()));
                // Auch bei `store = false, crawl = true` durchlaufen: die Ziele laufen über `decide` und
                // `discover`, nur die hreflang-Zeilen entfallen (CRAWL-038).
                if cfg.spider.hreflang.store || cfg.spider.hreflang.crawl {
                    sitemap_hreflang(
                        core,
                        cfg,
                        scope,
                        rules,
                        &mut batch,
                        sid,
                        id,
                        &loc.hreflang,
                        &mut hreflang_rows,
                    );
                }
            }
        }
        if foreign > 0 {
            batch.events.push(os_model::CrawlEvent {
                at: now,
                kind: "warn".to_string(),
                code: "sitemap_foreign_entries".to_string(),
                message: format!(
                    "{foreign} Einträge in {} verworfen: anderes Schema, anderer Host oder anderer Port als die Sitemap-Datei",
                    fs.url
                ),
            });
        }
        batch.sitemaps.push(SitemapWrite {
            id: sid,
            url: fs.url.clone(),
            url_id,
            parent,
            kind,
            entries,
            invalid_entries: invalid + foreign,
            size_bytes: size,
            status_code,
            depth: fs.depth,
            found_via: fs.found_via.clone(),
            entry_urls,
            hreflang: hreflang_rows,
        });
    }
    batch
}

/// Gleiches Schema, gleicher Host und gleicher Port wie eine der Adressen der Sitemap-Datei (K-41d).
fn same_origin(loc: &Url, files: &[Url]) -> bool {
    files.iter().any(|f| {
        f.scheme() == loc.scheme()
            && f.host_str() == loc.host_str()
            && f.port_or_known_default() == loc.port_or_known_default()
    })
}

/// hreflang-Angaben eines `<url>`-Blocks (TECH-210, K-41e): Ziele laufen über `decide` wie HTML-hreflang
/// (`LinkType::Hreflang`, `spider.hreflang`); `order` zählt je `loc`-URL fortlaufend über alle Sitemaps, damit
/// mehrfach gelistete URLs nichts überschreiben. `target_id` gilt, sobald eine Zielzeile existiert. Bei
/// `spider.hreflang.store = false` entstehen keine Zeilen, die Ziele werden trotzdem eingeplant.
#[allow(clippy::too_many_arguments)]
fn sitemap_hreflang(
    core: &mut Core,
    cfg: &CrawlConfig,
    scope: &impl Classify,
    rules: &os_url::RuleSet,
    batch: &mut WriteBatch,
    sitemap: SitemapId,
    loc_id: UrlId,
    refs: &[HreflangRef],
    out: &mut Vec<(UrlId, HreflangRow)>,
) {
    for h in refs {
        let ext = synth_link(LinkType::Hreflang, &h.href_raw, h.resolved.as_deref());
        let target = h
            .resolved
            .as_deref()
            .and_then(|r| Url::parse(r).ok())
            .filter(|u| matches!(u.scheme(), "http" | "https") && u.host_str().is_some())
            .map(|u| os_url::normalize(&u));
        let target_id = target.and_then(|norm| {
            let class = scope.classify(&norm);
            let internal = class == ScopeClass::Internal;
            let d = decide::decide(&ext, class, cfg, true);
            if !d.store && !d.crawl {
                return core.lookup(&norm.encoded).map(|s| s.id);
            }
            let cand = Cand {
                norm: &norm,
                internal,
                kind: DiscoveryKind::Sitemap,
                store: d.store,
                crawl: d.crawl,
                reason: d.reason,
                resource_like: false,
                apply_rules: true,
                purpose: if internal {
                    Purpose::Page
                } else {
                    Purpose::External
                },
            };
            // Listenmodus: Alternativen sind Links der Listenseite (Tiefe 1) und fallen unter das Tiefenlimit.
            let depth = core.list_mode.then_some(1);
            Some(core.discover(&cand, depth, 0, 0, rules, batch))
        });
        if !cfg.spider.hreflang.store {
            continue;
        }
        let next = core.sitemap_hreflang_next.entry(loc_id).or_insert(0);
        let mut r = h.clone();
        r.order = *next;
        r.source = HintSource::Sitemap;
        *next += 1;
        out.push((
            loc_id,
            HreflangRow {
                target_id,
                sitemap_id: Some(sitemap),
                r,
            },
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use os_model::{CanonicalRef, SitemapDoc, SitemapLoc};
    use os_url::Scope;

    fn response(status: u16, headers: &[(&str, &str)], body: &str) -> FetchResult {
        FetchResult {
            url: Url::parse("https://site.test/p").unwrap(),
            started_at: Timestamp::UNIX_EPOCH,
            outcome: FetchOutcome::Response(HttpResponse {
                status,
                reason_phrase: None,
                status_text: os_fetch::http::status_text(status, None),
                http_version: "HTTP/1.1".to_string(),
                headers: headers
                    .iter()
                    .map(|(n, v)| (n.to_string(), v.to_string()))
                    .collect(),
                body: Bytes::from(body.to_string()),
                size_bytes: body.len() as u64,
                transferred_bytes: 123,
                response_time_ms: 42,
                truncated: false,
                decode_error: None,
                md5_hex: "d41d8cd98f00b204e9800998ecf8427e".to_string(),
            }),
        }
    }

    // spec: CRAWL-022
    #[test]
    fn http_date_is_parsed_to_utc() {
        let t = parse_http_date("Wed, 21 Oct 2015 07:28:00 GMT").expect("gültig");
        assert_eq!(t.to_db(), "2015-10-21T07:28:00.000Z");
        assert!(parse_http_date("kein Datum").is_none());
    }

    // spec: ONPAGE-018
    #[test]
    fn hash_is_only_set_for_html_2xx_responses() {
        let cfg = CrawlConfig::default();
        let html = response(200, &[("content-type", "text/html")], "<html></html>");
        assert_eq!(
            facts_from_result(&html, &cfg)
                .hash_md5
                .as_deref()
                .map(str::len),
            Some(32)
        );
        // Fehlerseiten mit HTML-Körper, Bilder, CSS, Text und Weiterleitungen haben keinen Hash (K-60)
        let error = response(404, &[("content-type", "text/html")], "<html></html>");
        assert_eq!(facts_from_result(&error, &cfg).hash_md5, None);
        for ct in ["image/png", "text/css", "text/plain", "application/json"] {
            let res = response(200, &[("content-type", ct)], "abc");
            assert_eq!(facts_from_result(&res, &cfg).hash_md5, None, "{ct}");
        }
        let redirect = response(301, &[("location", "/x")], "");
        assert_eq!(facts_from_result(&redirect, &cfg).hash_md5, None);
        // 2xx ohne Content-Type (204): Hash des leeren Körpers wie in der Referenz
        let no_content = response(204, &[], "");
        assert_eq!(
            facts_from_result(&no_content, &cfg).hash_md5.as_deref(),
            Some("d41d8cd98f00b204e9800998ecf8427e")
        );
    }

    // spec: CRAWL-022
    #[test]
    fn facts_follow_the_response_and_k02() {
        let cfg = CrawlConfig::default();
        let res = response(
            200,
            &[
                ("content-type", "text/html; charset=utf-8"),
                ("last-modified", "Wed, 21 Oct 2015 07:28:00 GMT"),
            ],
            "<html></html>",
        );
        let f = facts_from_result(&res, &cfg);
        assert_eq!(f.status_code, 200);
        assert_eq!(f.status, "OK");
        assert_eq!(f.content_class, ContentClass::Html);
        assert_eq!(f.transferred_bytes, 123);
        assert_eq!(f.total_transferred_bytes, 123, "K-02");
        assert_eq!(f.response_time_ms, Some(42));
        assert_eq!(
            f.last_modified.map(|t| t.to_db()).as_deref(),
            Some("2015-10-21T07:28:00.000Z")
        );
        assert_eq!(f.hash_md5.as_deref().map(str::len), Some(32));

        let mut off = CrawlConfig::default();
        off.extraction.response_time = false;
        off.extraction.hash = false;
        off.extraction.last_modified = false;
        let f = facts_from_result(&res, &off);
        assert_eq!(
            (f.response_time_ms, f.hash_md5, f.last_modified_raw),
            (None, None, None)
        );
    }

    // spec: CRAWL-011
    #[test]
    fn broken_compression_keeps_the_status_code_but_reads_connection_error() {
        let cfg = CrawlConfig::default();
        let mut res = response(200, &[("content-type", "text/html")], "<html>");
        if let FetchOutcome::Response(r) = &mut res.outcome {
            r.decode_error = Some("unerwartetes Ende".into());
        }
        let f = facts_from_result(&res, &cfg);
        assert_eq!(f.status_code, 200);
        assert_eq!(f.status, "Connection Error");
        assert_eq!(
            f.no_response_reason,
            Some(NoResponseReason::ConnectionError)
        );
    }

    // spec: CRAWL-013
    #[test]
    fn no_response_gives_status_zero_and_the_reason_text() {
        let cfg = CrawlConfig::default();
        let res = FetchResult {
            url: Url::parse("https://site.test/").unwrap(),
            started_at: Timestamp::UNIX_EPOCH,
            outcome: FetchOutcome::NoResponse {
                reason: NoResponseReason::DnsLookupFailed,
                detail: String::new(),
                elapsed_ms: 5,
            },
        };
        let f = facts_from_result(&res, &cfg);
        assert_eq!(f.status_code, 0);
        assert_eq!(f.status, "DNS Lookup Failed");
        assert_eq!(f.response_time_ms, None);
    }

    // spec: OUT-031
    #[test]
    fn path_type_distinguishes_the_four_forms() {
        assert_eq!(path_type_of("https://a.test/x"), PathType::Absolute);
        assert_eq!(path_type_of("//a.test/x"), PathType::ProtocolRelative);
        assert_eq!(path_type_of("/x"), PathType::RootRelative);
        assert_eq!(path_type_of("x/y"), PathType::PathRelative);
    }

    fn canon(source: HintSource, in_head: bool, resolved: Option<&str>) -> CanonicalRef {
        CanonicalRef {
            source,
            raw: resolved.unwrap_or("").to_string(),
            resolved: resolved.map(str::to_string),
            in_head,
            order: 0,
        }
    }

    // spec: TECH-017
    #[test]
    fn authoritative_canonical_prefers_header_then_head_then_body() {
        let c = vec![
            canon(HintSource::Html, false, Some("https://s.test/body")),
            canon(HintSource::Html, true, Some("https://s.test/head")),
            canon(HintSource::Http, true, Some("https://s.test/header")),
        ];
        assert_eq!(pick_canonical(&c), Some(2));
        assert_eq!(pick_canonical(&c[..2]), Some(1));
        assert_eq!(pick_canonical(&c[..1]), Some(0));
        let invalid = vec![
            canon(HintSource::Http, true, None),
            canon(HintSource::Html, true, Some("https://s.test/ok")),
        ];
        assert_eq!(pick_canonical(&invalid), Some(1));
        assert_eq!(pick_canonical(&[]), None);
    }

    // spec: CRAWL-005, TECH-210
    #[test]
    fn sitemap_batch_queues_loc_urls_without_depth_and_links_the_hierarchy() {
        let cfg = CrawlConfig::default();
        let start = os_url::parse_start_url("https://site.test/").unwrap();
        let scope = Scope::new(&start, &cfg.scope);
        let rules = os_url::RuleSet::compile(&[], &[], false).unwrap();
        let mut core = Core::new(0, None);
        let doc = |kind, locs: Vec<&str>| SitemapDoc {
            kind,
            locs: locs
                .into_iter()
                .map(|l| SitemapLoc {
                    loc_raw: l.to_string(),
                    url: Some(l.to_string()),
                    lastmod: Some("2026-01-01".to_string()),
                    hreflang: vec![],
                })
                .collect(),
            invalid_entries: 1,
            size_bytes: 77,
        };
        let fr = |u: &str| FetchResult {
            url: Url::parse(u).unwrap(),
            started_at: Timestamp::UNIX_EPOCH,
            outcome: response(200, &[("content-type", "application/xml")], "<x/>").outcome,
        };
        let fetched = vec![
            FetchedSitemap {
                url: "https://site.test/sitemap.xml".into(),
                parent: None,
                depth: 0,
                found_via: "config".into(),
                fetch: fr("https://site.test/sitemap.xml"),
                doc: Some(doc(SitemapKind::Index, vec!["https://site.test/s1.xml"])),
            },
            FetchedSitemap {
                url: "https://site.test/s1.xml".into(),
                parent: Some(0),
                depth: 1,
                found_via: "config".into(),
                fetch: fr("https://site.test/s1.xml"),
                doc: Some(doc(
                    SitemapKind::Urlset,
                    vec!["https://site.test/nur-sitemap", "https://other.test/extern"],
                )),
            },
        ];
        let batch = build_sitemap_batch(
            &mut core,
            &cfg,
            &scope,
            &rules,
            &fetched,
            Timestamp::UNIX_EPOCH,
        );
        assert_eq!(batch.sitemaps.len(), 2);
        assert_eq!(batch.sitemaps[0].parent, None);
        assert_eq!(batch.sitemaps[1].parent, Some(batch.sitemaps[0].id));
        assert_eq!(batch.sitemaps[0].kind, SitemapKind::Index);
        assert_eq!(batch.sitemaps[0].entries, 1);
        // K-41d: der Eintrag auf other.test zählt zu den ungültigen (1 aus der Datei + 1 fremder)
        assert_eq!(batch.sitemaps[1].invalid_entries, 2);
        assert_eq!(batch.sitemaps[1].entries, 2);
        assert_eq!(batch.events.len(), 1);
        assert_eq!(batch.events[0].kind, "warn");
        assert_eq!(batch.events[0].code, "sitemap_foreign_entries");
        assert!(batch.events[0].message.starts_with("1 "));
        assert_eq!(batch.sitemaps[1].status_code, Some(200));
        assert_eq!(batch.sitemaps[1].found_via, "config");
        // nur der Eintrag desselben Ursprungs wird übernommen
        assert_eq!(batch.sitemaps[1].entry_urls.len(), 1);
        let (id, loc, lastmod) = &batch.sitemaps[1].entry_urls[0];
        assert_eq!(loc, "https://site.test/nur-sitemap");
        assert_eq!(lastmod.as_deref(), Some("2026-01-01"));
        let row = batch.new_urls.iter().find(|n| n.id == *id).unwrap();
        assert_eq!(row.crawl_depth, None, "ohne Tiefe");
        assert_eq!(row.discovered_via, DiscoveryKind::Sitemap);
        assert_eq!(row.state, UrlState::Queued);
        assert!(batch.queue.iter().any(
            |q| matches!(q, QueueOp::Push { id: i, priority: 1, depth: None, .. } if i == id)
        ));
        // die Sitemap-Dateien haben eine URL-Zeile mit Status
        assert!(batch.sitemaps.iter().all(|s| s.url_id.is_some()));
        assert_eq!(batch.pages.len(), 2);
        assert!(batch
            .new_urls
            .iter()
            .filter(|n| n.url.ends_with(".xml"))
            .all(|n| !n.in_tables));
    }

    fn alt(code: &str, href: &str) -> HreflangRef {
        HreflangRef {
            source: HintSource::Sitemap,
            code_raw: code.to_string(),
            code_norm: code.to_string(),
            code_valid: true,
            invalid_reason: None,
            href_raw: href.to_string(),
            resolved: Some(href.to_string()),
            in_head: false,
            order: 0,
        }
    }

    fn urlset_fetch(url: &str, locs: Vec<SitemapLoc>) -> FetchedSitemap {
        FetchedSitemap {
            url: url.to_string(),
            parent: None,
            depth: 0,
            found_via: "config".into(),
            fetch: FetchResult {
                url: Url::parse(url).unwrap(),
                started_at: Timestamp::UNIX_EPOCH,
                outcome: response(200, &[("content-type", "application/xml")], "<x/>").outcome,
            },
            doc: Some(SitemapDoc {
                kind: SitemapKind::Urlset,
                locs,
                invalid_entries: 0,
                size_bytes: 10,
            }),
        }
    }

    // spec: TECH-210
    #[test]
    fn sitemap_hreflang_is_numbered_per_url_across_sitemaps_and_targets_follow_decide() {
        let cfg = CrawlConfig::default();
        let start = os_url::parse_start_url("https://site.test/").unwrap();
        let scope = Scope::new(&start, &cfg.scope);
        let rules = os_url::RuleSet::compile(&[], &[], false).unwrap();
        let mut core = Core::new(0, None);
        let loc = |alts: Vec<HreflangRef>| SitemapLoc {
            loc_raw: "https://site.test/a".to_string(),
            url: Some("https://site.test/a".to_string()),
            lastmod: None,
            hreflang: alts,
        };
        let fetched = vec![
            urlset_fetch(
                "https://site.test/s1.xml",
                vec![loc(vec![
                    alt("de", "https://site.test/a"),
                    alt("fr", "https://site.test/fr/a"),
                ])],
            ),
            urlset_fetch(
                "https://site.test/s2.xml",
                vec![loc(vec![alt("en", "https://other.test/en/a")])],
            ),
        ];
        let batch = build_sitemap_batch(
            &mut core,
            &cfg,
            &scope,
            &rules,
            &fetched,
            Timestamp::UNIX_EPOCH,
        );
        let orders = |i: usize| -> Vec<(u32, String)> {
            batch.sitemaps[i]
                .hreflang
                .iter()
                .map(|(_, h)| (h.r.order, h.r.code_norm.clone()))
                .collect()
        };
        assert_eq!(orders(0), [(0, "de".to_string()), (1, "fr".to_string())]);
        assert_eq!(
            orders(1),
            [(2, "en".to_string())],
            "die Nummer läuft über alle Sitemaps weiter, nichts wird überschrieben"
        );
        let a_id = batch.sitemaps[0].entry_urls[0].0;
        assert!(batch
            .sitemaps
            .iter()
            .flat_map(|s| &s.hreflang)
            .all(|(id, h)| *id == a_id
                && h.r.source == HintSource::Sitemap
                && h.target_id.is_some()));
        assert_eq!(
            batch.sitemaps[0].hreflang[0].1.sitemap_id,
            Some(batch.sitemaps[0].id)
        );
        // Standard: hreflang speichern an, crawlen aus – interne und externe Ziele sind gespeichert, nicht eingeplant.
        let target = |url: &str| batch.new_urls.iter().find(|n| n.url == url).unwrap();
        let fr = target("https://site.test/fr/a");
        assert!(fr.in_tables);
        assert_eq!(fr.not_crawled_reason, Some(NotCrawledReason::StoreOnly));
        let en = target("https://other.test/en/a");
        assert!(!en.is_internal && en.in_tables);
        assert!(!batch
            .queue
            .iter()
            .any(|q| matches!(q, QueueOp::Push { id, .. } if *id == fr.id || *id == en.id)));

        // Speichern aus: keine Zeilen, keine Zielzeilen.
        let mut off = CrawlConfig::default();
        off.spider.hreflang = os_model::StoreCrawl::OFF;
        let mut core = Core::new(0, None);
        let batch = build_sitemap_batch(
            &mut core,
            &off,
            &scope,
            &rules,
            &fetched,
            Timestamp::UNIX_EPOCH,
        );
        assert!(batch.sitemaps.iter().all(|s| s.hreflang.is_empty()));
        assert!(batch.new_urls.iter().all(|n| !n.url.contains("/fr/")));
    }

    // spec: TECH-210
    #[test]
    fn sitemap_hreflang_with_store_off_and_crawl_on_queues_targets_but_writes_no_rows() {
        let cfg = {
            let mut c = CrawlConfig::default();
            c.spider.hreflang = os_model::StoreCrawl {
                store: false,
                crawl: true,
            };
            c
        };
        let start = os_url::parse_start_url("https://site.test/").unwrap();
        let scope = Scope::new(&start, &cfg.scope);
        let rules = os_url::RuleSet::compile(&[], &[], false).unwrap();
        let mut core = Core::new(0, None);
        let fetched = vec![urlset_fetch(
            "https://site.test/s1.xml",
            vec![SitemapLoc {
                loc_raw: "https://site.test/a".to_string(),
                url: Some("https://site.test/a".to_string()),
                lastmod: None,
                hreflang: vec![alt("fr", "https://site.test/fr/a")],
            }],
        )];
        let batch = build_sitemap_batch(
            &mut core,
            &cfg,
            &scope,
            &rules,
            &fetched,
            Timestamp::UNIX_EPOCH,
        );
        assert!(batch.sitemaps[0].hreflang.is_empty(), "keine Zeilen");
        let fr = batch
            .new_urls
            .iter()
            .find(|n| n.url == "https://site.test/fr/a")
            .expect("Ziel über decide eingeplant");
        assert!(!fr.in_tables, "nicht gespeichert");
        assert!(batch
            .queue
            .iter()
            .any(|q| matches!(q, QueueOp::Push { id, .. } if *id == fr.id)));
    }
}
