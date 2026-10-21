//! Wiederherstellung von Seen-Set, Zählern und Warteschlange beim Fortsetzen (CRAWL-125, CRAWL-126).
//!
//! Alles, was ein fortgesetzter Crawl braucht, steht in der Crawl-Datei:
//!
//! * **Seen-Set:** jede Zeile aus `urls` (Kennung, URL Encoded Address, kleinste Tiefe, Zustand). Eingeplante
//!   und abgerufene URLs tragen `F_QUEUED`; Zeilen im Zustand `discovered` tragen keine Regel-Markierung, ihre
//!   Include/Exclude-Entscheidung wird beim nächsten Fund neu getroffen (die Regeln dürfen sich in der Pause
//!   ändern).
//! * **Warteschlange:** Tabelle `queue` (Priorität, Tiefe, `seq`), verbunden mit `urls` für die Adresse. Eine große
//!   Warteschlange wird nicht ganz geladen: Nur das RAM-Fenster (die kleinsten Schlüssel,
//!   [`crate::queue::QUEUE_WINDOW`]) und die nicht auslagerbaren Einträge kommen ins RAM, der Rest bleibt in der
//!   Tabelle und wird beim Crawlen nachgeladen (K-78 b). Zweck (Seite/Ressource/extern) und Weiterleitungszähler
//!   stehen nicht in der Warteschlange; sie werden aus den eingehenden Links (Ressourcen-Linktypen, **ein**
//!   sequenzieller Lauf über `links`, ohne den Index `links_target`, K-74) bzw. den abgerufenen Weiterleitungen
//!   abgeleitet.
//! * **Zähler:** Kennungen und `seq` aus den Metadaten bzw. den größten Werten der Tabellen; „gecrawlt“ und
//!   „gestartet“ (URL-Limit) aus den abgerufenen URLs (ohne robots.txt und Sitemap-Dateien; von robots.txt
//!   gesperrte URLs zählen zu „gecrawlt“, aber nicht zu „gestartet“ – wie im laufenden Crawl).
//! * **Weiterleitungsgraph:** Kanten aus `urls.redirect_target_id`, Flags aus `url_flags`.
//! * **Sitemaps:** abgerufene Dateien (`sitemaps`), nächste Kennung, fortlaufende hreflang-Nummern.
//! * **robots.txt:** Die gespeicherten Dateien (`robots_files`) bedient [`PrimedFetch`] statt eines neuen
//!   Abrufs: Ein fortgesetzter Crawl ruft dieselbe URL nicht noch einmal ab („keine URL doppelt im
//!   Request-Log“).

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use bytes::Bytes;
use os_fetch::{
    CookieJar, Fetch, FetchOutcome, FetchPurpose, FetchRequest, FetchResult, HttpResponse,
};
use os_model::{CrawlConfig, CrawlMode, Timestamp, UrlId};
use os_store::{CrawlFile, StoreError};

use crate::error::EngineError;
use crate::frontier::{Core, F_IN_TABLES, F_QUEUED, F_RULE_OK};

/// Gespeicherte robots.txt eines Origins, mit der [`PrimedFetch`] den Abruf ersetzt.
#[derive(Debug, Clone)]
pub(crate) struct PrimedRobots {
    /// Origin (`scheme://host[:port]`).
    pub origin: String,
    /// Statuscode der Antwort.
    pub status: u16,
    /// Inhalt (leer bei „nicht vorhanden“).
    pub body: String,
}

/// `Fetch`, das die robots.txt bereits bekannter Origins aus der Crawl-Datei beantwortet und alles andere an
/// den eigentlichen Abrufer weiterreicht.
pub struct PrimedFetch<F: Fetch> {
    inner: Arc<F>,
    robots: HashMap<String, PrimedRobots>,
}

impl<F: Fetch> PrimedFetch<F> {
    pub(crate) fn new(inner: Arc<F>, primed: &[(String, PrimedRobots)]) -> Self {
        PrimedFetch {
            inner,
            robots: primed.iter().cloned().collect(),
        }
    }
}

impl<F: Fetch> Fetch for PrimedFetch<F> {
    fn render_cookie_jar(
        &self,
        session: Option<std::sync::Arc<os_fetch::CookieJar>>,
    ) -> Option<std::sync::Arc<os_fetch::CookieJar>> {
        self.inner.render_cookie_jar(session)
    }

    fn render_config(&self) -> Option<os_fetch::FetchConfig> {
        self.inner.render_config()
    }

    fn target_denial<'a>(
        &'a self,
        url: &'a url::Url,
    ) -> Pin<Box<dyn Future<Output = Option<FetchResult>> + Send + 'a>> {
        self.inner.target_denial(url)
    }

    fn fetch(&self, req: FetchRequest) -> Pin<Box<dyn Future<Output = FetchResult> + Send + '_>> {
        if req.purpose == FetchPurpose::RobotsTxt {
            if let Some(p) = self.robots.get(req.url.as_str()) {
                let status = p.status;
                let size = p.body.len() as u64;
                let body = Bytes::from(p.body.clone());
                return Box::pin(async move {
                    FetchResult {
                        url: req.url,
                        started_at: Timestamp::now_utc(),
                        outcome: FetchOutcome::Response(HttpResponse {
                            status,
                            reason_phrase: None,
                            status_text: os_fetch::http::status_text(status, None),
                            http_version: "HTTP/1.1".to_string(),
                            headers: vec![(
                                "content-type".to_string(),
                                "text/plain; charset=utf-8".to_string(),
                            )],
                            body,
                            size_bytes: size,
                            transferred_bytes: size,
                            response_time_ms: 0,
                            truncated: false,
                            decode_error: None,
                            md5_hex: String::new(),
                            request_headers: Vec::new(),
                            set_cookies: Vec::new(),
                            content_length: None,
                            untruncated_bytes: None,
                            synthetic: None,
                            attempts: 1,
                        }),
                    }
                });
            }
        }
        self.inner.fetch(req)
    }
}

/// Was `restore` aus der Datei aufbaut.
pub(crate) struct Restored {
    /// Kernzustand (Seen-Set, Warteschlange, Zähler).
    pub core: Core,
    /// Abgerufene URLs (Fortschritt „gecrawlt“).
    pub crawled: u64,
    /// robots.txt-Dateien, die nicht neu abgerufen werden (`(robots_url, Inhalt)`).
    pub primed: Vec<(String, PrimedRobots)>,
    /// Hosts aller Listen-URLs (Listenmodus).
    pub list_hosts: Vec<String>,
    /// URL, an der der Crawl-Bereich hängt: die Start-URL, im Listenmodus die erste gültige Listen-URL bzw.
    /// Sitemap-Adresse.
    pub anchor: Option<String>,
    /// Adressen von Sitemap-Dateien der Liste (`list_input.outcome = sitemap`).
    pub list_sitemaps: Vec<String>,
}

fn sql<T, E: Into<StoreError>>(r: Result<T, E>) -> Result<T, EngineError> {
    r.map_err(|e| EngineError::Store(e.into()))
}

fn meta_i64(file: &CrawlFile, key: &str) -> Result<i64, EngineError> {
    Ok(file
        .get_meta(key)?
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0))
}

/// Baut den Kernzustand aus der Datei. `load_queue = false` lässt die Warteschlange leer (Neu crawlen).
pub(crate) fn restore(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    mode: CrawlMode,
    load_queue: bool,
) -> Result<Restored, EngineError> {
    let conn = file.conn();
    let max_depth = match (mode, cfg.limits.max_depth) {
        (CrawlMode::List, None) => Some(0),
        (_, d) => d,
    };
    let mut core = Core::new(cfg.limits.max_urls, max_depth);
    core.list_mode = mode == CrawlMode::List;
    if cfg.advanced.cookies == os_model::CookieMode::SessionOnly {
        core.sessions[0] = Some(Arc::new(CookieJar::new()));
    }

    // Sitemap-Dateien: ihre URL-Zeilen zählen nicht als Abrufe
    let mut sitemap_rows: HashSet<i64> = HashSet::new();
    let mut max_sitemap_id = 0i64;
    {
        let mut stmt = sql(conn.prepare("SELECT id, url, url_id FROM sitemaps"))?;
        let rows = sql(stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
            ))
        }))?;
        for row in rows {
            let (id, url, url_id) = sql(row)?;
            max_sitemap_id = max_sitemap_id.max(id);
            if let Some(u) = url_id {
                sitemap_rows.insert(u);
            }
            if let Ok(parsed) = url::Url::parse(&url) {
                core.sitemap_keys
                    .insert(crate::sitemaps::sitemap_key(&parsed));
            }
        }
    }
    core.next_sitemap_id = max_sitemap_id + 1;
    {
        // die End-URL einer Sitemap-Datei (nach Weiterleitungen) steht als URL-Zeile
        let mut stmt = sql(conn.prepare(
            "SELECT u.url FROM sitemaps s JOIN urls u ON u.id = s.url_id WHERE s.url_id IS NOT NULL",
        ))?;
        let rows = sql(stmt.query_map([], |r| r.get::<_, String>(0)))?;
        for row in rows {
            if let Ok(parsed) = url::Url::parse(&sql(row)?) {
                core.sitemap_keys
                    .insert(crate::sitemaps::sitemap_key(&parsed));
            }
        }
    }
    {
        let mut stmt = sql(conn.prepare(
            "SELECT url_id, MAX(position) + 1 FROM hreflang WHERE source = 'sitemap' GROUP BY url_id",
        ))?;
        let rows = sql(stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))))?;
        for row in rows {
            let (id, next) = sql(row)?;
            core.sitemap_hreflang_next
                .insert(UrlId(id), u32::try_from(next).unwrap_or(u32::MAX));
        }
    }

    // Seen-Set und Zähler in einem Durchlauf
    let mut crawled = 0u64;
    let mut started = 0u64;
    let mut max_id = 0i64;
    let mut redirect_edges: Vec<(i64, i64)> = Vec::new();
    {
        let mut stmt = sql(conn.prepare(
            "SELECT id, url, crawl_depth, state, in_tables, discovered_via, no_response_reason, \
                    redirect_target_id FROM urls ORDER BY id",
        ))?;
        let mut rows = sql(stmt.query([]))?;
        while let Some(row) = sql(rows.next())? {
            let id: i64 = sql(row.get(0))?;
            let url: String = sql(row.get(1))?;
            let depth: Option<i64> = sql(row.get(2))?;
            let state: String = sql(row.get(3))?;
            let in_tables: i64 = sql(row.get(4))?;
            let via: String = sql(row.get(5))?;
            let no_response: Option<String> = sql(row.get(6))?;
            let redirect_target: Option<i64> = sql(row.get(7))?;
            max_id = max_id.max(id);
            let mut flags = 0u8;
            if state == "queued" || state == "done" {
                flags |= F_QUEUED | F_RULE_OK;
            }
            if in_tables != 0 {
                flags |= F_IN_TABLES;
            }
            if state == "done" && via != "robots_txt" && !sitemap_rows.contains(&id) {
                crawled += 1;
                let blocked = matches!(
                    no_response.as_deref(),
                    Some("blocked_by_robots" | "blocked_by_robots_unreachable")
                );
                if !blocked {
                    started += 1;
                }
            }
            if let Some(target) = redirect_target {
                redirect_edges.push((id, target));
            }
            core.register(
                &url,
                UrlId(id),
                depth.and_then(|d| u32::try_from(d).ok()),
                flags,
            );
        }
    }
    core.started = started;
    for (from, to) in &redirect_edges {
        let _ = core.redirects.add_edge(UrlId(*from), UrlId(*to));
    }
    {
        let mut stmt = sql(
            conn.prepare("SELECT url_id FROM url_flags WHERE flag = 'redirect_limit_reached'")
        )?;
        let rows = sql(stmt.query_map([], |r| r.get::<_, i64>(0)))?;
        for row in rows {
            core.redirects.flag_limit(UrlId(sql(row)?));
        }
    }

    // Kennungen
    let next_id = meta_i64(file, "next_url_id")?
        .max(max_id.saturating_add(1))
        .max(1);
    let max_seq: i64 =
        sql(conn.query_row("SELECT COALESCE(MAX(seq), 0) FROM queue", [], |r| r.get(0)))?;
    let next_seq = meta_i64(file, "next_seq")?
        .max(max_seq.saturating_add(1))
        .max(1);
    core.next_url_id = next_id;
    core.next_seq = u64::try_from(next_seq).unwrap_or(1);
    let discovery_seq: u64 = sql(file.conn().query_row(
        "SELECT COALESCE(MAX(seq), 0) FROM discovery_edges",
        [],
        |r| r.get(0),
    ))?;
    core.next_seq = core.next_seq.max(discovery_seq.saturating_add(1));

    if load_queue {
        core.frontier
            .enable_spill(crate::queue::queue_window(), file.path().to_path_buf());
        load_queue_into(file, &mut core, &redirect_edges)?;
    }

    // robots.txt
    let mut primed = Vec::new();
    {
        let mut stmt = sql(conn.prepare(
            "SELECT origin, robots_url, fetch_status, status_code, body FROM robots_files",
        ))?;
        let rows = sql(stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<i64>>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        }))?;
        for row in rows {
            let (origin, robots_url, fetch_status, status, body) = sql(row)?;
            let status = status.and_then(|s| u16::try_from(s).ok());
            let entry = match (fetch_status.as_str(), status, body) {
                ("ok", Some(s), Some(body)) if (200..300).contains(&s) => Some((s, body)),
                ("not_found", Some(s), _) if (400..500).contains(&s) && s != 429 => {
                    Some((s, String::new()))
                }
                _ => None,
            };
            if let Some((status, body)) = entry {
                let key = url::Url::parse(&robots_url)
                    .map(|u| u.to_string())
                    .unwrap_or(robots_url);
                primed.push((
                    key,
                    PrimedRobots {
                        origin,
                        status,
                        body,
                    },
                ));
            }
        }
    }

    // Listenmodus: Anker des Bereichs, Hosts, Sitemap-Adressen
    let mut list_hosts = Vec::new();
    let mut list_sitemaps = Vec::new();
    let mut anchor = None;
    if mode == CrawlMode::List {
        {
            let mut stmt = sql(conn.prepare(
                "SELECT DISTINCT u.origin FROM list_input l JOIN urls u ON u.id = l.url_id",
            ))?;
            let rows = sql(stmt.query_map([], |r| r.get::<_, String>(0)))?;
            for row in rows {
                if let Some(host) = url::Url::parse(&sql(row)?)
                    .ok()
                    .and_then(|u| u.host_str().map(str::to_string))
                {
                    list_hosts.push(host);
                }
            }
        }
        {
            let mut stmt = sql(conn.prepare(
                "SELECT u.url FROM list_input l JOIN urls u ON u.id = l.url_id \
                 WHERE l.url_id IS NOT NULL ORDER BY l.position LIMIT 1",
            ))?;
            let mut rows = sql(stmt.query([]))?;
            if let Some(row) = sql(rows.next())? {
                anchor = Some(sql(row.get::<_, String>(0))?);
            }
        }
        {
            let mut stmt = sql(conn.prepare(
                "SELECT original FROM list_input WHERE outcome = 'sitemap' ORDER BY position",
            ))?;
            let rows = sql(stmt.query_map([], |r| r.get::<_, String>(0)))?;
            for row in rows {
                list_sitemaps.push(sql(row)?);
            }
        }
        if anchor.is_none() {
            anchor = list_sitemaps
                .first()
                .and_then(|s| os_url::parse_start_url(s).ok())
                .map(|n| n.encoded);
        }
        for s in &list_sitemaps {
            if let Some(host) = os_url::parse_start_url(s)
                .ok()
                .and_then(|n| n.url.host_str().map(str::to_string))
            {
                list_hosts.push(host);
            }
        }
    }

    Ok(Restored {
        core,
        crawled,
        primed,
        list_hosts,
        anchor,
        list_sitemaps,
    })
}

/// Lädt die Warteschlange in `core.frontier`: das RAM-Fenster und die nicht auslagerbaren Einträge, der Rest bleibt
/// in der Tabelle `queue` (siehe [`crate::queue_disk::load_queue`]).
fn load_queue_into(
    file: &CrawlFile,
    core: &mut Core,
    redirect_edges: &[(i64, i64)],
) -> Result<(), EngineError> {
    // Weiterleitungsketten: Quelle je Ziel
    let mut source_of: HashMap<i64, i64> = HashMap::new();
    for (from, to) in redirect_edges {
        source_of.entry(*to).or_insert(*from);
    }
    let hops_of = |id: i64| -> u32 {
        let mut hops = 0u32;
        let mut current = id;
        while let Some(&src) = source_of.get(&current) {
            hops += 1;
            current = src;
            if hops > 1_000 {
                break;
            }
        }
        hops
    };
    let targets: HashSet<i64> = source_of.keys().copied().collect();
    crate::queue_disk::load_queue(file, core, &targets, &hops_of, crate::queue::queue_window())
        .map_err(EngineError::Join)
}
