//! Laden, Zusammenführen und Zuordnen des GA4-Reports (INTEG-092 … 096, INTEG-099, INTEG-014).
//!
//! Je Seitendimension entstehen Anfragen mit höchstens zehn Metriken; jede wird seitenweise geladen (`limit`/`offset`)
//! bis `rowCount` bzw. das Ergebnislimit erreicht ist. Die Antworten werden über den Dimensionswert zusammengeführt:
//! eine URL, die nur in einer Teilantwort vorkommt, erhält nur deren Werte. Für den URL-Abgleich hält [`Report`] je
//! Seitendimension einen Index vom Abgleichsschlüssel (INTEG-014) auf die Variante mit den meisten Sitzungen.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Mutex;
use std::time::Duration;

use futures_util::{stream, StreamExt, TryStreamExt};
use os_model::config::UrlMatchConfig;
use os_model::{ApiValueWrite, Clock, Timestamp};
use serde_json::{json, Value};

use super::client::Ga4Client;
use super::settings::{
    value_key, Ga4Settings, PageDimension, RequestDates, MAX_METRICS_PER_REQUEST,
};
use crate::core::ProviderQuota;
use crate::{matching, ApiError, UrlRef};

/// Hinweis bei `dataLossFromOtherRow` (INTEG-096).
pub const OTHER_ROW_HINT: &str = "GA4 hat Zeilen zu (other) zusammengefasst – Daten unvollständig";
/// Hinweis bei `subjectToThresholding` (INTEG-096).
pub const THRESHOLD_HINT: &str =
    "GA4 hat Schwellenwerte angewendet – Zeilen mit wenigen Nutzern fehlen möglicherweise";

/// Anteil des Stundenkontingents, unter dem Anfragen bis zur nächsten vollen Stunde zurückgehalten werden (INTEG-099).
pub const HOURLY_RESERVE: f64 = 0.05;

/// Kontingentstand einer Dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TokenQuota {
    /// Verbraucht.
    pub consumed: u64,
    /// Verbleibend.
    pub remaining: u64,
}

/// Kontingente aus `propertyQuota` und Zurückhaltung bis zum Stundenwechsel (INTEG-099, INTEG-006).
#[derive(Default)]
pub struct QuotaTracker {
    state: Mutex<QuotaState>,
}

#[derive(Default)]
struct QuotaState {
    per_day: Option<TokenQuota>,
    per_hour: Option<TokenQuota>,
    hold_until: Option<Timestamp>,
}

fn parse_quota(v: &Value) -> Option<TokenQuota> {
    let num = |k: &str| v[k].as_u64().or_else(|| v[k].as_str()?.parse().ok());
    Some(TokenQuota {
        consumed: num("consumed").unwrap_or(0),
        remaining: num("remaining")?,
    })
}

/// Beginn der nächsten vollen Stunde (UTC).
pub fn next_hour(now: Timestamp) -> Timestamp {
    let ms = now.unix_millis();
    Timestamp::from_unix_millis((ms / 3_600_000 + 1) * 3_600_000)
}

impl QuotaTracker {
    /// Übernimmt `propertyQuota` einer Antwort.
    pub fn update(&self, quota: &Value, now: Timestamp) {
        let mut s = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(day) = parse_quota(&quota["tokensPerDay"]) {
            s.per_day = Some(day);
        }
        if let Some(hour) = parse_quota(&quota["tokensPerHour"]) {
            s.per_hour = Some(hour);
            let total = (hour.consumed + hour.remaining) as f64;
            if total > 0.0 && (hour.remaining as f64) < total * HOURLY_RESERVE {
                s.hold_until = Some(next_hour(now));
            }
        }
    }

    /// Ende einer laufenden Zurückhaltung.
    pub fn hold_until(&self) -> Option<Timestamp> {
        self.state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .hold_until
    }

    /// Wartet, bis keine Zurückhaltung mehr gilt (Uhr der Quelle; prüft in Schritten von höchstens einer Sekunde).
    pub async fn ready(&self, clock: &dyn Clock) {
        loop {
            let Some(until) = self.hold_until() else {
                return;
            };
            let now = clock.now();
            if now >= until {
                let mut s = self.state.lock().unwrap_or_else(|p| p.into_inner());
                if s.hold_until == Some(until) {
                    s.hold_until = None;
                    if let Some(hour) = &mut s.per_hour {
                        hour.remaining += hour.consumed;
                        hour.consumed = 0;
                    }
                }
                return;
            }
            let wait = (until.unix_millis() - now.unix_millis()).clamp(1, 1000) as u64;
            tokio::time::sleep(Duration::from_millis(wait)).await;
        }
    }

    /// Kontingente für die Anzeige (`tokensPerDay`, `tokensPerHour`).
    pub fn quotas(&self) -> Vec<ProviderQuota> {
        let s = self.state.lock().unwrap_or_else(|p| p.into_inner());
        let mut out = Vec::new();
        for (name, q, reset) in [
            ("tokensPerDay", s.per_day, None),
            ("tokensPerHour", s.per_hour, s.hold_until),
        ] {
            if let Some(q) = q {
                out.push(ProviderQuota {
                    dimension: name.into(),
                    used: q.consumed,
                    limit: Some(q.consumed + q.remaining),
                    resets_at: reset,
                });
            }
        }
        out
    }
}

/// Werte einer URL je Metrikschlüssel (`sessions`, `sessions@landing`).
pub type RowValues = BTreeMap<String, f64>;

/// Ein geladener Report.
#[derive(Debug, Default)]
pub struct Report {
    /// Zeilen je Seitendimension: absolute URL → Werte.
    pub rows: BTreeMap<PageDimension, BTreeMap<String, RowValues>>,
    /// Hinweise (Filter, `(other)`, Schwellenwerte, entfernte Metriken).
    pub notices: Vec<String>,
    /// Währung der Property.
    pub currency: Option<String>,
    /// Zeitraum.
    pub dates: Option<RequestDates>,
    /// Zahl der `runReport`-Anfragen.
    pub requests: usize,
    index: HashMap<PageDimension, HashMap<String, String>>,
}

/// Ergebnis der Zuordnung aller URLs (für den Crawl- und APIs-Modus, M2-I06).
#[derive(Debug, Default)]
pub struct Assignment {
    /// Werte der gecrawlten URLs.
    pub values: Vec<ApiValueWrite>,
    /// GA4-URLs ohne gecrawltes Gegenstück auf den erlaubten Hosts (Orphan-Kandidaten, INTEG-015).
    pub orphans: Vec<String>,
}

impl Report {
    /// Baut den Abgleichsindex: je Schlüssel die Variante mit den meisten Sitzungen, bei Gleichstand die mit mehr
    /// Aufrufen, danach die alphabetisch erste (Werte werden nie summiert, INTEG-014).
    pub fn build_index(&mut self, options: &UrlMatchConfig) {
        self.index.clear();
        for (dim, rows) in &self.rows {
            let sessions = value_key("sessions", *dim);
            let views = value_key("screenPageViews", *dim);
            let rank = |v: &RowValues| {
                (
                    v.get(&sessions).copied().unwrap_or(0.0),
                    v.get(&views).copied().unwrap_or(0.0),
                )
            };
            let mut best: HashMap<String, String> = HashMap::new();
            for (url, values) in rows {
                let Some(key) = matching::match_key(url, options) else {
                    continue;
                };
                match best.get(&key) {
                    Some(current) => {
                        let (a1, a2) = rank(values);
                        let (b1, b2) = rank(&rows[current]);
                        if a1.total_cmp(&b1).then(a2.total_cmp(&b2)).is_gt() {
                            best.insert(key, url.clone());
                        }
                    }
                    None => {
                        best.insert(key, url.clone());
                    }
                }
            }
            self.index.insert(*dim, best);
        }
    }

    /// Werte einer gecrawlten URL; leer, wenn GA4 keine Zeile geliefert hat.
    pub fn values_for(
        &self,
        url: &UrlRef,
        options: &UrlMatchConfig,
        fetched_at: Timestamp,
    ) -> Vec<ApiValueWrite> {
        let Some(key) = matching::match_key(&url.url, options) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for (dim, rows) in &self.rows {
            let Some(source_url) = self.index.get(dim).and_then(|i| i.get(&key)) else {
                continue;
            };
            for (metric, value) in &rows[source_url] {
                out.push(ApiValueWrite {
                    url_id: url.url_id,
                    url: source_url.clone(),
                    source: "ga4".into(),
                    metric: metric.clone(),
                    level: "url".into(),
                    v_real: Some(*value),
                    v_text: None,
                    fetched_at,
                });
            }
        }
        out
    }

    /// Ordnet alle gecrawlten URLs zu und meldet GA4-URLs ohne Gegenstück auf `hosts` (Orphan-Kandidaten).
    pub fn assign(
        &self,
        urls: &[UrlRef],
        options: &UrlMatchConfig,
        hosts: &BTreeSet<String>,
        fetched_at: Timestamp,
    ) -> Assignment {
        let mut out = Assignment::default();
        let mut matched: BTreeSet<String> = BTreeSet::new();
        for url in urls {
            if let Some(key) = matching::match_key(&url.url, options) {
                matched.insert(key);
            }
            if url.url_id.is_some() {
                out.values.extend(self.values_for(url, options, fetched_at));
            }
        }
        let mut orphans = BTreeSet::new();
        for index in self.index.values() {
            for (key, url) in index {
                let host = url::Url::parse(url)
                    .ok()
                    .and_then(|u| u.host_str().map(str::to_lowercase));
                if !matched.contains(key) && host.is_some_and(|h| hosts.contains(&h)) {
                    orphans.insert(url.clone());
                }
            }
        }
        out.orphans = orphans.into_iter().collect();
        out
    }
}

/// Umgebung eines Ladevorgangs.
pub struct LoadCtx<'a> {
    /// Client der Property.
    pub client: &'a Ga4Client,
    /// Einstellungen.
    pub settings: &'a Ga4Settings,
    /// Zeitraum.
    pub dates: &'a RequestDates,
    /// Host des gewählten Datenstreams.
    pub stream_host: Option<&'a str>,
    /// Kontingente.
    pub quota: &'a QuotaTracker,
    /// Uhr.
    pub clock: &'a dyn Clock,
    /// Gleichzeitige Seitenabrufe.
    pub concurrency: usize,
}

/// Anfrage-Body einer Seite.
pub fn request_body(
    ctx: &LoadCtx<'_>,
    dimension: PageDimension,
    metrics: &[String],
    offset: u64,
    limit: u64,
) -> Value {
    let mut dims = Vec::new();
    if dimension == PageDimension::LandingPage && ctx.stream_host.is_none() {
        dims.push(json!({"name": "hostName"}));
    }
    dims.push(json!({"name": dimension.api()}));
    let order = if metrics.iter().any(|m| m == "sessions") {
        "sessions"
    } else {
        metrics.first().map_or("sessions", String::as_str)
    };
    let mut body = json!({
        "dateRanges": [{"startDate": ctx.dates.start, "endDate": ctx.dates.end}],
        "dimensions": dims,
        "metrics": metrics.iter().map(|m| json!({"name": m})).collect::<Vec<_>>(),
        "orderBys": [{"metric": {"metricName": order}, "desc": true}],
        "limit": limit.to_string(),
        "offset": offset.to_string(),
        "keepEmptyRows": false,
        "returnPropertyQuota": true
    });
    if let Some(filter) = ctx.settings.dimension_filter(ctx.stream_host) {
        body["dimensionFilter"] = filter;
    }
    body
}

/// Absolute URL einer Zeile; `None` für `(not set)`, `(other)`, fremde Hosts und unlesbare Werte (INTEG-093).
fn row_url(row: &Value, dimension: PageDimension, ctx: &LoadCtx<'_>) -> Option<String> {
    let values: Vec<&str> = row["dimensionValues"]
        .as_array()?
        .iter()
        .map(|v| v["value"].as_str().unwrap_or_default())
        .collect();
    let url = match dimension {
        PageDimension::PageLocation => values.first()?.to_string(),
        PageDimension::LandingPage => {
            let (host, path) = match ctx.stream_host {
                Some(h) => (h, *values.first()?),
                None => (*values.first()?, *values.get(1)?),
            };
            if !path.starts_with('/') || host.is_empty() || host.starts_with('(') {
                return None;
            }
            format!("https://{host}{path}")
        }
    };
    let parsed = url::Url::parse(&url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    if let Some(stream) = ctx.stream_host {
        if !parsed
            .host_str()
            .is_some_and(|h| h.eq_ignore_ascii_case(stream))
        {
            return None;
        }
    }
    Some(url)
}

fn metric_value(v: &Value) -> Option<f64> {
    v["value"].as_str()?.parse().ok()
}

/// Metrik aus der Meldung „Field X is not a valid metric“ (INTEG-092).
fn unknown_metric(message: &str, metrics: &[String]) -> Option<String> {
    if !message.contains("not a valid metric") {
        return None;
    }
    let named = message
        .split_whitespace()
        .skip_while(|w| *w != "Field")
        .nth(1)
        .map(|w| w.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != ':' && c != '_'));
    named
        .filter(|n| metrics.iter().any(|m| m == n))
        .map(str::to_string)
        .or_else(|| {
            metrics
                .iter()
                .find(|m| message.contains(m.as_str()))
                .cloned()
        })
}

struct Page {
    rows: Vec<(String, Vec<f64>)>,
    row_count: u64,
    headers: Vec<String>,
}

async fn fetch_page(
    ctx: &LoadCtx<'_>,
    dimension: PageDimension,
    metrics: &[String],
    offset: u64,
    limit: u64,
    notices: &Mutex<BTreeSet<String>>,
) -> Result<Page, ApiError> {
    ctx.quota.ready(ctx.clock).await;
    let body = request_body(ctx, dimension, metrics, offset, limit);
    let response = ctx.client.run_report(&ctx.settings.property, &body).await?;
    if let Some(q) = response.get("propertyQuota") {
        ctx.quota.update(q, ctx.clock.now());
    }
    let meta = &response["metadata"];
    let mut n = notices.lock().unwrap_or_else(|p| p.into_inner());
    if meta["dataLossFromOtherRow"].as_bool() == Some(true) {
        n.insert(OTHER_ROW_HINT.to_string());
    }
    if meta["subjectToThresholding"].as_bool() == Some(true) {
        n.insert(THRESHOLD_HINT.to_string());
    }
    drop(n);
    let headers: Vec<String> = response["metricHeaders"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|h| h["name"].as_str().map(str::to_string))
        .collect();
    let headers = if headers.is_empty() {
        metrics.to_vec()
    } else {
        headers
    };
    let rows = response["rows"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|row| {
            let url = row_url(row, dimension, ctx)?;
            let values = row["metricValues"]
                .as_array()?
                .iter()
                .map(|v| metric_value(v).unwrap_or(f64::NAN))
                .collect();
            Some((url, values))
        })
        .collect();
    let returned = response["rows"].as_array().map_or(0, Vec::len) as u64;
    Ok(Page {
        rows,
        row_count: response["rowCount"]
            .as_u64()
            .or_else(|| response["rowCount"].as_str()?.parse().ok())
            .unwrap_or(offset + returned),
        headers,
    })
}

/// Lädt eine Teilanfrage (eine Seitendimension, höchstens zehn Metriken) über alle Seiten.
async fn load_chunk(
    ctx: &LoadCtx<'_>,
    dimension: PageDimension,
    metrics: &[String],
    notices: &Mutex<BTreeSet<String>>,
    requests: &Mutex<usize>,
) -> Result<Vec<Page>, ApiError> {
    let page_size = ctx.settings.page_size;
    let limit_total = ctx.settings.max_results;
    let count = |n: usize| *requests.lock().unwrap_or_else(|p| p.into_inner()) += n;
    let first = fetch_page(
        ctx,
        dimension,
        metrics,
        0,
        page_size.min(limit_total),
        notices,
    )
    .await?;
    count(1);
    let total = first.row_count.min(limit_total);
    let offsets: Vec<(u64, u64)> = (1..)
        .map(|i| i * page_size)
        .take_while(|o| *o < total)
        .map(|o| (o, page_size.min(limit_total - o)))
        .collect();
    count(offsets.len());
    let mut pages = vec![first];
    let rest: Vec<Page> = stream::iter(offsets)
        .map(|(offset, limit)| fetch_page(ctx, dimension, metrics, offset, limit, notices))
        .buffered(ctx.concurrency.max(1))
        .try_collect()
        .await?;
    pages.extend(rest);
    Ok(pages)
}

/// Lädt den vollständigen Report.
pub async fn load(ctx: &LoadCtx<'_>) -> Result<Report, ApiError> {
    let notices = Mutex::new(BTreeSet::new());
    let requests = Mutex::new(0usize);
    let mut report = Report {
        dates: Some(ctx.dates.clone()),
        ..Report::default()
    };
    let mut removed: Vec<String> = Vec::new();
    for dimension in ctx.settings.page_dimensions() {
        let all = ctx.settings.request_metrics(dimension);
        let rows = report.rows.entry(dimension).or_default();
        for chunk in all.chunks(MAX_METRICS_PER_REQUEST) {
            let mut chunk: Vec<String> = chunk.to_vec();
            let pages = loop {
                if chunk.is_empty() {
                    break Vec::new();
                }
                match load_chunk(ctx, dimension, &chunk, &notices, &requests).await {
                    Ok(pages) => break pages,
                    Err(ApiError::Provider {
                        status: 400,
                        message,
                    }) => {
                        let Some(bad) = unknown_metric(&message, &chunk) else {
                            return Err(ApiError::Provider {
                                status: 400,
                                message,
                            });
                        };
                        chunk.retain(|m| *m != bad);
                        removed.push(bad);
                    }
                    Err(e) => return Err(e),
                }
            };
            for page in pages {
                for (url, values) in page.rows {
                    let entry = rows.entry(url).or_default();
                    for (name, value) in page.headers.iter().zip(values) {
                        if value.is_finite() {
                            entry.insert(value_key(name, dimension), value);
                        }
                    }
                }
            }
        }
    }
    report.requests = *requests.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(hint) = ctx.settings.filter_hint() {
        report.notices.push(hint);
    }
    for m in removed {
        report.notices.push(format!(
            "GA4 kennt die Metrik {m} nicht mehr; sie wurde aus der Anfrage entfernt"
        ));
    }
    report
        .notices
        .extend(notices.into_inner().unwrap_or_else(|p| p.into_inner()));
    report.build_index(&ctx.settings.url_match);
    Ok(report)
}
