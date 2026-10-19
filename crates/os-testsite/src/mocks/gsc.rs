//! Mock: Google Search Console – `sites.list`, `searchanalytics.query` und URL Inspection (INTEG-110 ff., INTEG-019).
//!
//! Routen (unter `/gsc`):
//! - `GET /webmasters/v3/sites` – Property-Liste (Standard: vier Properties, eine davon unverifiziert),
//! - `POST /webmasters/v3/sites/{site}/searchAnalytics/query` – Zeilen mit `rowLimit`/`startRow`, bei Discover und
//!   Google News ohne `position`,
//! - `POST /v1/urlInspection/index:inspect` – Antwort je URL (Standard: [`fixture`] `inspect-indexed`), Restkontingent
//!   je Property, 403 für URLs außerhalb der Property.
//!
//! Szenarien über Header `X-Scenario`, Query-Parameter `scenario` oder [`GscMock::set_scenario`]: `ok`, `empty`,
//! `error` (401), `throttle` (429), `429x2`, `delay-90s`, `delay-100s`, `delay-180s`, `paged-delay-100s`,
//! `page2-delay-100s` (nur Folgeseiten verzögert), `403-not-in-property`, `429-daily`. Jede Anfrage steht mit Zeitpunkt (tokio-Uhr, auch mit pausierter Testzeit
//! aussagekräftig) und Körper im Protokoll ([`GscMock::requests`]).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};

use super::common::{scenario_response, CallLog};
use super::MockApi;

/// Eine protokollierte Anfrage.
#[derive(Debug, Clone)]
pub struct Logged {
    /// Ankunft (tokio-Uhr).
    pub at: tokio::time::Instant,
    /// `sites`, `query` oder `inspect`.
    pub endpoint: &'static str,
    /// Property aus Pfad bzw. Körper.
    pub property: Option<String>,
    /// JSON-Körper (`Null` bei `GET`).
    pub body: Value,
}

#[derive(Default)]
struct MockState {
    scenario: Option<String>,
    sites: Option<Vec<(String, String)>>,
    rows: Vec<Value>,
    inspections: HashMap<String, Value>,
    remaining: HashMap<String, u64>,
    attempts: HashMap<String, usize>,
}

/// Der Search-Console-Mock; Klone teilen Zustand und Protokoll.
#[derive(Clone, Default)]
pub struct GscMock {
    state: Arc<Mutex<MockState>>,
    requests: Arc<Mutex<Vec<Logged>>>,
    /// Pfade wie beim gemeinsamen Mock-Protokoll.
    pub log: CallLog,
}

impl GscMock {
    /// Neuer Mock mit den Standarddaten.
    pub fn new() -> GscMock {
        GscMock::default()
    }

    fn with_state<T>(&self, f: impl FnOnce(&mut MockState) -> T) -> T {
        f(&mut self.state.lock().unwrap_or_else(|p| p.into_inner()))
    }

    /// Szenario für alle Anfragen ohne eigenen `X-Scenario`-Header.
    pub fn set_scenario(&self, scenario: Option<&str>) {
        self.with_state(|s| s.scenario = scenario.map(str::to_string));
    }

    /// Property-Liste als `(siteUrl, permissionLevel)`.
    pub fn set_sites(&self, sites: &[(&str, &str)]) {
        self.with_state(|s| {
            s.sites = Some(
                sites
                    .iter()
                    .map(|(a, b)| (a.to_string(), b.to_string()))
                    .collect(),
            )
        });
    }

    /// Search-Analytics-Zeilen `(page, clicks, impressions, ctr, position)`.
    pub fn set_rows(&self, rows: &[(&str, f64, f64, f64, f64)]) {
        self.set_raw_rows(
            rows.iter()
                .map(|(page, clicks, impressions, ctr, position)| {
                    json!({ "keys": [page], "clicks": clicks, "impressions": impressions, "ctr": ctr,
                            "position": position })
                })
                .collect(),
        );
    }

    /// Search-Analytics-Zeilen als JSON (Form der API).
    pub fn set_raw_rows(&self, rows: Vec<Value>) {
        self.with_state(|s| s.rows = rows);
    }

    /// `n` generierte Zeilen `https://integ.test/p/<i>` (für die Paginierung).
    pub fn generate_rows(&self, n: usize) {
        self.set_raw_rows(
            (0..n)
                .map(|i| {
                    json!({ "keys": [format!("https://integ.test/p/{i}")], "clicks": 1, "impressions": 10,
                            "ctr": 0.1, "position": 5.0 })
                })
                .collect(),
        );
    }

    /// Antwort der URL Inspection für eine URL (Form der API, z. B. aus [`fixture`]).
    pub fn set_inspection(&self, url: &str, response: Value) {
        self.with_state(|s| {
            s.inspections.insert(url.to_string(), response);
        });
    }

    /// Restkontingent der URL Inspection einer Property beim Anbieter; danach 429 mit Tageskontingent-Grund.
    pub fn set_remaining(&self, property: &str, remaining: u64) {
        self.with_state(|s| {
            s.remaining.insert(property.to_string(), remaining);
        });
    }

    /// Alle Anfragen in Ankunftsreihenfolge.
    pub fn requests(&self) -> Vec<Logged> {
        self.requests
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// Anfragen eines Endpunkts (`sites`, `query`, `inspect`).
    pub fn requests_to(&self, endpoint: &str) -> Vec<Logged> {
        self.requests()
            .into_iter()
            .filter(|r| r.endpoint == endpoint)
            .collect()
    }

    fn record(&self, endpoint: &'static str, property: Option<String>, body: Value) {
        self.log.record(endpoint);
        self.requests
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(Logged {
                at: tokio::time::Instant::now(),
                endpoint,
                property,
                body,
            });
    }

    fn scenario(&self, headers: &HeaderMap, query: &HashMap<String, String>) -> String {
        headers
            .get("X-Scenario")
            .and_then(|h| h.to_str().ok())
            .map(str::to_string)
            .or_else(|| query.get("scenario").cloned())
            .or_else(|| self.with_state(|s| s.scenario.clone()))
            .unwrap_or_else(|| "ok".into())
    }

    fn attempt(&self, scenario: &str) -> usize {
        self.with_state(|s| {
            let n = s.attempts.entry(scenario.to_string()).or_default();
            *n += 1;
            *n - 1
        })
    }
}

impl MockApi for GscMock {
    fn name(&self) -> &'static str {
        "gsc"
    }

    fn base_url_key(&self) -> Option<&'static str> {
        Some("api.gsc.base_url")
    }

    fn router(&self) -> Router {
        Router::new()
            .route("/webmasters/v3/sites", get(sites))
            .route(
                "/webmasters/v3/sites/{site}/searchAnalytics/query",
                post(query),
            )
            .route("/v1/urlInspection/index:inspect", post(inspect))
            .with_state(self.clone())
    }

    fn scenario_path(&self) -> &'static str {
        "/webmasters/v3/sites"
    }
}

/// Der Mock für die Registrierung in `mocks::registry`.
pub fn mock() -> Option<Arc<dyn MockApi>> {
    Some(Arc::new(GscMock::new()))
}

fn unauthorized(headers: &HeaderMap) -> Option<Response> {
    headers
        .get("Authorization")
        .is_some_and(|v| v == "Bearer bad")
        .then(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": {"code": 401, "message": "Invalid Credentials", "status": "UNAUTHENTICATED"}})),
            )
                .into_response()
        })
}

/// Verzögerung eines Szenarios `delay-<n>s` bzw. `paged-delay-<n>s`.
fn delay_of(scenario: &str) -> Option<Duration> {
    let secs = scenario
        .strip_prefix("paged-delay-")
        .or_else(|| scenario.strip_prefix("delay-"))?
        .strip_suffix('s')?
        .parse::<u64>()
        .ok()?;
    Some(Duration::from_secs(secs))
}

/// Gemeinsame Szenarien (Fehler, Drosselung, Verzögerung); `None` = normal antworten.
async fn common(mock: &GscMock, scenario: &str) -> Option<Response> {
    if let Some(delay) = delay_of(scenario) {
        tokio::time::sleep(delay).await;
        return None;
    }
    if scenario == "empty" {
        return None;
    }
    let attempt = mock.attempt(scenario);
    scenario_response(scenario, attempt).await
}

async fn sites(
    State(mock): State<GscMock>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    mock.record("sites", None, Value::Null);
    if let Some(r) = unauthorized(&headers) {
        return r;
    }
    let scenario = mock.scenario(&headers, &q);
    if let Some(r) = common(&mock, &scenario).await {
        return r;
    }
    if scenario == "empty" {
        return Json(json!({})).into_response();
    }
    let sites = mock.with_state(|s| s.sites.clone()).unwrap_or_else(|| {
        [
            ("sc-domain:integ.test", "siteOwner"),
            ("https://integ.test/", "siteFullUser"),
            ("https://integ.test/blog/", "siteRestrictedUser"),
            ("https://unverified.test/", "siteUnverifiedUser"),
        ]
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect()
    });
    let entries: Vec<Value> = sites
        .iter()
        .map(|(url, level)| json!({ "siteUrl": url, "permissionLevel": level }))
        .collect();
    Json(json!({ "siteEntry": entries })).into_response()
}

async fn query(
    State(mock): State<GscMock>,
    Path(site): Path<String>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
    Json(body): Json<Value>,
) -> Response {
    mock.record("query", Some(site), body.clone());
    if let Some(r) = unauthorized(&headers) {
        return r;
    }
    let scenario = mock.scenario(&headers, &q);
    let start = body["startRow"].as_u64().unwrap_or(0) as usize;
    // `page2-delay-<n>s`: nur Folgeseiten der Paginierung sind verzögert.
    if let Some(rest) = scenario.strip_prefix("page2-") {
        if start > 0 {
            if let Some(delay) = delay_of(rest) {
                tokio::time::sleep(delay).await;
            }
        }
    } else if let Some(r) = common(&mock, &scenario).await {
        return r;
    }
    if scenario == "empty" {
        return Json(json!({ "responseAggregationType": "byPage" })).into_response();
    }
    let limit = body["rowLimit"].as_u64().unwrap_or(1000) as usize;
    let no_position = matches!(body["type"].as_str(), Some("discover" | "googleNews"));
    let rows: Vec<Value> = mock.with_state(|s| {
        s.rows
            .iter()
            .skip(start)
            .take(limit)
            .cloned()
            .map(|mut r| {
                if no_position {
                    if let Some(o) = r.as_object_mut() {
                        o.remove("position");
                    }
                }
                r
            })
            .collect()
    });
    if rows.is_empty() {
        return Json(json!({ "responseAggregationType": "byPage" })).into_response();
    }
    Json(json!({ "rows": rows, "responseAggregationType": "byPage" })).into_response()
}

fn error(status: StatusCode, code: u16, api_status: &str, message: &str, reason: &str) -> Response {
    (
        status,
        Json(
            json!({ "error": { "code": code, "message": message, "status": api_status,
                                 "errors": [{ "reason": reason, "message": message }] } }),
        ),
    )
        .into_response()
}

fn not_in_property() -> Response {
    error(
        StatusCode::FORBIDDEN,
        403,
        "PERMISSION_DENIED",
        "The inspected URL is not part of this property.",
        "forbidden",
    )
}

fn daily_quota() -> Response {
    error(
        StatusCode::TOO_MANY_REQUESTS,
        429,
        "RESOURCE_EXHAUSTED",
        "Quota exceeded for quota metric 'URL inspection' per day.",
        "quotaExceeded",
    )
}

/// Liegt die URL in der Property (Domain- oder URL-Präfix-Property)?
fn in_property(url: &str, property: &str) -> bool {
    match property.strip_prefix("sc-domain:") {
        Some(domain) => {
            let host = url
                .split("://")
                .nth(1)
                .and_then(|rest| rest.split(['/', ':', '?', '#']).next())
                .unwrap_or_default();
            host == domain || host.ends_with(&format!(".{domain}"))
        }
        None => url.starts_with(property),
    }
}

async fn inspect(
    State(mock): State<GscMock>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
    Json(body): Json<Value>,
) -> Response {
    let property = body["siteUrl"].as_str().unwrap_or_default().to_string();
    let url = body["inspectionUrl"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    mock.record("inspect", Some(property.clone()), body.clone());
    if let Some(r) = unauthorized(&headers) {
        return r;
    }
    let scenario = mock.scenario(&headers, &q);
    match scenario.as_str() {
        "403-not-in-property" => return not_in_property(),
        "429-daily" => return daily_quota(),
        _ => {}
    }
    if let Some(r) = common(&mock, &scenario).await {
        return r;
    }
    if !in_property(&url, &property) {
        return not_in_property();
    }
    let exhausted = mock.with_state(|s| match s.remaining.get_mut(&property) {
        Some(0) => true,
        Some(n) => {
            *n -= 1;
            false
        }
        None => false,
    });
    if exhausted {
        return daily_quota();
    }
    let response = mock
        .with_state(|s| s.inspections.get(&url).cloned())
        .unwrap_or_else(|| fixture("inspect-indexed").expect("Standardantwort"));
    Json(with_url(response, &url)).into_response()
}

/// Ersetzt den Platzhalter `{url}` in einer Fixture-Antwort.
fn with_url(response: Value, url: &str) -> Value {
    serde_json::from_str(&response.to_string().replace("{url}", url)).unwrap_or(response)
}

/// Aufgezeichnete, anonymisierte Inspection-Antworten (INTEG-123): `inspect-indexed`, `inspect-not-indexed`,
/// `inspect-canonical-mismatch`, `inspect-rich-error`, `inspect-amp-fail`, `inspect-mobile-fail`, `inspect-minimal`.
/// `{url}` steht für die geprüfte URL.
pub fn fixture(name: &str) -> Option<Value> {
    let index_pass = json!({
        "verdict": "PASS",
        "coverageState": "Submitted and indexed",
        "robotsTxtState": "ALLOWED",
        "indexingState": "INDEXING_ALLOWED",
        "lastCrawlTime": "2026-10-10T08:15:30Z",
        "pageFetchState": "SUCCESSFUL",
        "googleCanonical": "{url}",
        "userCanonical": "{url}",
        "crawledAs": "MOBILE",
        "sitemap": ["https://integ.test/sitemap.xml"],
        "referringUrls": ["https://integ.test/"]
    });
    let mobile_pass = json!({ "verdict": "PASS" });
    let rich_pass = json!({
        "verdict": "PASS",
        "detectedItems": [{ "richResultType": "Breadcrumbs", "items": [{ "name": "Unnamed item", "issues": [] }] }]
    });
    let link = json!("https://search.google.com/search-console/inspect?resource_id=x&id=y");
    let result = match name {
        "inspect-indexed" => json!({
            "inspectionResultLink": link,
            "indexStatusResult": index_pass,
            "mobileUsabilityResult": mobile_pass,
            "richResultsResult": rich_pass
        }),
        "inspect-not-indexed" => json!({
            "inspectionResultLink": link,
            "indexStatusResult": {
                "verdict": "NEUTRAL",
                "coverageState": "Crawled - currently not indexed",
                "robotsTxtState": "ALLOWED",
                "indexingState": "INDEXING_ALLOWED",
                "lastCrawlTime": "2026-09-30T23:01:02Z",
                "pageFetchState": "SUCCESSFUL",
                "crawledAs": "DESKTOP"
            }
        }),
        "inspect-canonical-mismatch" => json!({
            "inspectionResultLink": link,
            "indexStatusResult": {
                "verdict": "NEUTRAL",
                "coverageState": "Duplicate, Google chose different canonical than user",
                "robotsTxtState": "ALLOWED",
                "indexingState": "INDEXING_ALLOWED",
                "lastCrawlTime": "2026-10-01T10:00:00Z",
                "pageFetchState": "SUCCESSFUL",
                "userCanonical": "{url}",
                "googleCanonical": "https://integ.test/original",
                "crawledAs": "MOBILE"
            }
        }),
        "inspect-rich-error" => json!({
            "inspectionResultLink": link,
            "indexStatusResult": index_pass,
            "richResultsResult": {
                "verdict": "FAIL",
                "detectedItems": [
                    { "richResultType": "Product snippets", "items": [{ "name": "Shoe", "issues": [
                        { "issueMessage": "Missing field \"name\"", "severity": "ERROR" },
                        { "issueMessage": "Either \"offers\", \"review\", or \"aggregateRating\" should be specified",
                          "severity": "ERROR" }
                    ] }] },
                    { "richResultType": "Breadcrumbs", "items": [{ "name": "Unnamed item", "issues": [
                        { "issueMessage": "Missing field \"item\" (optional)", "severity": "WARNING" }
                    ] }] },
                    { "richResultType": "FAQ", "items": [{ "name": "Unnamed item", "issues": [] }] }
                ]
            }
        }),
        "inspect-amp-fail" => json!({
            "inspectionResultLink": link,
            "indexStatusResult": index_pass,
            "ampResult": {
                "verdict": "FAIL",
                "ampUrl": "https://integ.test/amp/page",
                "issues": [
                    { "issueMessage": "The attribute 'onclick' may not appear in tag 'div'.", "severity": "ERROR" },
                    { "issueMessage": "Custom JavaScript is not allowed.", "severity": "ERROR" }
                ]
            }
        }),
        "inspect-mobile-fail" => json!({
            "inspectionResultLink": link,
            "indexStatusResult": index_pass,
            "mobileUsabilityResult": {
                "verdict": "FAIL",
                "issues": [
                    { "issueType": "USES_INCOMPATIBLE_PLUGINS", "severity": "ERROR", "message": "Plugins" },
                    { "issueType": "TEXT_TOO_SMALL", "severity": "ERROR", "message": "Text too small" }
                ]
            }
        }),
        "inspect-minimal" => json!({
            "inspectionResultLink": link,
            "indexStatusResult": { "verdict": "NEUTRAL", "coverageState": "URL is unknown to Google" }
        }),
        _ => return None,
    };
    Some(json!({ "inspectionResult": result }))
}
