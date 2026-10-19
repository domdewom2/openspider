//! Lighthouse- und PSI-Antworten → Spaltenwerte (INTEG-134 … INTEG-139, INTEG-143).
//!
//! Beide Quellen (Remote über die PSI-API, Lokal über einen eigenen Lighthouse-Lauf) liefern dasselbe
//! `lighthouseResult`; [`parse_lighthouse`] bildet es für beide gleich ab (TECH-230). Nur die Remote-Antwort trägt
//! zusätzlich CrUX-Felddaten ([`parse_crux`]).

use std::collections::BTreeSet;

use serde_json::Value;

use super::columns::{fail_metric, PSI_ERROR, PSI_STATUS};

/// Ein Zellwert.
#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    /// Zahl (`api_values.v_real`).
    Num(f64),
    /// Text (`api_values.v_text`).
    Text(String),
}

/// Ergebnis einer URL: Zellen nach Spaltenname bzw. `fail:<Filter>`, in stabiler Reihenfolge.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Outcome {
    /// `(Metrik, Wert)`.
    pub cells: Vec<(String, Cell)>,
}

impl Outcome {
    fn put(&mut self, name: &str, cell: Option<Cell>) {
        if let Some(cell) = cell {
            self.cells.push((name.to_string(), cell));
        }
    }
    fn num(&mut self, name: &str, value: Option<f64>) {
        self.put(name, value.map(Cell::Num));
    }
    fn text(&mut self, name: &str, value: Option<&str>) {
        self.put(name, value.map(|v| Cell::Text(v.to_string())));
    }

    /// Wert einer Metrik.
    pub fn get(&self, name: &str) -> Option<&Cell> {
        self.cells.iter().find(|(n, _)| n == name).map(|(_, c)| c)
    }

    /// `PSI Status = Success`?
    pub fn is_success(&self) -> bool {
        self.get(PSI_STATUS) == Some(&Cell::Text("Success".into()))
    }

    /// Fehlerergebnis (INTEG-143): `PSI Status = Error`, `PSI Error = <Text>`, keine weiteren Spalten.
    pub fn error(message: impl Into<String>) -> Outcome {
        Outcome {
            cells: vec![
                (PSI_STATUS.into(), Cell::Text("Error".into())),
                (PSI_ERROR.into(), Cell::Text(message.into())),
            ],
        }
    }
}

/// Audits mit Einsparangaben, deren Summen `Total Size Savings`/`Total Time Savings` bilden (INTEG-134).
const SAVINGS_AUDITS: [&str; 17] = [
    "document-latency-insight",
    "lcp-discovery-insight",
    "render-blocking-insight",
    "network-dependency-tree-insight",
    "cache-insight",
    "cls-culprits-insight",
    "image-delivery-insight",
    "forced-reflow-insight",
    "legacy-javascript-insight",
    "duplicated-javascript-insight",
    "dom-size-insight",
    "font-display-insight",
    "unminified-css",
    "unminified-javascript",
    "unused-css-rules",
    "unused-javascript",
    "uses-long-cache-ttl",
];

/// Wertet eine vollständige PSI-Antwort (Remote) aus: Lighthouse-Teil, CrUX und Laufzeitfehler.
pub fn parse_psi(body: &Value) -> Outcome {
    let Some(lh) = body.get("lighthouseResult") else {
        return Outcome::error("NO_LIGHTHOUSE_RESULT Antwort ohne Lighthouse-Ergebnis");
    };
    let mut out = parse_lighthouse(lh);
    if out.is_success() {
        out.cells.extend(parse_crux(body));
    }
    out
}

/// Wertet ein `lighthouseResult` aus (gemeinsam für Remote und Lokal, TECH-230).
pub fn parse_lighthouse(lh: &Value) -> Outcome {
    if let Some(code) = lh.pointer("/runtimeError/code").and_then(Value::as_str) {
        let message = lh
            .pointer("/runtimeError/message")
            .and_then(Value::as_str)
            .unwrap_or_default();
        return Outcome::error(format!("{code} {message}").trim().to_string());
    }
    let audits = lh.get("audits").cloned().unwrap_or(Value::Null);
    let a = |id: &str| audits.get(id).filter(|a| mode(a) != "error");
    let mut out = Outcome::default();
    out.text(PSI_STATUS, Some("Success"));

    overview(lh, &audits, &mut out);
    lab(lh, &a, &mut out);
    insights(&a, &mut out);
    diagnostics(&a, &mut out);
    fails(&a, &mut out);
    out
}

fn mode(audit: &Value) -> &str {
    audit
        .get("scoreDisplayMode")
        .and_then(Value::as_str)
        .unwrap_or("")
}

fn score(audit: &Value) -> Option<f64> {
    audit.get("score").and_then(Value::as_f64)
}

/// Score 0–1 → ganzzahlig 0–100.
fn score100(audit: Option<&Value>) -> Option<f64> {
    audit.and_then(score).map(|s| (s * 100.0).round())
}

fn numeric(audit: Option<&Value>) -> Option<f64> {
    audit
        .and_then(|a| a.get("numericValue"))
        .and_then(Value::as_f64)
}

fn round_to(v: f64, decimals: i32) -> f64 {
    let f = 10f64.powi(decimals);
    (v * f).round() / f
}

/// Millisekunden → Sekunden mit zwei Nachkommastellen.
fn secs(ms: Option<f64>) -> Option<f64> {
    ms.map(|ms| round_to(ms / 1000.0, 2))
}

/// Ein Audit gilt als bestanden, wenn es nicht anwendbar/manuell ist, sein Score ≥ 0,9 ist oder es ohne Score weder
/// Einsparung noch beanstandete Elemente hat (INTEG-139).
fn passed(audit: &Value) -> bool {
    match mode(audit) {
        "notApplicable" | "manual" => return true,
        _ => {}
    }
    if let Some(s) = score(audit) {
        return s >= 0.9;
    }
    savings_ms(audit).unwrap_or(0.0) <= 0.0
        && savings_bytes(audit).unwrap_or(0.0) <= 0.0
        && rows(audit.get("details")).unwrap_or(0) == 0
}

/// Einsparung in ms: `details.overallSavingsMs`, sonst das Maximum der Zeit-Metriken aus `metricSavings`.
fn savings_ms(audit: &Value) -> Option<f64> {
    if let Some(ms) = audit
        .pointer("/details/overallSavingsMs")
        .and_then(Value::as_f64)
    {
        return Some(ms);
    }
    let savings = audit.get("metricSavings")?.as_object()?;
    savings
        .iter()
        .filter(|(k, _)| k.as_str() != "CLS")
        .filter_map(|(_, v)| v.as_f64())
        .reduce(f64::max)
}

/// Einsparung in Bytes: `details.overallSavingsBytes`, sonst die Summe der `wastedBytes` der Zeilen.
fn savings_bytes(audit: &Value) -> Option<f64> {
    if let Some(b) = audit
        .pointer("/details/overallSavingsBytes")
        .and_then(Value::as_f64)
    {
        return Some(b);
    }
    let items = audit.pointer("/details/items")?.as_array()?;
    let wasted: Vec<f64> = items
        .iter()
        .filter_map(|i| i.get("wastedBytes").and_then(Value::as_f64))
        .collect();
    (!wasted.is_empty()).then(|| wasted.iter().sum())
}

/// Einsparzelle (INTEG-137): Wert, sonst `0` bei bestandenem Audit, sonst leer.
fn savings_cell(audit: Option<&Value>, bytes: bool) -> Option<f64> {
    let audit = audit?;
    let value = if bytes {
        savings_bytes(audit)
    } else {
        savings_ms(audit)
    };
    match value {
        Some(v) => Some(v.round()),
        None if passed(audit) => Some(0.0),
        None => None,
    }
}

/// Zeilen der ersten Tabelle in `details` (auch verschachtelt in einer Liste).
fn rows(details: Option<&Value>) -> Option<usize> {
    let details = details?;
    match details.get("type").and_then(Value::as_str) {
        Some("table") | Some("opportunity") => {
            details.get("items").and_then(Value::as_array).map(Vec::len)
        }
        Some("list") => details
            .get("items")?
            .as_array()?
            .iter()
            .find_map(|i| rows(Some(i))),
        _ => None,
    }
}

/// Zählt Zeilen eines Audits; ein vorhandenes Audit ohne Tabelle zählt `0`.
fn row_count(audit: Option<&Value>) -> Option<f64> {
    audit.map(|a| rows(a.get("details")).unwrap_or(0) as f64)
}

fn category(audit: Option<&Value>) -> Option<&'static str> {
    // INTEG-138: ≥ 0,9 Good, 0,5 ≤ s < 0,9 Average, < 0,5 Slow.
    audit.and_then(score).map(|s| {
        if s >= 0.9 {
            "Good"
        } else if s >= 0.5 {
            "Average"
        } else {
            "Slow"
        }
    })
}

/// Kategorie eines Audit-Scores (für die Grenzfalltests von INTEG-138).
pub fn score_category(score: f64) -> &'static str {
    category(Some(&serde_json::json!({ "score": score }))).unwrap_or("Slow")
}

/// Farbklasse eines Scores 0–100 (INTEG-136): 90–100 grün, 50–89 orange, 0–49 rot.
pub fn score_color(score: u8) -> &'static str {
    match score {
        90..=u8::MAX => "green",
        50..=89 => "orange",
        _ => "red",
    }
}

fn host(url: &str) -> Option<String> {
    url::Url::parse(url)
        .ok()?
        .host_str()
        .map(|h| h.to_ascii_lowercase())
}

/// Grobe Site-Zuordnung ohne Public-Suffix-Liste (nur Rückfall, wenn Lighthouse keine `entities` liefert).
fn site(host: &str) -> String {
    let labels: Vec<&str> = host.rsplit('.').take(2).collect();
    labels.into_iter().rev().collect::<Vec<_>>().join(".")
}

fn overview(lh: &Value, audits: &Value, out: &mut Outcome) {
    // Summen der Einsparungen; `uses-long-cache-ttl` nur ohne `cache-insight` (keine Doppelzählung).
    let has_cache_insight = audits.get("cache-insight").is_some();
    let mut bytes = 0.0;
    let mut ms = 0.0;
    for id in SAVINGS_AUDITS {
        if id == "uses-long-cache-ttl" && has_cache_insight {
            continue;
        }
        let Some(a) = audits.get(id).filter(|a| mode(a) != "error") else {
            continue;
        };
        bytes += savings_bytes(a).unwrap_or(0.0);
        ms += savings_ms(a).unwrap_or(0.0);
    }
    out.num("Total Size Savings (Bytes)", Some(bytes.round()));
    out.num("Total Time Savings (ms)", Some(ms.round()));

    let Some(items) = audits
        .pointer("/network-requests/details/items")
        .and_then(Value::as_array)
    else {
        return;
    };
    let first_party: BTreeSet<&str> = lh
        .get("entities")
        .and_then(Value::as_array)
        .map(|e| {
            e.iter()
                .filter(|e| e.get("isFirstParty").and_then(Value::as_bool) == Some(true))
                .filter_map(|e| e.get("name").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default();
    let has_entities = lh.get("entities").is_some();
    let main_site = ["finalDisplayedUrl", "finalUrl", "requestedUrl"]
        .iter()
        .find_map(|k| lh.get(*k).and_then(Value::as_str))
        .and_then(host)
        .map(|h| site(&h));
    // HTML, Image, CSS, JavaScript, Font, Media, Other, Third Party
    let mut sums = [(0.0f64, 0u64); 8];
    for item in items {
        let size = item
            .get("transferSize")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let slot = match item
            .get("resourceType")
            .and_then(Value::as_str)
            .unwrap_or("")
        {
            "Document" => 0,
            "Image" => 1,
            "Stylesheet" => 2,
            "Script" => 3,
            "Font" => 4,
            "Media" => 5,
            _ => 6,
        };
        sums[slot].0 += size;
        sums[slot].1 += 1;
        let third = if has_entities {
            item.get("entity")
                .and_then(Value::as_str)
                .is_some_and(|e| !first_party.contains(e))
        } else {
            match (
                item.get("url").and_then(Value::as_str).and_then(host),
                &main_site,
            ) {
                (Some(h), Some(main)) => site(&h) != *main,
                _ => false,
            }
        };
        if third {
            sums[7].0 += size;
            sums[7].1 += 1;
        }
    }
    let total: f64 = sums[..7].iter().map(|s| s.0).sum();
    out.num("Total Requests", Some(items.len() as f64));
    out.num("Total Page Size (Bytes)", Some(total.round()));
    let names = [
        "HTML",
        "Image",
        "CSS",
        "JavaScript",
        "Font",
        "Media",
        "Other",
        "Third Party",
    ];
    for (name, (size, count)) in names.iter().zip(sums) {
        out.num(&format!("{name} Size"), Some(size.round()));
        out.num(&format!("{name} Count"), Some(count as f64));
    }
}

fn lab<'a>(lh: &Value, a: &impl Fn(&str) -> Option<&'a Value>, out: &mut Outcome) {
    let perf = lh
        .pointer("/categories/performance/score")
        .and_then(Value::as_f64)
        .map(|s| (s * 100.0).round());
    out.num("Performance Score", perf);
    for (name, id) in [
        ("First Contentful Paint", "first-contentful-paint"),
        ("Speed Index", "speed-index"),
        ("Largest Contentful Paint", "largest-contentful-paint"),
    ] {
        out.num(&format!("{name} Time (sec)"), secs(numeric(a(id))));
        out.num(&format!("{name} Score"), score100(a(id)));
    }
    out.num("Time to Interactive (sec)", secs(numeric(a("interactive"))));
    out.num("Time to Interactive Score", score100(a("interactive")));
    out.num(
        "Max Potential First Input Delay (ms)",
        numeric(a("max-potential-fid")).map(f64::round),
    );
    out.num(
        "Max Potential First Input Delay Score",
        score100(a("max-potential-fid")),
    );
    out.num(
        "Total Blocking Time (ms)",
        numeric(a("total-blocking-time")).map(f64::round),
    );
    out.num(
        "Total Blocking Time Score",
        score100(a("total-blocking-time")),
    );
    out.num(
        "Cumulative Layout Shift",
        numeric(a("cumulative-layout-shift")).map(|v| round_to(v, 3)),
    );
    out.num(
        "Cumulative Layout Shift Score",
        score100(a("cumulative-layout-shift")),
    );
    out.text(
        "Lighthouse Version",
        lh.get("lighthouseVersion").and_then(Value::as_str),
    );
}

fn yes_no(v: Option<bool>) -> Option<&'static str> {
    v.map(|b| if b { "Yes" } else { "No" })
}

fn pass_fail(audit: Option<&Value>) -> Option<&'static str> {
    audit.map(|a| if passed(a) { "Pass" } else { "Fail" })
}

/// Sucht das erste Objekt mit dem Schlüssel `key` in einer verschachtelten Struktur.
fn find_key<'v>(v: &'v Value, key: &str) -> Option<&'v Value> {
    match v {
        Value::Object(map) => map
            .get(key)
            .or_else(|| map.values().find_map(|c| find_key(c, key))),
        Value::Array(items) => items.iter().find_map(|c| find_key(c, key)),
        _ => None,
    }
}

/// Alle Objekte mit dem Schlüssel `key` (Tiefensuche).
fn collect_with<'v>(v: &'v Value, key: &str, out: &mut Vec<&'v Value>) {
    match v {
        Value::Object(map) => {
            if map.contains_key(key) {
                out.push(v);
            }
            map.values().for_each(|c| collect_with(c, key, out));
        }
        Value::Array(items) => items.iter().for_each(|c| collect_with(c, key, out)),
        _ => {}
    }
}

fn insights<'a>(a: &impl Fn(&str) -> Option<&'a Value>, out: &mut Outcome) {
    // Document Request Latency mit Teilprüfungen.
    let doc = a("document-latency-insight");
    out.text("Document Request Latency", pass_fail(doc));
    let check = |name: &str| {
        doc.and_then(|d| d.pointer(&format!("/details/items/{name}/value")))
            .and_then(Value::as_bool)
    };
    let redirects = check("noRedirects").map(|none| {
        if none {
            0.0
        } else {
            // `redirects`-Audit: Zeilen = Kette inklusive Ziel.
            rows(a("redirects").and_then(|r| r.get("details")))
                .map(|n| n.saturating_sub(1).max(1) as f64)
                .unwrap_or(1.0)
        }
    });
    out.num("Number of Redirects", redirects);
    out.text(
        "Server Responds Quickly",
        yes_no(check("serverResponseIsFast")),
    );
    out.text("Applies Text Compression", yes_no(check("usesCompression")));
    out.text(
        "LCP Request Discovery",
        pass_fail(a("lcp-discovery-insight")),
    );

    // LCP Breakdown (Phasen in ms).
    if let Some(details) = a("lcp-breakdown-insight").and_then(|b| b.get("details")) {
        let mut parts = Vec::new();
        collect_with(details, "subpart", &mut parts);
        for (subpart, name) in [
            ("timeToFirstByte", "LCP Breakdown TTFB (ms)"),
            ("resourceLoadDelay", "LCP Breakdown Load Delay (ms)"),
            ("resourceLoadDuration", "LCP Breakdown Load Duration (ms)"),
            ("elementRenderDelay", "LCP Breakdown Render Delay (ms)"),
        ] {
            let v = parts
                .iter()
                .find(|p| p.get("subpart").and_then(Value::as_str) == Some(subpart))
                .and_then(|p| p.get("duration"))
                .and_then(Value::as_f64)
                .map(f64::round);
            out.num(name, v);
        }
    }

    out.num(
        "Render Blocking Requests Savings (ms)",
        savings_cell(a("render-blocking-insight"), false),
    );
    let tree = a("network-dependency-tree-insight");
    out.num(
        "Preconnect Candidates Savings (ms)",
        savings_cell(tree, false),
    );
    out.num(
        "Maximum Critical Path Latency (ms)",
        tree.and_then(|t| find_key(t, "longestChain"))
            .and_then(|c| c.get("duration"))
            .and_then(Value::as_f64)
            .map(f64::round),
    );
    out.num(
        "Use Efficient Cache Lifetimes Savings (Bytes)",
        savings_cell(a("cache-insight"), true),
    );
    out.num(
        "Layout Shift Culprits",
        row_count(a("cls-culprits-insight")),
    );
    out.num(
        "Improve Image Delivery Savings (Bytes)",
        savings_cell(a("image-delivery-insight"), true),
    );
    out.num(
        "Forced Reflow Savings (ms)",
        savings_cell(a("forced-reflow-insight"), false),
    );
    out.num(
        "Legacy JavaScript Savings (Bytes)",
        savings_cell(a("legacy-javascript-insight"), true),
    );
    out.num(
        "Duplicated JavaScript Savings (Bytes)",
        savings_cell(a("duplicated-javascript-insight"), true),
    );
    out.num("3rd Parties", row_count(a("third-parties-insight")));
    out.num(
        "Network Payload Size (Bytes)",
        numeric(a("total-byte-weight")).map(f64::round),
    );
}

fn dom_elements(audit: Option<&Value>) -> Option<f64> {
    let audit = audit?;
    let items = audit.pointer("/details/items").and_then(Value::as_array);
    let value = |i: &Value| {
        i.pointer("/value/value")
            .or_else(|| i.get("value"))
            .and_then(Value::as_f64)
    };
    items
        .and_then(|items| {
            items
                .iter()
                .find(|i| {
                    i.get("statistic")
                        .and_then(Value::as_str)
                        .is_some_and(|s| s.to_ascii_lowercase().contains("total"))
                })
                .or_else(|| items.first())
                .and_then(value)
        })
        .or_else(|| numeric(Some(audit)))
        .map(f64::round)
}

fn diagnostics<'a>(a: &impl Fn(&str) -> Option<&'a Value>, out: &mut Outcome) {
    for (name, id) in [
        ("Minify CSS", "unminified-css"),
        ("Minify JavaScript", "unminified-javascript"),
        ("Reduce Unused CSS", "unused-css-rules"),
        ("Reduce Unused JavaScript", "unused-javascript"),
    ] {
        out.num(&format!("{name} Savings (ms)"), savings_cell(a(id), false));
        out.num(
            &format!("{name} Savings (Bytes)"),
            savings_cell(a(id), true),
        );
    }
    out.num(
        "JavaScript Execution Time (sec)",
        secs(numeric(a("bootup-time"))),
    );
    out.text(
        "JavaScript Execution Time Category",
        category(a("bootup-time")),
    );
    out.num(
        "Efficient Cache Policy Savings (Bytes)",
        savings_cell(a("uses-long-cache-ttl"), true),
    );
    out.num(
        "Minimize Main-Thread Work (sec)",
        secs(numeric(a("mainthread-work-breakdown"))),
    );
    out.text(
        "Minimize Main-Thread Work Category",
        category(a("mainthread-work-breakdown")),
    );
    out.num("DOM Size", dom_elements(a("dom-size-insight")));
    out.num(
        "Font Display Savings (ms)",
        savings_cell(a("font-display-insight"), false),
    );
    out.num(
        "User Timing Marks and Measures",
        row_count(a("user-timings")),
    );
}

/// Filterergebnisse (INTEG-139) als `fail:<Filter>` = 1/0; fehlt das Audit, entfällt das Ergebnis.
fn fails<'a>(a: &impl Fn(&str) -> Option<&'a Value>, out: &mut Outcome) {
    let failed = |id: &str| a(id).map(|x| !passed(x));
    let ms_over = |id: &str| a(id).map(|x| savings_ms(x).unwrap_or(0.0) > 0.0);
    let bytes_over = |id: &str| a(id).map(|x| savings_bytes(x).unwrap_or(0.0) > 0.0);
    let any_over = |id: &str| {
        a(id).map(|x| savings_ms(x).unwrap_or(0.0) > 0.0 || savings_bytes(x).unwrap_or(0.0) > 0.0)
    };
    let not_good = |id: &str| category(a(id)).map(|c| c != "Good");
    let results = [
        (
            "Document Request Latency",
            failed("document-latency-insight"),
        ),
        ("LCP Request Discovery", failed("lcp-discovery-insight")),
        (
            "Render Blocking Requests",
            ms_over("render-blocking-insight"),
        ),
        (
            "Network Dependency Tree",
            failed("network-dependency-tree-insight"),
        ),
        ("Use Efficient Cache Lifetimes", bytes_over("cache-insight")),
        (
            "Layout Shift Culprits",
            row_count(a("cls-culprits-insight")).map(|n| n > 0.0),
        ),
        (
            "Improve Image Delivery",
            bytes_over("image-delivery-insight"),
        ),
        ("Forced Reflow", failed("forced-reflow-insight")),
        ("Legacy JavaScript", bytes_over("legacy-javascript-insight")),
        (
            "Duplicated JavaScript",
            bytes_over("duplicated-javascript-insight"),
        ),
        (
            "Avoid Enormous Network Payloads",
            a("total-byte-weight").and_then(score).map(|s| s < 0.9),
        ),
        ("Minify CSS", any_over("unminified-css")),
        ("Minify JavaScript", any_over("unminified-javascript")),
        ("Reduce Unused CSS", any_over("unused-css-rules")),
        ("Reduce Unused JavaScript", any_over("unused-javascript")),
        ("Reduce JavaScript Execution Time", not_good("bootup-time")),
        (
            "Minimize Main-Thread Work",
            not_good("mainthread-work-breakdown"),
        ),
        ("Optimize DOM Size", failed("dom-size-insight")),
        ("Font Display", ms_over("font-display-insight")),
    ];
    for (filter, result) in results {
        out.num(
            &fail_metric(filter),
            result.map(|f| if f { 1.0 } else { 0.0 }),
        );
    }
}

/// CrUX-Kategorie der API → Spaltenwert (INTEG-135).
pub fn crux_category(api: &str) -> Option<&'static str> {
    match api {
        "FAST" => Some("Good"),
        "AVERAGE" => Some("Needs Improvement"),
        "SLOW" => Some("Poor"),
        _ => None,
    }
}

/// CrUX-Felddaten für URL und Origin (INTEG-135). Origin-Rückfalldaten in `loadingExperience`
/// (`origin_fallback: true`) gelten nicht als URL-Daten.
pub fn parse_crux(body: &Value) -> Vec<(String, Cell)> {
    let mut out = Outcome::default();
    let url = body
        .get("loadingExperience")
        .filter(|l| l.get("origin_fallback").and_then(Value::as_bool) != Some(true));
    crux_block(url, "CrUX", "Core Web Vitals Assessment", &mut out);
    crux_block(
        body.get("originLoadingExperience"),
        "CrUX Origin",
        "CrUX Origin Core Web Vitals Assessment",
        &mut out,
    );
    out.cells
}

fn crux_block(block: Option<&Value>, prefix: &str, assessment: &str, out: &mut Outcome) {
    let Some(metrics) = block.and_then(|b| b.get("metrics")) else {
        return;
    };
    let get = |key: &str| {
        let m = metrics.get(key)?;
        let p = m.get("percentile").and_then(Value::as_f64)?;
        let c = m
            .get("category")
            .and_then(Value::as_str)
            .and_then(crux_category);
        Some((p, c))
    };
    let fcp = get("FIRST_CONTENTFUL_PAINT_MS");
    let lcp = get("LARGEST_CONTENTFUL_PAINT_MS");
    let cls = get("CUMULATIVE_LAYOUT_SHIFT_SCORE");
    let inp = get("INTERACTION_TO_NEXT_PAINT");
    let ttfb = get("EXPERIMENTAL_TIME_TO_FIRST_BYTE");
    // Pass: LCP, CLS und (falls vorhanden) INP Good; leer ohne LCP oder CLS.
    let verdict = match (lcp.and_then(|m| m.1), cls.and_then(|m| m.1)) {
        (Some(l), Some(c)) => {
            let inp_ok = inp.and_then(|m| m.1).is_none_or(|i| i == "Good");
            Some(if l == "Good" && c == "Good" && inp_ok {
                "Pass"
            } else {
                "Fail"
            })
        }
        _ => None,
    };
    out.text(assessment, verdict);
    let mut put =
        |name: &str, metric: Option<(f64, Option<&'static str>)>, conv: fn(f64) -> f64| {
            out.num(&format!("{prefix} {name}"), metric.map(|m| conv(m.0)));
            let label = name
                .rsplit_once(" (")
                .map_or(name, |(base, _)| base)
                .to_string();
            out.text(
                &format!("{prefix} {label} Category"),
                metric.and_then(|m| m.1),
            );
        };
    put("First Contentful Paint (sec)", fcp, |ms| {
        round_to(ms / 1000.0, 2)
    });
    put("Largest Contentful Paint (sec)", lcp, |ms| {
        round_to(ms / 1000.0, 2)
    });
    // CLS: Die API liefert das Perzentil × 100.
    put("Cumulative Layout Shift", cls, |p| round_to(p / 100.0, 2));
    put("Interaction to Next Paint (ms)", inp, f64::round);
    put("Time to First Byte (ms)", ttfb, f64::round);
}
