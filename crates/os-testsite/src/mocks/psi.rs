//! Mock: PageSpeed Insights (INTEG-130 ff., M2-I04).
//!
//! Endpunkt `GET /psi/pagespeedonline/v5/runPagespeed?url=…&key=…&strategy=mobile|desktop&category=performance`.
//! Die Antwort wählt das Szenario aus (Vorrang in dieser Reihenfolge): Header `X-Scenario`, Parameter `scenario`,
//! API-Schlüssel (`key-not-enabled`, `key-invalid`) und schließlich der letzte Pfadabschnitt der abgefragten URL
//! (`https://site.test/heavy` → `heavy`). So kann ein Test je URL ein anderes Verhalten auslösen.
//!
//! | Szenario | Antwort |
//! |---|---|
//! | `ok`, `fast`, sonst | 200, [`psi_fast`] |
//! | `heavy` | 200, [`psi_heavy`] (löst alle 19 PageSpeed-Filter aus) |
//! | `legacy-cache` | 200, [`psi_legacy_cache`] (ältere Lighthouse-Version mit `uses-long-cache-ttl`) |
//! | `crux-full`, `crux-origin-only`, `crux-fallback`, `crux-none` | 200, [`psi_fast`] mit den CrUX-Varianten |
//! | `key-not-enabled` | 403 `SERVICE_DISABLED` |
//! | `key-invalid` | 400 `API_KEY_INVALID` |
//! | `500-once` | erste Anfrage je URL 500, danach 200 |
//! | `500-burst` | immer 500 |
//! | `lh-errored-document` | 200 mit `lighthouseResult.runtimeError.code = ERRORED_DOCUMENT_REQUEST` |
//! | `quota-exhausted` | 429 `RESOURCE_EXHAUSTED` (Restkontingent 0) |
//! | `empty`, `error`, `throttle`, … | gemeinsame Szenarien aus [`super::common::scenario_response`] |
//!
//! Die Antworten sind eigene, synthetische Lighthouse-13-Strukturen (Feldnamen wie im öffentlichen Lighthouse-Format)
//! mit bekannten Zahlen; die erwarteten Spaltenwerte stehen als Golden in [`expected_heavy`], [`expected_fast`] und
//! [`expected_crux`] (Ersatz für `expected-*.csv`, weil das Paket keinen Fixture-Ordner besitzt).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};

use super::common::{scenario_response, CallLog};
use super::MockApi;

/// Pfad des Endpunkts (relativ zu `/psi`).
pub const RUN_PATH: &str = "/pagespeedonline/v5/runPagespeed";

/// Eine aufgezeichnete Anfrage (ohne Schlüssel).
#[derive(Debug, Clone)]
pub struct PsiCall {
    /// Abgefragte URL (`url`).
    pub url: String,
    /// `strategy` (leer, wenn nicht gesetzt).
    pub strategy: String,
    /// Wirksames Szenario.
    pub scenario: String,
    /// Ankunftszeit.
    pub at: Instant,
}

/// Der PSI-Mock. `log` und `calls` sind zwischen Klonen geteilt.
#[derive(Clone, Default)]
pub struct PsiMock {
    /// Pfade wie bei den übrigen Mocks.
    pub log: CallLog,
    calls: Arc<Mutex<Vec<PsiCall>>>,
    attempts: Arc<Mutex<HashMap<String, usize>>>,
}

impl PsiMock {
    /// Alle bisherigen Anfragen in Ankunftsreihenfolge.
    pub fn calls(&self) -> Vec<PsiCall> {
        self.calls.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
}

impl MockApi for PsiMock {
    fn name(&self) -> &'static str {
        "psi"
    }

    fn base_url_key(&self) -> Option<&'static str> {
        Some("api.psi.base_url")
    }

    fn router(&self) -> Router {
        Router::new()
            .route(RUN_PATH, get(run))
            .with_state(self.clone())
    }

    fn scenario_path(&self) -> &'static str {
        RUN_PATH
    }
}

/// Der Mock für die Registrierung in `mocks::registry`.
pub fn mock() -> Option<Arc<dyn MockApi>> {
    Some(Arc::new(PsiMock::default()))
}

fn scenario_of(headers: &HeaderMap, q: &HashMap<String, String>) -> String {
    if let Some(s) = headers.get("X-Scenario").and_then(|h| h.to_str().ok()) {
        return s.to_string();
    }
    if let Some(s) = q.get("scenario") {
        return s.clone();
    }
    if let Some(key) = q.get("key").filter(|k| k.starts_with("key-")) {
        return key.clone();
    }
    q.get("url")
        .and_then(|u| u.trim_end_matches('/').rsplit('/').next())
        .map(|s| s.trim_end_matches(".html").to_string())
        .unwrap_or_else(|| "ok".into())
}

async fn run(
    State(s): State<PsiMock>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    s.log.record(RUN_PATH);
    let scenario = scenario_of(&headers, &q);
    let url = q.get("url").cloned().unwrap_or_default();
    let strategy = q.get("strategy").cloned().unwrap_or_default();
    s.calls
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .push(PsiCall {
            url: url.clone(),
            strategy: strategy.clone(),
            scenario: scenario.clone(),
            at: Instant::now(),
        });
    let attempt = {
        let mut map = s.attempts.lock().unwrap_or_else(|p| p.into_inner());
        let n = map.entry(format!("{scenario}\u{0}{url}")).or_default();
        let value = *n;
        *n += 1;
        value
    };
    if let Some(response) = scenario_response(&scenario, attempt).await {
        return response;
    }
    let desktop = strategy == "desktop";
    match scenario.as_str() {
        "key-not-enabled" => google_error(
            StatusCode::FORBIDDEN,
            "PERMISSION_DENIED",
            "SERVICE_DISABLED",
            "The PageSpeed Insights API is not enabled for this project.",
        ),
        "key-invalid" => google_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "API_KEY_INVALID",
            "The API key is not valid.",
        ),
        "500-once" if attempt == 0 => server_error(),
        "500-burst" => server_error(),
        "quota-exhausted" => google_error(
            StatusCode::TOO_MANY_REQUESTS,
            "RESOURCE_EXHAUSTED",
            "RATE_LIMIT_EXCEEDED",
            "Daily request quota of the project is exhausted (remaining: 0).",
        ),
        "lh-errored-document" => Json(errored_document(&url, desktop)).into_response(),
        "heavy" => Json(psi_heavy(&url, desktop)).into_response(),
        "legacy-cache" => Json(psi_legacy_cache(&url, desktop)).into_response(),
        "crux-full" | "crux-origin-only" | "crux-fallback" | "crux-none" => {
            let mut body = psi_fast(&url, desktop);
            set_crux(&mut body, &scenario, desktop);
            Json(body).into_response()
        }
        _ => Json(psi_fast(&url, desktop)).into_response(),
    }
}

fn google_error(code: StatusCode, status: &str, reason: &str, message: &str) -> Response {
    (
        code,
        Json(json!({
            "error": {
                "code": code.as_u16(),
                "message": message,
                "status": status,
                "errors": [{"reason": if code == StatusCode::TOO_MANY_REQUESTS { "rateLimitExceeded" } else { "forbidden" }}],
                "details": [{
                    "@type": "type.googleapis.com/google.rpc.ErrorInfo",
                    "reason": reason,
                    "domain": "googleapis.com"
                }]
            }
        })),
    )
        .into_response()
}

fn server_error() -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": {"code": 500, "message": "Unable to process request. Please wait a while and try again.", "status": "INTERNAL"}})),
    )
        .into_response()
}

// ---------------------------------------------------------------------------------------------------------------------
// Antwortbausteine

fn audit(id: &str, score: Value, mode: &str) -> Value {
    json!({"id": id, "title": id, "score": score, "scoreDisplayMode": mode})
}

fn numeric(id: &str, score: f64, value: f64) -> Value {
    let mut a = audit(id, json!(score), "numeric");
    a["numericValue"] = json!(value);
    a
}

/// Insight-Audit mit Einsparangaben (`metricSavings` und `details.overallSavings*`).
fn insight(id: &str, score: f64, ms: Option<f64>, bytes: Option<f64>, details: Value) -> Value {
    let mut a = audit(id, json!(score), "metricSavings");
    let mut details = details;
    if details.is_null() {
        details = json!({"type": "table", "headings": [], "items": []});
    }
    if let Some(ms) = ms {
        a["metricSavings"] = json!({"FCP": ms, "LCP": ms});
        details["overallSavingsMs"] = json!(ms);
    } else {
        a["metricSavings"] = json!({"FCP": 0, "LCP": 0});
    }
    if let Some(bytes) = bytes {
        details["overallSavingsBytes"] = json!(bytes);
    }
    a["details"] = details;
    a
}

fn table(rows: usize) -> Value {
    let items: Vec<Value> = (0..rows)
        .map(|i| json!({"url": format!("https://site.test/r{i}")}))
        .collect();
    json!({"type": "table", "headings": [], "items": items})
}

fn checklist(no_redirects: bool, fast: bool, compression: bool) -> Value {
    json!({
        "type": "checklist",
        "items": {
            "noRedirects": {"label": "no redirects", "value": no_redirects},
            "serverResponseIsFast": {"label": "fast server", "value": fast},
            "usesCompression": {"label": "compression", "value": compression}
        }
    })
}

fn request(url: &str, resource_type: &str, transfer: u64, entity: &str) -> Value {
    json!({"url": url, "resourceType": resource_type, "transferSize": transfer, "resourceSize": transfer * 2, "entity": entity})
}

/// Lighthouse-Ergebnis „heavy“ (Lighthouse 13): jede Prüfung schlägt fehl, bekannte Zahlen (Golden [`expected_heavy`]).
pub fn lighthouse_heavy(url: &str, desktop: bool) -> Value {
    let mut audits = serde_json::Map::new();
    let mut put = |a: Value| {
        audits.insert(a["id"].as_str().unwrap_or_default().to_string(), a);
    };
    // Labordaten (INTEG-136); `interactive` fehlt wie in Lighthouse 13.
    put(numeric("first-contentful-paint", 0.35, 3456.7));
    put(numeric("speed-index", 0.40, 6123.4));
    put(numeric("largest-contentful-paint", 0.12, 7890.1));
    put(numeric("max-potential-fid", 0.21, 456.6));
    put(numeric("total-blocking-time", 0.18, 1234.4));
    put(numeric("cumulative-layout-shift", 0.30, 0.3124));
    // Netzwerk (INTEG-134): 10 Anfragen, Drittanbieter cdn.other.test.
    let mut network = audit("network-requests", Value::Null, "informative");
    network["details"] = json!({"type": "table", "headings": [], "items": [
        request(url, "Document", 45_000, "site.test"),
        request("https://site.test/a.css", "Stylesheet", 30_000, "site.test"),
        request("https://site.test/b.css", "Stylesheet", 20_000, "site.test"),
        request("https://site.test/a.js", "Script", 100_000, "site.test"),
        request("https://cdn.other.test/lib.js", "Script", 150_000, "other.test"),
        request("https://site.test/b.js", "Script", 50_000, "site.test"),
        request("https://site.test/hero.jpg", "Image", 400_000, "site.test"),
        request("https://cdn.other.test/ad.png", "Image", 300_000, "other.test"),
        request("https://site.test/font.woff2", "Font", 25_000, "site.test"),
        request("https://site.test/api", "XHR", 5_000, "site.test")
    ]});
    put(network);
    // Insights (INTEG-137)
    put(insight(
        "document-latency-insight",
        0.0,
        Some(250.0),
        None,
        checklist(false, false, false),
    ));
    let mut redirects = audit("redirects", json!(0), "metricSavings");
    redirects["details"] = table(3);
    put(redirects);
    put(insight(
        "lcp-discovery-insight",
        0.0,
        Some(0.0),
        None,
        json!({"type": "checklist", "items": {
            "priorityHinted": {"label": "fetchpriority", "value": false},
            "requestDiscoverable": {"label": "discoverable", "value": true},
            "eagerlyLoaded": {"label": "not lazy", "value": false}
        }}),
    ));
    let mut breakdown = audit("lcp-breakdown-insight", json!(null), "informative");
    breakdown["details"] = json!({"type": "list", "items": [{"type": "table", "headings": [], "items": [
        {"subpart": "timeToFirstByte", "duration": 812.4},
        {"subpart": "resourceLoadDelay", "duration": 1500.0},
        {"subpart": "resourceLoadDuration", "duration": 2300.2},
        {"subpart": "elementRenderDelay", "duration": 599.6}
    ]}]});
    put(breakdown);
    put(insight(
        "render-blocking-insight",
        0.0,
        Some(1200.0),
        None,
        table(2),
    ));
    put(insight(
        "network-dependency-tree-insight",
        0.0,
        Some(300.0),
        None,
        json!({"type": "list", "items": [
            {"type": "network-tree", "chains": {}, "longestChain": {"duration": 3456.6}},
            {"type": "table", "headings": [], "items": [{"origin": "https://cdn.other.test"}]}
        ]}),
    ));
    put(insight(
        "cache-insight",
        0.0,
        None,
        Some(80_000.0),
        table(4),
    ));
    put(insight(
        "cls-culprits-insight",
        0.0,
        None,
        None,
        json!({"type": "list", "items": [{"type": "table", "headings": [], "items": [
            {"node": "img.hero"}, {"node": "div.ad"}, {"node": "iframe"}
        ]}]}),
    ));
    put(insight(
        "image-delivery-insight",
        0.0,
        Some(400.0),
        Some(456_789.0),
        table(2),
    ));
    put(insight(
        "forced-reflow-insight",
        0.0,
        Some(210.0),
        None,
        table(1),
    ));
    put(insight(
        "legacy-javascript-insight",
        0.0,
        None,
        Some(23_456.0),
        table(1),
    ));
    put(insight(
        "duplicated-javascript-insight",
        0.0,
        None,
        Some(34_567.0),
        table(1),
    ));
    let mut third = audit("third-parties-insight", json!(null), "informative");
    third["details"] = table(4);
    put(third);
    put(numeric("total-byte-weight", 0.30, 5_432_100.0));
    // Diagnostics (INTEG-138)
    put(insight(
        "unminified-css",
        0.5,
        Some(150.0),
        Some(12_000.0),
        table(1),
    ));
    put(insight(
        "unminified-javascript",
        0.5,
        Some(300.0),
        Some(45_000.0),
        table(1),
    ));
    put(insight(
        "unused-css-rules",
        0.5,
        Some(450.0),
        Some(67_000.0),
        table(2),
    ));
    put(insight(
        "unused-javascript",
        0.0,
        Some(900.0),
        Some(234_000.0),
        table(3),
    ));
    put(numeric("bootup-time", 0.45, 3456.7));
    put(numeric("mainthread-work-breakdown", 0.62, 5678.9));
    put(insight(
        "dom-size-insight",
        0.0,
        Some(120.0),
        None,
        json!({"type": "table", "headings": [], "items": [
            {"statistic": "Total elements", "value": {"type": "numeric", "value": 2345}},
            {"statistic": "Most children", "value": {"type": "numeric", "value": 120}},
            {"statistic": "DOM depth", "value": {"type": "numeric", "value": 32}}
        ]}),
    ));
    put(insight(
        "font-display-insight",
        0.0,
        Some(80.0),
        None,
        table(1),
    ));
    let mut timings = audit("user-timings", json!(null), "informative");
    timings["details"] = table(3);
    put(timings);
    json!({
        "lighthouseVersion": "13.0.0",
        "requestedUrl": url,
        "finalDisplayedUrl": url,
        "configSettings": {"formFactor": if desktop { "desktop" } else { "mobile" }},
        "categories": {"performance": {"id": "performance", "score": 0.42}},
        "audits": Value::Object(audits),
        "entities": [
            {"name": "site.test", "isFirstParty": true, "origins": ["https://site.test"]},
            {"name": "other.test", "origins": ["https://cdn.other.test"]}
        ]
    })
}

/// Lighthouse-Ergebnis „fast“ (Lighthouse 13): alles bestanden.
pub fn lighthouse_fast(url: &str, desktop: bool) -> Value {
    let mut audits = serde_json::Map::new();
    let mut put = |a: Value| {
        audits.insert(a["id"].as_str().unwrap_or_default().to_string(), a);
    };
    put(numeric("first-contentful-paint", 1.0, 812.0));
    put(numeric("speed-index", 0.99, 1001.0));
    put(numeric("largest-contentful-paint", 0.98, 1234.0));
    put(numeric("max-potential-fid", 1.0, 16.0));
    put(numeric("total-blocking-time", 1.0, 0.0));
    put(numeric("cumulative-layout-shift", 1.0, 0.0));
    put(numeric("interactive", 0.97, 1500.0));
    let mut network = audit("network-requests", Value::Null, "informative");
    network["details"] = json!({"type": "table", "headings": [], "items": [
        request(url, "Document", 8_000, "site.test"),
        request("https://site.test/app.css", "Stylesheet", 4_000, "site.test")
    ]});
    put(network);
    put(insight(
        "document-latency-insight",
        1.0,
        Some(0.0),
        None,
        checklist(true, true, true),
    ));
    put(audit("lcp-discovery-insight", json!(null), "notApplicable"));
    put(insight(
        "render-blocking-insight",
        1.0,
        Some(0.0),
        None,
        table(0),
    ));
    put(insight(
        "network-dependency-tree-insight",
        1.0,
        Some(0.0),
        None,
        json!({"type": "list", "items": [
            {"type": "network-tree", "chains": {}, "longestChain": {"duration": 120.4}}
        ]}),
    ));
    // Einsparangabe fehlt bei bestandenem Audit → Zelle 0 (INTEG-137).
    put(audit("cache-insight", json!(1), "metricSavings"));
    put(audit("cls-culprits-insight", json!(null), "notApplicable"));
    put(insight(
        "image-delivery-insight",
        1.0,
        None,
        Some(0.0),
        table(0),
    ));
    put(audit("forced-reflow-insight", json!(null), "notApplicable"));
    put(insight(
        "legacy-javascript-insight",
        1.0,
        None,
        Some(0.0),
        table(0),
    ));
    put(insight(
        "duplicated-javascript-insight",
        1.0,
        None,
        Some(0.0),
        table(0),
    ));
    let mut third = audit("third-parties-insight", json!(null), "notApplicable");
    third["details"] = table(0);
    put(third);
    put(numeric("total-byte-weight", 1.0, 12_000.0));
    put(insight(
        "unminified-css",
        1.0,
        Some(0.0),
        Some(0.0),
        table(0),
    ));
    put(insight(
        "unminified-javascript",
        1.0,
        Some(0.0),
        Some(0.0),
        table(0),
    ));
    put(insight(
        "unused-css-rules",
        1.0,
        Some(0.0),
        Some(0.0),
        table(0),
    ));
    put(insight(
        "unused-javascript",
        1.0,
        Some(0.0),
        Some(0.0),
        table(0),
    ));
    put(numeric("bootup-time", 0.9, 400.0));
    put(numeric("mainthread-work-breakdown", 1.0, 900.0));
    put(insight(
        "dom-size-insight",
        1.0,
        Some(0.0),
        None,
        json!({"type": "table", "headings": [], "items": [
            {"statistic": "Total elements", "value": {"type": "numeric", "value": 120}}
        ]}),
    ));
    put(audit("font-display-insight", json!(null), "notApplicable"));
    let mut timings = audit("user-timings", json!(null), "notApplicable");
    timings["details"] = table(0);
    put(timings);
    json!({
        "lighthouseVersion": "13.0.0",
        "requestedUrl": url,
        "finalDisplayedUrl": url,
        "configSettings": {"formFactor": if desktop { "desktop" } else { "mobile" }},
        "categories": {"performance": {"id": "performance", "score": 0.985}},
        "audits": Value::Object(audits),
        "entities": [{"name": "site.test", "isFirstParty": true, "origins": ["https://site.test"]}]
    })
}

fn crux_metrics(desktop: bool, origin: bool) -> Value {
    // (Perzentil, Kategorie); Desktop und Origin unterscheiden sich, damit Verwechslungen auffallen.
    let (fcp, lcp, cls, inp, ttfb) = match (desktop, origin) {
        (false, false) => (
            (1700, "FAST"),
            (2300, "FAST"),
            (12, "AVERAGE"),
            (180, "FAST"),
            (900, "AVERAGE"),
        ),
        (false, true) => (
            (2100, "AVERAGE"),
            (4200, "SLOW"),
            (5, "FAST"),
            (520, "SLOW"),
            (1900, "SLOW"),
        ),
        (true, false) => (
            (900, "FAST"),
            (1500, "FAST"),
            (2, "FAST"),
            (90, "FAST"),
            (400, "FAST"),
        ),
        (true, true) => (
            (1100, "FAST"),
            (2000, "FAST"),
            (8, "FAST"),
            (150, "FAST"),
            (700, "FAST"),
        ),
    };
    let m = |(p, c): (u32, &str)| json!({"percentile": p, "category": c, "distributions": []});
    json!({
        "FIRST_CONTENTFUL_PAINT_MS": m(fcp),
        "LARGEST_CONTENTFUL_PAINT_MS": m(lcp),
        "CUMULATIVE_LAYOUT_SHIFT_SCORE": m(cls),
        "INTERACTION_TO_NEXT_PAINT": m(inp),
        "EXPERIMENTAL_TIME_TO_FIRST_BYTE": m(ttfb)
    })
}

fn set_crux(body: &mut Value, variant: &str, desktop: bool) {
    let url = body["id"].as_str().unwrap_or_default().to_string();
    let origin = json!({"id": "https://site.test", "metrics": crux_metrics(desktop, true), "overall_category": "SLOW"});
    match variant {
        "crux-full" => {
            body["loadingExperience"] = json!({"id": url, "metrics": crux_metrics(desktop, false), "overall_category": "AVERAGE"});
            body["originLoadingExperience"] = origin;
        }
        "crux-origin-only" => {
            body["loadingExperience"] = json!({"id": url});
            body["originLoadingExperience"] = origin;
        }
        "crux-fallback" => {
            body["loadingExperience"] = json!({"id": "https://site.test", "metrics": crux_metrics(desktop, true), "overall_category": "SLOW", "origin_fallback": true});
            body["originLoadingExperience"] = origin;
        }
        _ => {
            if let Some(map) = body.as_object_mut() {
                map.remove("loadingExperience");
                map.remove("originLoadingExperience");
            }
        }
    }
}

fn envelope(url: &str, lighthouse: Value) -> Value {
    json!({
        "kind": "pagespeedonline#result",
        "id": url,
        "captchaResult": "CAPTCHA_NOT_NEEDED",
        "analysisUTCTimestamp": "2026-10-19T08:00:00.000Z",
        "lighthouseResult": lighthouse
    })
}

/// `psi-heavy.json`: Remote-Antwort mit [`lighthouse_heavy`] und vollständigen CrUX-Daten.
pub fn psi_heavy(url: &str, desktop: bool) -> Value {
    let mut body = envelope(url, lighthouse_heavy(url, desktop));
    set_crux(&mut body, "crux-full", desktop);
    body
}

/// `psi-fast.json`: Remote-Antwort mit [`lighthouse_fast`] und vollständigen CrUX-Daten.
pub fn psi_fast(url: &str, desktop: bool) -> Value {
    let mut body = envelope(url, lighthouse_fast(url, desktop));
    set_crux(&mut body, "crux-full", desktop);
    body
}

/// `psi-legacy-cache.json`: ältere Lighthouse-Version mit `uses-long-cache-ttl` (120.000 Bytes) und ohne `cache-insight`.
pub fn psi_legacy_cache(url: &str, desktop: bool) -> Value {
    let mut lh = lighthouse_fast(url, desktop);
    lh["lighthouseVersion"] = json!("12.8.2");
    let audits = lh["audits"].as_object_mut().expect("audits");
    audits.remove("cache-insight");
    audits.insert(
        "uses-long-cache-ttl".into(),
        insight("uses-long-cache-ttl", 0.5, None, Some(120_000.0), table(3)),
    );
    let mut body = envelope(url, lh);
    set_crux(&mut body, "crux-none", desktop);
    body
}

/// Antwort mit Lighthouse-Laufzeitfehler (`lh-errored-document`).
pub fn errored_document(url: &str, desktop: bool) -> Value {
    let mut lh = lighthouse_fast(url, desktop);
    lh["runtimeError"] = json!({
        "code": "ERRORED_DOCUMENT_REQUEST",
        "message": "The page could not be loaded (status code 401)."
    });
    lh["categories"]["performance"]["score"] = Value::Null;
    envelope(url, lh)
}

// ---------------------------------------------------------------------------------------------------------------------
// Golden-Werte (Ersatz für expected-overview.csv, expected-insights.csv, expected-diagnostics.csv)

/// Ein erwarteter Spaltenwert.
#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    /// Zahl.
    Num(f64),
    /// Text.
    Text(&'static str),
}

/// Erwartete Werte für [`psi_heavy`] (Mobil); Spalten, die nicht genannt sind, bleiben leer.
pub fn expected_heavy() -> Vec<(&'static str, Cell)> {
    use Cell::{Num, Text};
    vec![
        ("PSI Status", Text("Success")),
        // Overview (INTEG-134)
        // Bytes: Cache 80.000 + Bild 456.789 + Legacy 23.456 + Duplikat 34.567 + CSS 12.000 + JS 45.000
        //        + unbenutztes CSS 67.000 + unbenutztes JS 234.000 = 952.812
        ("Total Size Savings (Bytes)", Num(952_812.0)),
        // ms: Latenz 250 + LCP-Auffindbarkeit 0 + Render-Blocking 1.200 + Preconnect 300 + Bild 400 + Reflow 210
        //     + DOM 120 + Font 80 + CSS 150 + JS 300 + unbenutztes CSS 450 + unbenutztes JS 900 = 4.360
        ("Total Time Savings (ms)", Num(4_360.0)),
        ("Total Requests", Num(10.0)),
        ("Total Page Size (Bytes)", Num(1_125_000.0)),
        ("HTML Size", Num(45_000.0)),
        ("HTML Count", Num(1.0)),
        ("Image Size", Num(700_000.0)),
        ("Image Count", Num(2.0)),
        ("CSS Size", Num(50_000.0)),
        ("CSS Count", Num(2.0)),
        ("JavaScript Size", Num(300_000.0)),
        ("JavaScript Count", Num(3.0)),
        ("Font Size", Num(25_000.0)),
        ("Font Count", Num(1.0)),
        ("Media Size", Num(0.0)),
        ("Media Count", Num(0.0)),
        ("Other Size", Num(5_000.0)),
        ("Other Count", Num(1.0)),
        ("Third Party Size", Num(450_000.0)),
        ("Third Party Count", Num(2.0)),
        // CrUX: siehe expected_crux
        // Lighthouse (INTEG-136)
        ("Performance Score", Num(42.0)),
        ("First Contentful Paint Time (sec)", Num(3.46)),
        ("First Contentful Paint Score", Num(35.0)),
        ("Speed Index Time (sec)", Num(6.12)),
        ("Speed Index Score", Num(40.0)),
        ("Largest Contentful Paint Time (sec)", Num(7.89)),
        ("Largest Contentful Paint Score", Num(12.0)),
        ("Max Potential First Input Delay (ms)", Num(457.0)),
        ("Max Potential First Input Delay Score", Num(21.0)),
        ("Total Blocking Time (ms)", Num(1_234.0)),
        ("Total Blocking Time Score", Num(18.0)),
        ("Cumulative Layout Shift", Num(0.312)),
        ("Cumulative Layout Shift Score", Num(30.0)),
        ("Lighthouse Version", Text("13.0.0")),
        // Insights (INTEG-137)
        ("Document Request Latency", Text("Fail")),
        ("Number of Redirects", Num(2.0)),
        ("Server Responds Quickly", Text("No")),
        ("Applies Text Compression", Text("No")),
        ("LCP Request Discovery", Text("Fail")),
        ("LCP Breakdown TTFB (ms)", Num(812.0)),
        ("LCP Breakdown Load Delay (ms)", Num(1_500.0)),
        ("LCP Breakdown Load Duration (ms)", Num(2_300.0)),
        ("LCP Breakdown Render Delay (ms)", Num(600.0)),
        ("Render Blocking Requests Savings (ms)", Num(1_200.0)),
        ("Preconnect Candidates Savings (ms)", Num(300.0)),
        ("Maximum Critical Path Latency (ms)", Num(3_457.0)),
        (
            "Use Efficient Cache Lifetimes Savings (Bytes)",
            Num(80_000.0),
        ),
        ("Layout Shift Culprits", Num(3.0)),
        ("Improve Image Delivery Savings (Bytes)", Num(456_789.0)),
        ("Forced Reflow Savings (ms)", Num(210.0)),
        ("Legacy JavaScript Savings (Bytes)", Num(23_456.0)),
        ("Duplicated JavaScript Savings (Bytes)", Num(34_567.0)),
        ("3rd Parties", Num(4.0)),
        ("Network Payload Size (Bytes)", Num(5_432_100.0)),
        // Diagnostics (INTEG-138); `uses-long-cache-ttl` fehlt → Efficient Cache Policy Savings (Bytes) leer.
        ("Minify CSS Savings (ms)", Num(150.0)),
        ("Minify CSS Savings (Bytes)", Num(12_000.0)),
        ("Minify JavaScript Savings (ms)", Num(300.0)),
        ("Minify JavaScript Savings (Bytes)", Num(45_000.0)),
        ("Reduce Unused CSS Savings (ms)", Num(450.0)),
        ("Reduce Unused CSS Savings (Bytes)", Num(67_000.0)),
        ("Reduce Unused JavaScript Savings (ms)", Num(900.0)),
        ("Reduce Unused JavaScript Savings (Bytes)", Num(234_000.0)),
        ("JavaScript Execution Time (sec)", Num(3.46)),
        ("JavaScript Execution Time Category", Text("Slow")),
        ("Minimize Main-Thread Work (sec)", Num(5.68)),
        ("Minimize Main-Thread Work Category", Text("Average")),
        ("DOM Size", Num(2_345.0)),
        ("Font Display Savings (ms)", Num(80.0)),
        ("User Timing Marks and Measures", Num(3.0)),
    ]
}

/// Erwartete Werte für [`psi_fast`] (Mobil, ohne CrUX).
pub fn expected_fast() -> Vec<(&'static str, Cell)> {
    use Cell::{Num, Text};
    vec![
        ("PSI Status", Text("Success")),
        ("Total Size Savings (Bytes)", Num(0.0)),
        ("Total Time Savings (ms)", Num(0.0)),
        ("Total Requests", Num(2.0)),
        ("Total Page Size (Bytes)", Num(12_000.0)),
        ("Performance Score", Num(99.0)),
        ("Time to Interactive (sec)", Num(1.5)),
        ("Time to Interactive Score", Num(97.0)),
        ("Cumulative Layout Shift", Num(0.0)),
        ("Document Request Latency", Text("Pass")),
        ("Number of Redirects", Num(0.0)),
        ("Server Responds Quickly", Text("Yes")),
        ("Applies Text Compression", Text("Yes")),
        ("LCP Request Discovery", Text("Pass")),
        ("Use Efficient Cache Lifetimes Savings (Bytes)", Num(0.0)),
        ("Layout Shift Culprits", Num(0.0)),
        ("3rd Parties", Num(0.0)),
        ("Network Payload Size (Bytes)", Num(12_000.0)),
        ("JavaScript Execution Time Category", Text("Good")),
        ("Minimize Main-Thread Work Category", Text("Good")),
        ("DOM Size", Num(120.0)),
        ("Font Display Savings (ms)", Num(0.0)),
        ("User Timing Marks and Measures", Num(0.0)),
    ]
}

/// Erwartete CrUX-Werte für `crux-full` (Mobil): URL- und Origin-Spalten.
pub fn expected_crux() -> Vec<(&'static str, Cell)> {
    use Cell::{Num, Text};
    vec![
        // URL: LCP Good, CLS Average → Fail.
        ("Core Web Vitals Assessment", Text("Fail")),
        ("CrUX First Contentful Paint (sec)", Num(1.7)),
        ("CrUX First Contentful Paint Category", Text("Good")),
        ("CrUX Largest Contentful Paint (sec)", Num(2.3)),
        ("CrUX Largest Contentful Paint Category", Text("Good")),
        ("CrUX Cumulative Layout Shift", Num(0.12)),
        (
            "CrUX Cumulative Layout Shift Category",
            Text("Needs Improvement"),
        ),
        ("CrUX Interaction to Next Paint (ms)", Num(180.0)),
        ("CrUX Interaction to Next Paint Category", Text("Good")),
        ("CrUX Time to First Byte (ms)", Num(900.0)),
        (
            "CrUX Time to First Byte Category",
            Text("Needs Improvement"),
        ),
        ("CrUX Origin Core Web Vitals Assessment", Text("Fail")),
        ("CrUX Origin First Contentful Paint (sec)", Num(2.1)),
        (
            "CrUX Origin First Contentful Paint Category",
            Text("Needs Improvement"),
        ),
        ("CrUX Origin Largest Contentful Paint (sec)", Num(4.2)),
        (
            "CrUX Origin Largest Contentful Paint Category",
            Text("Poor"),
        ),
        ("CrUX Origin Cumulative Layout Shift", Num(0.05)),
        ("CrUX Origin Cumulative Layout Shift Category", Text("Good")),
        ("CrUX Origin Interaction to Next Paint (ms)", Num(520.0)),
        (
            "CrUX Origin Interaction to Next Paint Category",
            Text("Poor"),
        ),
        ("CrUX Origin Time to First Byte (ms)", Num(1_900.0)),
        ("CrUX Origin Time to First Byte Category", Text("Poor")),
    ]
}

/// Die 19 PageSpeed-Filter (INTEG-139) in Spec-Reihenfolge mit ihrer Metrikgruppe (INTEG-133).
pub const FILTERS: [(&str, &str); 19] = [
    ("Document Request Latency", "insights"),
    ("LCP Request Discovery", "insights"),
    ("Render Blocking Requests", "insights"),
    ("Network Dependency Tree", "insights"),
    ("Use Efficient Cache Lifetimes", "insights"),
    ("Layout Shift Culprits", "insights"),
    ("Improve Image Delivery", "insights"),
    ("Forced Reflow", "insights"),
    ("Legacy JavaScript", "insights"),
    ("Duplicated JavaScript", "insights"),
    ("Avoid Enormous Network Payloads", "insights"),
    ("Minify CSS", "diagnostics"),
    ("Minify JavaScript", "diagnostics"),
    ("Reduce Unused CSS", "diagnostics"),
    ("Reduce Unused JavaScript", "diagnostics"),
    ("Reduce JavaScript Execution Time", "diagnostics"),
    ("Minimize Main-Thread Work", "diagnostics"),
    ("Optimize DOM Size", "diagnostics"),
    ("Font Display", "diagnostics"),
];
