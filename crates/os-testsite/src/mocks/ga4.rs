//! Mock: Google Analytics 4 (INTEG-090 ff.) – Admin API (`accountSummaries`, `dataStreams`, Property) und Data API
//! (`runReport`, `getMetadata`, `checkCompatibility`) unter **einer** Basis-URL (`api.ga4.base_url`, INTEG-019).
//!
//! Der Mock hält seinen Zustand in [`Ga4Mock::state`]; Tests klonen den Mock vor dem Start des [`super::MockServer`] und
//! steuern darüber Szenario, Datensatz, Kontingent, Verzögerung und Hinweis-Flags. Jede Anfrage wird mit Pfad und
//! JSON-Body protokolliert ([`Ga4Mock::requests`]), die höchste Zahl gleichzeitig offener Report-Anfragen ist abrufbar
//! ([`Ga4Mock::max_inflight`]).
//!
//! Fixture-Daten (INTEG-097/098): [`SITE_SCENARIO`] ist das Szenario der Testsite mit acht Seiten, [`SITE_ROWS`] die
//! GA4-Werte dieser Seiten und [`EXPECTED_FILTERS`] die erwartete Filterzählung des Analytics-Tabs. Der Standard-Datensatz
//! des Mocks ([`default_rows`]) enthält dieselben Seiten auf dem Host `integ.test`, die Varianten des URL-Abgleichs
//! (INTEG-014), eine Zeile eines fremden Hosts und eine `(not set)`-Zeile (INTEG-093).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::{
    body::Bytes,
    http::{HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    Json, Router,
};
use serde_json::{json, Value};

use super::common::{scenario_response, CallLog};
use super::MockApi;

/// Gemeinsamer Metrikkatalog (Quelle: `os-integrations`, INTEG-092).
pub const CATALOG_JSON: &str = include_str!("../../../os-integrations/src/ga4/metrics.json");

/// Benutzerdefinierte Metrik der Fixture-Property (Kategorie „Custom“, INTEG-092).
pub const CUSTOM_METRIC: (&str, &str) = ("customEvent:fixture_score", "Fixture Score");

/// Property mit Web-Datenströmen (INTEG-090).
pub const PROPERTY: &str = "properties/2001";
/// Datenstrom mit der Domain `integ.test`.
pub const STREAM: &str = "properties/2001/dataStreams/3001";
/// Host des Datenstroms [`STREAM`].
pub const STREAM_HOST: &str = "integ.test";

/// Szenario der Testsite mit acht HTML-Seiten (INTEG-097/098), Start `http://integ.test/`; `/orphan` ist nur über die
/// Sitemap der robots.txt erreichbar (Crawl mit `sitemaps.crawl_linked = true`).
pub const SITE_SCENARIO: &str = r#"
[[route]]
path = "/"
content_type = "text/html; charset=utf-8"
body = '''<html><head><title>Home</title></head><body><a href="/about">about</a><a href="/zero">zero</a><a href="/five">five</a><a href="/three">three</a><a href="/noindex">noindex</a><a href="/nodata">nodata</a></body></html>'''

[[route]]
path = "/about"
content_type = "text/html; charset=utf-8"
body = '''<html><head><title>About</title></head><body><a href="/">home</a></body></html>'''

[[route]]
path = "/zero"
content_type = "text/html; charset=utf-8"
body = '''<html><head><title>Zero</title></head><body><a href="/">home</a></body></html>'''

[[route]]
path = "/five"
content_type = "text/html; charset=utf-8"
body = '''<html><head><title>Five</title></head><body><a href="/">home</a></body></html>'''

[[route]]
path = "/three"
content_type = "text/html; charset=utf-8"
body = '''<html><head><title>Three</title></head><body><a href="/">home</a></body></html>'''

[[route]]
path = "/noindex"
content_type = "text/html; charset=utf-8"
body = '''<html><head><title>Noindex</title><meta name="robots" content="noindex"></head><body><a href="/">home</a></body></html>'''

[[route]]
path = "/nodata"
content_type = "text/html; charset=utf-8"
body = '''<html><head><title>No data</title></head><body><a href="/">home</a></body></html>'''

[[route]]
path = "/orphan"
content_type = "text/html; charset=utf-8"
body = '''<html><head><title>Orphan</title></head><body>no inlinks</body></html>'''

[[route]]
path = "/robots.txt"
content_type = "text/plain"
body = '''User-agent: *
Allow: /
Sitemap: http://integ.test/sitemap.xml
'''

[[route]]
path = "/sitemap.xml"
content_type = "application/xml"
body = '''<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"><url><loc>http://integ.test/orphan</loc></url></urlset>'''
"#;

/// GA4-Werte einer Fixture-Seite (Standardmetriken plus Bounce Rate).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SiteRow {
    /// Pfad der Seite.
    pub path: &'static str,
    /// `sessions`.
    pub sessions: f64,
    /// `bounceRate` (Anteil, `0.70` = 70 %).
    pub bounce_rate: f64,
}

/// GA4-Daten der Testsite: `/nodata` hat keine Zeile, `/zero` liefert `0` Sitzungen (aber ein Ereignis, sonst liefe die
/// Zeile wie bei der Data API ohne `keepEmptyRows` aus), `/about` genau 100 Sitzungen (INTEG-100).
pub const SITE_ROWS: &[SiteRow] = &[
    SiteRow {
        path: "/",
        sessions: 120.0,
        bounce_rate: 0.40,
    },
    SiteRow {
        path: "/about",
        sessions: 100.0,
        bounce_rate: 0.20,
    },
    SiteRow {
        path: "/zero",
        sessions: 0.0,
        bounce_rate: 0.0,
    },
    SiteRow {
        path: "/five",
        sessions: 5.0,
        bounce_rate: 0.70,
    },
    SiteRow {
        path: "/three",
        sessions: 3.0,
        bounce_rate: 0.71,
    },
    SiteRow {
        path: "/noindex",
        sessions: 9.0,
        bounce_rate: 0.50,
    },
    SiteRow {
        path: "/orphan",
        sessions: 4.0,
        bounce_rate: 0.25,
    },
];

/// Erwartete Filterzählung des Analytics-Tabs für [`SITE_SCENARIO`] + [`SITE_ROWS`] nach der Crawl-Analyse.
pub const EXPECTED_FILTERS: &[(&str, u64)] = &[
    ("All", 8),
    ("Sessions Above 0", 6),
    ("Bounce Rate Above 70%", 1),
    ("No GA Data", 1),
    ("Non-Indexable with GA Data", 1),
    ("Orphan URLs", 1),
];

impl SiteRow {
    /// Alle Standardmetriken der Seite (abgeleitet aus Sitzungen und Bounce Rate).
    pub fn metrics(&self) -> BTreeMap<String, f64> {
        let s = self.sessions;
        let engaged = (s * (1.0 - self.bounce_rate)).round();
        BTreeMap::from([
            ("sessions".to_string(), s),
            ("engagedSessions".to_string(), engaged),
            ("engagementRate".to_string(), 1.0 - self.bounce_rate),
            ("bounceRate".to_string(), self.bounce_rate),
            ("screenPageViews".to_string(), s * 2.0),
            ("keyEvents".to_string(), (s / 10.0).floor()),
            ("eventCount".to_string(), s * 5.0 + 1.0),
            ("totalRevenue".to_string(), s * 1.5),
            ("activeUsers".to_string(), (s * 0.8).round()),
            (CUSTOM_METRIC.0.to_string(), s / 4.0),
        ])
    }
}

/// Eine Datenzeile des Mocks: Dimensionswerte und Metriken.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MockRow {
    /// Dimensionswerte nach API-Name; fehlende Dimensionen liefern `(not set)`.
    pub dims: BTreeMap<String, String>,
    /// Metrikwerte nach API-Name; fehlende Metriken liefern `0`.
    pub metrics: BTreeMap<String, f64>,
}

impl MockRow {
    /// Zeile für eine Seiten-URL; `hostName`, `pagePath` und `landingPagePlusQueryString` werden abgeleitet.
    pub fn page(url: &str, metrics: BTreeMap<String, f64>) -> MockRow {
        let (host, path) = split_url(url);
        let mut dims = BTreeMap::new();
        dims.insert("pageLocation".to_string(), url.to_string());
        dims.insert("hostName".to_string(), host.clone());
        dims.insert(
            "pagePath".to_string(),
            path.split('?').next().unwrap_or_default().to_string(),
        );
        dims.insert("landingPagePlusQueryString".to_string(), path);
        dims.insert(
            "sessionDefaultChannelGroup".to_string(),
            "Organic Search".to_string(),
        );
        dims.insert("firstUserSource".to_string(), "google".to_string());
        MockRow { dims, metrics }
    }

    /// Zeile mit nur Sitzungen (Abgleich-Fixture).
    pub fn sessions(url: &str, sessions: f64) -> MockRow {
        MockRow::page(url, BTreeMap::from([("sessions".to_string(), sessions)]))
    }
}

fn split_url(url: &str) -> (String, String) {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    match rest.find('/') {
        Some(i) => (rest[..i].to_string(), rest[i..].to_string()),
        None => (rest.to_string(), "/".to_string()),
    }
}

/// Standard-Datensatz: Testsite auf `integ.test`, Abgleich-Varianten (INTEG-014), fremder Host, `(not set)`.
pub fn default_rows() -> Vec<MockRow> {
    let mut rows: Vec<MockRow> = SITE_ROWS
        .iter()
        .map(|r| MockRow::page(&format!("https://{STREAM_HOST}{}", r.path), r.metrics()))
        .collect();
    for (url, sessions) in [
        ("http://integ.test/a/", 5.0),
        ("https://integ.test/b", 10.0),
        ("https://integ.test/b/", 30.0),
        ("https://integ.test/c/", 7.0),
        ("https://integ.test/Produkt", 5.0),
        ("https://integ.test/d?x=1", 2.0),
        ("https://integ.test/d", 1.0),
        ("https://integ.test/%C3%BC", 3.0),
        ("https://fremd.test/x", 50.0),
    ] {
        rows.push(MockRow::sessions(url, sessions));
    }
    let mut not_set = MockRow::sessions("https://integ.test/unused", 11.0);
    for dim in ["pageLocation", "landingPagePlusQueryString", "pagePath"] {
        not_set
            .dims
            .insert(dim.to_string(), "(not set)".to_string());
    }
    rows.push(not_set);
    rows
}

/// Synthetischer Datensatz: `n` Zeilen `https://integ.test/p/<i>` mit absteigenden Sitzungen (`n - i`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Synthetic {
    /// Zeilen insgesamt (`rowCount`).
    pub rows: usize,
    /// Höchstens so viele Zeilen je Antwort (nur `rowCount` bleibt exakt; für sehr große Seiten).
    pub page_cap: Option<usize>,
}

/// GA4-Kontingent einer Dimension (`consumed` / `remaining`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaStatus {
    /// Verbraucht.
    pub consumed: u64,
    /// Verbleibend.
    pub remaining: u64,
}

/// Steuerbarer Zustand des Mocks.
#[derive(Debug, Clone)]
pub struct MockConfig {
    /// Szenario (`ok`, `empty`, `error`, `throttle`, `no-properties`, `400-invalid-date-range`, `resource-exhausted`);
    /// der Header `X-Scenario` hat Vorrang.
    pub scenario: String,
    /// Datensatz.
    pub rows: Vec<MockRow>,
    /// Synthetischer Datensatz statt `rows`.
    pub synthetic: Option<Synthetic>,
    /// Kontingent je Stunde.
    pub tokens_per_hour: QuotaStatus,
    /// Kontingent je Tag.
    pub tokens_per_day: QuotaStatus,
    /// Tokens je `runReport`.
    pub tokens_per_report: u64,
    /// Verzögerung je `runReport`.
    pub delay: Duration,
    /// `metadata.dataLossFromOtherRow`.
    pub data_loss_from_other_row: bool,
    /// `metadata.subjectToThresholding`.
    pub thresholding: bool,
    /// `checkCompatibility` antwortet mit 503 (nicht erreichbar).
    pub compatibility_down: bool,
}

impl Default for MockConfig {
    fn default() -> Self {
        MockConfig {
            scenario: "ok".into(),
            rows: default_rows(),
            synthetic: None,
            tokens_per_hour: QuotaStatus {
                consumed: 0,
                remaining: 40_000,
            },
            tokens_per_day: QuotaStatus {
                consumed: 0,
                remaining: 200_000,
            },
            tokens_per_report: 10,
            delay: Duration::ZERO,
            data_loss_from_other_row: false,
            thresholding: false,
            compatibility_down: false,
        }
    }
}

/// Protokollierte Anfrage.
#[derive(Debug, Clone, PartialEq)]
pub struct Ga4Request {
    /// Methode.
    pub method: String,
    /// Pfad relativ zum Mock (`/v1beta/properties/2001:runReport`).
    pub path: String,
    /// JSON-Body (Null bei GET).
    pub body: Value,
    /// Zeitpunkt (tokio-Zeit) des Eingangs.
    pub at: tokio::time::Instant,
}

/// Gemeinsamer Zustand.
#[derive(Default)]
pub struct Ga4State {
    /// Redigiertes Aufruf-Log (Pfade).
    pub log: CallLog,
    config: Mutex<MockConfig>,
    requests: Mutex<Vec<Ga4Request>>,
    inflight: AtomicUsize,
    max_inflight: AtomicUsize,
}

/// GA4-Mock (`/ga4`).
#[derive(Clone, Default)]
pub struct Ga4Mock {
    /// Zustand, geteilt zwischen Test und Router.
    pub state: Arc<Ga4State>,
}

impl Ga4Mock {
    /// Ändert die Konfiguration.
    pub fn configure(&self, f: impl FnOnce(&mut MockConfig)) {
        f(&mut self.state.config.lock().unwrap_or_else(|p| p.into_inner()));
    }

    /// Setzt das Szenario.
    pub fn set_scenario(&self, scenario: &str) {
        self.configure(|c| c.scenario = scenario.to_string());
    }

    /// Alle bisherigen Anfragen.
    pub fn requests(&self) -> Vec<Ga4Request> {
        self.state
            .requests
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// Bodies aller Anfragen an einen Endpunkt (`runReport`, `checkCompatibility`).
    pub fn bodies(&self, endpoint: &str) -> Vec<Value> {
        self.requests()
            .into_iter()
            .filter(|r| r.path.ends_with(&format!(":{endpoint}")))
            .map(|r| r.body)
            .collect()
    }

    /// Höchste Zahl gleichzeitig offener `runReport`-Anfragen.
    pub fn max_inflight(&self) -> usize {
        self.state.max_inflight.load(Ordering::SeqCst)
    }

    /// Löscht das Anfrageprotokoll.
    pub fn clear_requests(&self) {
        self.state
            .requests
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clear();
    }
}

impl MockApi for Ga4Mock {
    fn name(&self) -> &'static str {
        "ga4"
    }

    fn base_url_key(&self) -> Option<&'static str> {
        Some("api.ga4.base_url")
    }

    fn router(&self) -> Router {
        Router::new()
            .fallback(handle)
            .with_state(self.state.clone())
    }

    fn scenario_path(&self) -> &'static str {
        "/v1beta/accountSummaries"
    }
}

/// Der Mock für `mocks::registry`.
pub fn mock() -> Option<Arc<dyn MockApi>> {
    Some(Arc::new(Ga4Mock::default()))
}

fn error(status: StatusCode, message: &str, code: &str) -> Response {
    (
        status,
        Json(json!({"error": {"code": status.as_u16(), "message": message, "status": code}})),
    )
        .into_response()
}

struct Inflight<'a>(&'a Ga4State);
impl Drop for Inflight<'_> {
    fn drop(&mut self) {
        self.0.inflight.fetch_sub(1, Ordering::SeqCst);
    }
}

async fn handle(
    axum::extract::State(s): axum::extract::State<Arc<Ga4State>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let path = uri.path().to_string();
    s.log.record(&path);
    let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    s.requests
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .push(Ga4Request {
            method: method.to_string(),
            path: path.clone(),
            body: body.clone(),
            at: tokio::time::Instant::now(),
        });
    if headers
        .get("Authorization")
        .is_some_and(|v| v == "Bearer bad")
    {
        return error(
            StatusCode::UNAUTHORIZED,
            "Request had invalid authentication credentials.",
            "UNAUTHENTICATED",
        );
    }
    let cfg = s.config.lock().unwrap_or_else(|p| p.into_inner()).clone();
    let scenario = headers
        .get("X-Scenario")
        .and_then(|v| v.to_str().ok())
        .map_or(cfg.scenario.clone(), str::to_string);
    let Some(rest) = path.strip_prefix("/v1beta/") else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if method == Method::GET && rest == "accountSummaries" {
        return account_summaries(&scenario).await;
    }
    let Some(rest) = rest.strip_prefix("properties/") else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if method == Method::POST {
        let Some((id, action)) = rest.split_once(':') else {
            return StatusCode::NOT_FOUND.into_response();
        };
        return match action {
            "runReport" => {
                let now = s.inflight.fetch_add(1, Ordering::SeqCst) + 1;
                s.max_inflight.fetch_max(now, Ordering::SeqCst);
                let _guard = Inflight(&s);
                if !cfg.delay.is_zero() {
                    tokio::time::sleep(cfg.delay).await;
                }
                run_report(&s, &cfg, &scenario, id, &body)
            }
            "checkCompatibility" if cfg.compatibility_down => error(
                StatusCode::SERVICE_UNAVAILABLE,
                "The service is currently unavailable.",
                "UNAVAILABLE",
            ),
            "checkCompatibility" => check_compatibility(&body),
            _ => StatusCode::NOT_FOUND.into_response(),
        };
    }
    match rest.split_once('/') {
        None => property(rest),
        Some((id, "dataStreams")) => data_streams(id),
        Some((_, "metadata")) => Json(metadata(rest)).into_response(),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn account_summaries(scenario: &str) -> Response {
    match scenario {
        "empty" => return Json(json!({})).into_response(),
        "error" | "4xx" => {
            return error(
                StatusCode::UNAUTHORIZED,
                "Request had invalid authentication credentials.",
                "UNAUTHENTICATED",
            )
        }
        _ => {}
    }
    if let Some(response) = scenario_response(scenario, 0).await {
        return response;
    }
    let mut accounts = vec![
        json!({
            "name": "accountSummaries/1001", "account": "accounts/1001", "displayName": "Integ Account A",
            "propertySummaries": [
                {"property": "properties/2001", "displayName": "Integ Shop", "propertyType": "PROPERTY_TYPE_ORDINARY", "parent": "accounts/1001"},
                {"property": "properties/2002", "displayName": "Integ Staging", "propertyType": "PROPERTY_TYPE_ORDINARY", "parent": "accounts/1001"}
            ]
        }),
        json!({
            "name": "accountSummaries/1002", "account": "accounts/1002", "displayName": "Integ Account B",
            "propertySummaries": [
                {"property": "properties/2003", "displayName": "Other Site", "propertyType": "PROPERTY_TYPE_ORDINARY", "parent": "accounts/1002"}
            ]
        }),
    ];
    if scenario == "no-properties" {
        accounts.push(json!({
            "name": "accountSummaries/1003", "account": "accounts/1003", "displayName": "Empty Account"
        }));
    }
    Json(json!({ "accountSummaries": accounts })).into_response()
}

fn property(id: &str) -> Response {
    Json(json!({
        "name": format!("properties/{id}"),
        "displayName": "Integ Shop",
        "timeZone": "Europe/Berlin",
        "currencyCode": "EUR"
    }))
    .into_response()
}

fn data_streams(id: &str) -> Response {
    if id != "2001" {
        return Json(json!({})).into_response();
    }
    Json(json!({"dataStreams": [
        {"name": "properties/2001/dataStreams/3001", "type": "WEB_DATA_STREAM", "displayName": "Integ Web",
         "webStreamData": {"measurementId": "G-FIXTURE1", "defaultUri": "https://integ.test"}},
        {"name": "properties/2001/dataStreams/3002", "type": "WEB_DATA_STREAM", "displayName": "Integ Blog",
         "webStreamData": {"measurementId": "G-FIXTURE2", "defaultUri": "https://blog.integ.test"}}
    ]}))
    .into_response()
}

fn catalog() -> Value {
    serde_json::from_str(CATALOG_JSON).expect("GA4-Katalog")
}

fn metric_type(kind: &str) -> &'static str {
    match kind {
        "integer" => "TYPE_INTEGER",
        "currency" => "TYPE_CURRENCY",
        "seconds" => "TYPE_SECONDS",
        _ => "TYPE_FLOAT",
    }
}

/// Bekannte Metriken des Mocks: API-Name → Typ.
fn known_metrics() -> BTreeMap<String, &'static str> {
    let mut out: BTreeMap<String, &'static str> = catalog()["metrics"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|m| {
            (
                m["api"].as_str().unwrap_or_default().to_string(),
                metric_type(m["kind"].as_str().unwrap_or_default()),
            )
        })
        .collect();
    out.insert(CUSTOM_METRIC.0.to_string(), "TYPE_FLOAT");
    out
}

fn known_dimensions() -> BTreeSet<String> {
    catalog()["dimensions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|d| d["api"].as_str().map(str::to_string))
        .collect()
}

fn metadata(rest: &str) -> Value {
    let cat = catalog();
    let mut metrics: Vec<Value> = cat["metrics"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|m| {
            json!({"apiName": m["api"], "uiName": m["name"], "category": m["category"],
                   "type": metric_type(m["kind"].as_str().unwrap_or_default()), "customDefinition": false})
        })
        .collect();
    metrics.push(
        json!({"apiName": CUSTOM_METRIC.0, "uiName": CUSTOM_METRIC.1, "category": "Custom",
                        "type": "TYPE_FLOAT", "customDefinition": true}),
    );
    let dimensions: Vec<Value> = cat["dimensions"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|d| json!({"apiName": d["api"], "uiName": d["name"], "category": d["category"], "customDefinition": false}))
        .collect();
    json!({"name": format!("properties/{rest}"), "dimensions": dimensions, "metrics": metrics})
}

/// Feldnamen eines Filterausdrucks.
fn filter_fields(expr: &Value, out: &mut BTreeSet<String>) {
    if let Some(f) = expr.pointer("/filter/fieldName").and_then(Value::as_str) {
        out.insert(f.to_string());
    }
    for key in ["andGroup", "orGroup"] {
        for e in expr
            .pointer(&format!("/{key}/expressions"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            filter_fields(e, out);
        }
    }
    if let Some(e) = expr.get("notExpression") {
        filter_fields(e, out);
    }
}

fn names(body: &Value, key: &str) -> Vec<String> {
    body[key]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|d| d["name"].as_str().map(str::to_string))
        .collect()
}

/// Kompatibilitätsregeln der Fixture (INTEG-101): `(Metrik, Dimension)` – die Metrik ist mit der Dimension nicht
/// kombinierbar, ob diese Gruppierungs- oder Filterdimension ist; die zweite Regel gilt nur als Filterdimension.
fn incompatible(
    metrics: &[String],
    dims: &[String],
    filter_dims: &BTreeSet<String>,
) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let all_dims: BTreeSet<&String> = dims.iter().chain(filter_dims.iter()).collect();
    for m in metrics {
        if m == "advertiserAdCost" && all_dims.iter().any(|d| *d == "pageLocation") {
            out.push((m.clone(), "pageLocation".to_string()));
        }
        if m == "itemRevenue" && filter_dims.contains("firstUserSource") {
            out.push((m.clone(), "firstUserSource".to_string()));
        }
    }
    out
}

fn check_compatibility(body: &Value) -> Response {
    let metrics = names(body, "metrics");
    let dims = names(body, "dimensions");
    let mut filter_dims = BTreeSet::new();
    if let Some(f) = body.get("dimensionFilter") {
        filter_fields(f, &mut filter_dims);
    }
    let all: Vec<String> = known_metrics().into_keys().collect();
    let bad = incompatible(&all, &dims, &filter_dims);
    let metric_rows: Vec<Value> = known_metrics()
        .into_keys()
        .map(|m| {
            let c = if bad.iter().any(|(b, _)| *b == m) {
                "INCOMPATIBLE"
            } else {
                "COMPATIBLE"
            };
            json!({"metricMetadata": {"apiName": m}, "compatibility": c})
        })
        .collect();
    let selected_bad = incompatible(&metrics, &[], &BTreeSet::new());
    let dim_rows: Vec<Value> = known_dimensions()
        .into_iter()
        .map(|d| {
            let c = if incompatible(&metrics, std::slice::from_ref(&d), &BTreeSet::new()).len()
                > selected_bad.len()
            {
                "INCOMPATIBLE"
            } else {
                "COMPATIBLE"
            };
            json!({"dimensionMetadata": {"apiName": d}, "compatibility": c})
        })
        .collect();
    Json(json!({"dimensionCompatibilities": dim_rows, "metricCompatibilities": metric_rows}))
        .into_response()
}

fn eval_filter(expr: &Value, row: &MockRow) -> bool {
    if let Some(group) = expr
        .pointer("/andGroup/expressions")
        .and_then(Value::as_array)
    {
        return group.iter().all(|e| eval_filter(e, row));
    }
    if let Some(group) = expr
        .pointer("/orGroup/expressions")
        .and_then(Value::as_array)
    {
        return group.iter().any(|e| eval_filter(e, row));
    }
    if let Some(e) = expr.get("notExpression") {
        return !eval_filter(e, row);
    }
    let Some(filter) = expr.get("filter") else {
        return true;
    };
    let field = filter["fieldName"].as_str().unwrap_or_default();
    let value = row
        .dims
        .get(field)
        .cloned()
        .unwrap_or_else(|| "(not set)".to_string());
    if let Some(list) = filter
        .pointer("/inListFilter/values")
        .and_then(Value::as_array)
    {
        return list.iter().any(|v| v.as_str() == Some(value.as_str()));
    }
    let Some(string_filter) = filter.get("stringFilter") else {
        return true;
    };
    let case = string_filter["caseSensitive"].as_bool().unwrap_or(false);
    let (value, wanted) = if case {
        (
            value,
            string_filter["value"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
        )
    } else {
        (
            value.to_lowercase(),
            string_filter["value"]
                .as_str()
                .unwrap_or_default()
                .to_lowercase(),
        )
    };
    match string_filter["matchType"].as_str().unwrap_or("EXACT") {
        "EXACT" => value == wanted,
        "BEGINS_WITH" => value.starts_with(&wanted),
        "ENDS_WITH" => value.ends_with(&wanted),
        "CONTAINS" => value.contains(&wanted),
        // Regex-Filter werten erst die echten Anbieter aus; der Mock prüft nur den Request-Body.
        _ => true,
    }
}

fn quota_json(q: QuotaStatus) -> Value {
    json!({"consumed": q.consumed, "remaining": q.remaining})
}

fn run_report(s: &Ga4State, cfg: &MockConfig, scenario: &str, id: &str, body: &Value) -> Response {
    match scenario {
        "400-invalid-date-range" => {
            return error(
                StatusCode::BAD_REQUEST,
                "Invalid date range: the requested start date is not supported for this property.",
                "INVALID_ARGUMENT",
            )
        }
        "resource-exhausted" => {
            return error(
                StatusCode::TOO_MANY_REQUESTS,
                "Exhausted property tokens per hour for a project per property.",
                "RESOURCE_EXHAUSTED",
            )
        }
        _ => {}
    }
    let metrics = names(body, "metrics");
    let dims = names(body, "dimensions");
    let known = known_metrics();
    if metrics.len() > 10 {
        return error(
            StatusCode::BAD_REQUEST,
            "Requests are limited to 10 metrics.",
            "INVALID_ARGUMENT",
        );
    }
    for m in &metrics {
        if !known.contains_key(m) {
            return error(
                StatusCode::BAD_REQUEST,
                &format!("Field {m} is not a valid metric. For a list of valid dimensions and metrics, see the API schema."),
                "INVALID_ARGUMENT",
            );
        }
    }
    let known_dims = known_dimensions();
    for d in &dims {
        if !known_dims.contains(d) {
            return error(
                StatusCode::BAD_REQUEST,
                &format!("Field {d} is not a valid dimension."),
                "INVALID_ARGUMENT",
            );
        }
    }
    let mut filter_dims = BTreeSet::new();
    if let Some(f) = body.get("dimensionFilter") {
        filter_fields(f, &mut filter_dims);
    }
    if let Some((m, _)) = incompatible(&metrics, &dims, &filter_dims).first() {
        return error(
            StatusCode::BAD_REQUEST,
            &format!("Please remove {m} to make the request compatible. The request's dimensions & metrics are incompatible."),
            "INVALID_ARGUMENT",
        );
    }
    let limit = body["limit"]
        .as_str()
        .and_then(|v| v.parse().ok())
        .or_else(|| body["limit"].as_u64())
        .unwrap_or(10_000)
        .min(250_000) as usize;
    let offset = body["offset"]
        .as_str()
        .and_then(|v| v.parse().ok())
        .or_else(|| body["offset"].as_u64())
        .unwrap_or(0) as usize;
    let (row_count, page): (usize, Vec<(Vec<String>, Vec<f64>)>) = if let Some(syn) = cfg.synthetic
    {
        let end = (offset + limit.min(syn.page_cap.unwrap_or(usize::MAX))).min(syn.rows);
        let page = (offset.min(end)..end)
            .map(|i| {
                let url = format!("https://{STREAM_HOST}/p/{i}");
                let row = MockRow::sessions(&url, (syn.rows - i) as f64);
                project(&row, &dims, &metrics)
            })
            .collect();
        (syn.rows, page)
    } else {
        let mut grouped: BTreeMap<Vec<String>, Vec<f64>> = BTreeMap::new();
        for row in &cfg.rows {
            if let Some(f) = body.get("dimensionFilter") {
                if !eval_filter(f, row) {
                    continue;
                }
            }
            let (key, values) = project(row, &dims, &metrics);
            // Wie die Data API: Zeilen, deren angefragte Metriken alle 0 sind, fehlen ohne `keepEmptyRows`.
            if body["keepEmptyRows"].as_bool() != Some(true) && values.iter().all(|v| *v == 0.0) {
                continue;
            }
            let entry = grouped
                .entry(key)
                .or_insert_with(|| vec![0.0; metrics.len()]);
            for (slot, v) in entry.iter_mut().zip(values) {
                *slot += v;
            }
        }
        let mut all: Vec<(Vec<String>, Vec<f64>)> = grouped.into_iter().collect();
        if let Some(order) = body["orderBys"]
            .as_array()
            .and_then(|o| o.first())
            .and_then(|o| o.pointer("/metric/metricName"))
            .and_then(Value::as_str)
        {
            if let Some(i) = metrics.iter().position(|m| m == order) {
                all.sort_by(|a, b| b.1[i].total_cmp(&a.1[i]).then(a.0.cmp(&b.0)));
            }
        }
        let count = all.len();
        (count, all.into_iter().skip(offset).take(limit).collect())
    };
    let quota = {
        let mut guard = s.config.lock().unwrap_or_else(|p| p.into_inner());
        let c: &mut MockConfig = &mut guard;
        let per_report = c.tokens_per_report;
        for q in [&mut c.tokens_per_hour, &mut c.tokens_per_day] {
            let used = per_report.min(q.remaining);
            q.consumed += used;
            q.remaining -= used;
        }
        json!({
            "tokensPerDay": quota_json(c.tokens_per_day),
            "tokensPerHour": quota_json(c.tokens_per_hour),
            "concurrentRequests": {"consumed": 0, "remaining": 10},
            "serverErrorsPerProjectPerHour": {"consumed": 0, "remaining": 10},
            "potentiallyThresholdedRequestsPerHour": {"consumed": 0, "remaining": 120},
            "tokensPerProjectPerHour": quota_json(c.tokens_per_hour)
        })
    };
    let rows: Vec<Value> = page
        .into_iter()
        .map(|(d, m)| {
            json!({
                "dimensionValues": d.into_iter().map(|v| json!({"value": v})).collect::<Vec<_>>(),
                "metricValues": m.into_iter().map(|v| json!({"value": format_value(v)})).collect::<Vec<_>>()
            })
        })
        .collect();
    let mut out = json!({
        "dimensionHeaders": dims.iter().map(|d| json!({"name": d})).collect::<Vec<_>>(),
        "metricHeaders": metrics.iter().map(|m| json!({"name": m, "type": known[m]})).collect::<Vec<_>>(),
        "metadata": {
            "currencyCode": "EUR",
            "timeZone": "Europe/Berlin",
            "dataLossFromOtherRow": cfg.data_loss_from_other_row,
            "subjectToThresholding": cfg.thresholding
        },
        "kind": "analyticsData#runReport"
    });
    if scenario != "empty" && row_count > 0 {
        out["rows"] = Value::Array(rows);
        out["rowCount"] = json!(row_count);
    }
    if body["returnPropertyQuota"].as_bool() == Some(true) {
        out["propertyQuota"] = quota;
    }
    let _ = id;
    Json(out).into_response()
}

fn project(row: &MockRow, dims: &[String], metrics: &[String]) -> (Vec<String>, Vec<f64>) {
    (
        dims.iter()
            .map(|d| {
                row.dims
                    .get(d)
                    .cloned()
                    .unwrap_or_else(|| "(not set)".to_string())
            })
            .collect(),
        metrics
            .iter()
            .map(|m| row.metrics.get(m).copied().unwrap_or(0.0))
            .collect(),
    )
}

fn format_value(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Infrastruktur (INTEG-019): Standard-Datensatz und Katalog sind in sich stimmig.
    #[test]
    fn default_rows_cover_site_matching_and_special_rows() {
        let rows = default_rows();
        assert!(rows
            .iter()
            .any(|r| r.dims["pageLocation"] == "https://fremd.test/x"));
        assert!(rows
            .iter()
            .any(|r| r.dims["landingPagePlusQueryString"] == "(not set)"));
        assert_eq!(split_url("https://integ.test/d?x=1").1, "/d?x=1");
        assert!(known_metrics().len() >= 66);
    }
}
