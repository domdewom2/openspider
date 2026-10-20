//! URL Inspection (`urlInspection/index:inspect`, INTEG-120 … 125, 127): Auswahl der URLs, Tageskontingent und
//! Property je URL, Fehlerbehandlung, die 17 Spalten und die Ableitung von `Summary`.
//!
//! Ablauf von [`Inspector::run`]: Die Ziele kommen in Entdeckungsreihenfolge (Spider: kleinste Crawl-Tiefe, dann
//! früheste Entdeckung; Listenmodus: Listenreihenfolge, [`targets_from_file`]). Nicht-HTML-URLs werden nie abgefragt,
//! mit „Nicht indexierbare auslassen“ auch keine nicht indexierbaren (beides ohne Werte, `Summary` leer). Jede übrige
//! URL bekommt ihre Property (die gewählte oder mit „Mehrere Properties“ die spezifischste); ohne passende Property
//! gilt `Error: URL not in property`, ohne Kontingentverbrauch. Danach reserviert der Inspector je Property auf einmal
//! so viele Abfragen, wie das Tageskontingent noch zulässt; die ersten in Reihenfolge werden abgefragt, die übrigen
//! erhalten `Quota exceeded`. Eine Property weicht nie auf eine andere aus. Meldet der Anbieter das Tageskontingent
//! als erschöpft (429/403 mit Kontingentgrund), steht der lokale Zähler danach auf dem Limit, und keine weitere
//! Anfrage geht an diese Property.

use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, Mutex};

use futures_util::{stream, StreamExt};
use os_model::{ApiValueWrite, Clock, Timestamp, UrlId};
use serde_json::{json, Value};

use super::client::{Endpoint, Failure, GscHttp};
use super::options::{GscOptions, INSPECTION_DAILY_LIMIT};
use super::sites::{self, Property};
use super::{metric, SOURCE};
use crate::quota::QuotaLedger;
use crate::ApiError;

/// Kennung der Quelle im Kontingentzähler (je Property und Pazifiktag).
pub const QUOTA_SOURCE: &str = "gsc.inspection";

/// `Summary`-Werte (INTEG-124).
pub mod summary {
    pub const ON_GOOGLE: &str = "URL is on Google";
    pub const ON_GOOGLE_WITH_ISSUES: &str = "URL is on Google, but has issues";
    pub const NOT_ON_GOOGLE: &str = "URL is not on Google";
    pub const QUOTA_EXCEEDED: &str = "Quota exceeded";
    /// Präfix der Fehlerwerte (`Error: <Meldung>`).
    pub const ERROR_PREFIX: &str = "Error: ";
}

/// Eine zu prüfende URL mit den Fakten aus dem Crawl.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub url_id: Option<UrlId>,
    pub url: String,
    /// HTML-Seite (nur diese werden abgefragt).
    pub is_html: bool,
    /// Indexierbar nach OpenSpiders Bewertung.
    pub indexable: bool,
}

/// Die 17 Spalten einer Inspection-Antwort (INTEG-123); leere Teilergebnisse bleiben `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InspectionFields {
    pub summary: Option<String>,
    pub coverage: Option<String>,
    pub last_crawl: Option<String>,
    pub crawled_as: Option<String>,
    pub crawl_allowed: Option<String>,
    pub page_fetch: Option<String>,
    pub indexing_allowed: Option<String>,
    pub user_canonical: Option<String>,
    pub google_canonical: Option<String>,
    pub mobile_usability: Option<String>,
    pub mobile_usability_issues: Option<String>,
    pub amp_results: Option<String>,
    pub amp_issues: Option<String>,
    pub rich_results: Option<String>,
    pub rich_results_types: Option<String>,
    pub rich_results_types_errors: Option<String>,
    pub rich_results_warnings: Option<String>,
}

impl InspectionFields {
    /// Weichen User-Declared und Google-Selected Canonical nach URL-Normalisierung voneinander ab? `None`, wenn eines
    /// der beiden fehlt (dann greift der Filter „User-Declared Canonical Not Selected“ nicht, INTEG-125).
    pub fn canonical_not_selected(&self) -> Option<bool> {
        let normalize = |u: &str| {
            url::Url::parse(u.trim())
                .map(|u| u.to_string())
                .unwrap_or_else(|_| u.trim().to_string())
        };
        let user = self.user_canonical.as_deref()?;
        let google = self.google_canonical.as_deref()?;
        Some(normalize(user) != normalize(google))
    }

    /// `(Metrik, Spaltenname, Wert)` in Spaltenreihenfolge.
    pub fn columns(&self) -> [(&'static str, &'static str, Option<&str>); 17] {
        [
            (metric::SUMMARY, "Summary", self.summary.as_deref()),
            (metric::COVERAGE, "Coverage", self.coverage.as_deref()),
            (metric::LAST_CRAWL, "Last Crawl", self.last_crawl.as_deref()),
            (metric::CRAWLED_AS, "Crawled As", self.crawled_as.as_deref()),
            (
                metric::CRAWL_ALLOWED,
                "Crawl Allowed",
                self.crawl_allowed.as_deref(),
            ),
            (metric::PAGE_FETCH, "Page Fetch", self.page_fetch.as_deref()),
            (
                metric::INDEXING_ALLOWED,
                "Indexing Allowed",
                self.indexing_allowed.as_deref(),
            ),
            (
                metric::USER_CANONICAL,
                "User-Declared Canonical",
                self.user_canonical.as_deref(),
            ),
            (
                metric::GOOGLE_CANONICAL,
                "Google-Selected Canonical",
                self.google_canonical.as_deref(),
            ),
            (
                metric::MOBILE_USABILITY,
                "Mobile Usability",
                self.mobile_usability.as_deref(),
            ),
            (
                metric::MOBILE_USABILITY_ISSUES,
                "Mobile Usability Issues",
                self.mobile_usability_issues.as_deref(),
            ),
            (
                metric::AMP_RESULTS,
                "AMP Results",
                self.amp_results.as_deref(),
            ),
            (metric::AMP_ISSUES, "AMP Issues", self.amp_issues.as_deref()),
            (
                metric::RICH_RESULTS,
                "Rich Results",
                self.rich_results.as_deref(),
            ),
            (
                metric::RICH_RESULTS_TYPES,
                "Rich Results Types",
                self.rich_results_types.as_deref(),
            ),
            (
                metric::RICH_RESULTS_TYPES_ERRORS,
                "Rich Results Types Errors",
                self.rich_results_types_errors.as_deref(),
            ),
            (
                metric::RICH_RESULTS_WARNINGS,
                "Rich Results Warnings",
                self.rich_results_warnings.as_deref(),
            ),
        ]
    }
}

/// Ergebnis für eine URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Nicht abgefragt (Nicht-HTML bzw. nicht indexierbar mit Option): keine Werte, `Summary` leer.
    Skipped,
    /// Abgefragt.
    Inspected {
        property: String,
        fields: Box<InspectionFields>,
    },
    /// Kontingent der Property erschöpft.
    QuotaExceeded { property: String },
    /// Fehler für diese URL (`Summary = Error: <Meldung>`).
    Error {
        property: Option<String>,
        message: String,
    },
}

impl Outcome {
    /// Der `Summary`-Wert.
    pub fn summary(&self) -> Option<String> {
        match self {
            Outcome::Skipped => None,
            Outcome::Inspected { fields, .. } => fields.summary.clone(),
            Outcome::QuotaExceeded { .. } => Some(summary::QUOTA_EXCEEDED.into()),
            Outcome::Error { message, .. } => Some(format!("{}{message}", summary::ERROR_PREFIX)),
        }
    }

    /// Verwendete Property (Spalte `Inspection Property`).
    pub fn property(&self) -> Option<&str> {
        match self {
            Outcome::Skipped => None,
            Outcome::Inspected { property, .. } | Outcome::QuotaExceeded { property } => {
                Some(property)
            }
            Outcome::Error { property, .. } => property.as_deref(),
        }
    }

    /// Werte für `api_values` (Quelle `gsc`; Texte, dazu der Canonical-Vergleich als Zahl).
    pub fn values(&self, target: &Target, fetched_at: Timestamp) -> Vec<ApiValueWrite> {
        let value = |metric: &str, v_real: Option<f64>, v_text: Option<&str>| ApiValueWrite {
            url_id: target.url_id,
            url: target.url.clone(),
            source: SOURCE.into(),
            metric: metric.into(),
            level: "url".into(),
            v_real,
            v_text: v_text.map(str::to_string),
            fetched_at,
        };
        let mut texts: Vec<(&str, String)> = Vec::new();
        let mut out = Vec::new();
        match self {
            Outcome::Skipped => {}
            Outcome::Inspected { fields, .. } => {
                for (metric, _, v) in fields.columns() {
                    if let Some(v) = v {
                        texts.push((metric, v.to_string()));
                    }
                }
                if let Some(differs) = fields.canonical_not_selected() {
                    out.push(value(
                        metric::CANONICAL_NOT_SELECTED,
                        Some(if differs { 1.0 } else { 0.0 }),
                        None,
                    ));
                }
            }
            _ => texts.extend(self.summary().map(|s| (metric::SUMMARY, s))),
        }
        if let Some(property) = self.property() {
            texts.push((metric::INSPECTION_PROPERTY, property.to_string()));
        }
        out.extend(
            texts
                .iter()
                .filter(|(_, v)| !v.is_empty())
                .map(|(metric, v)| value(metric, None, Some(v))),
        );
        out
    }
}

/// Verdikt lesbar (INTEG-123): PASS → Pass, PARTIAL → Partial, FAIL → Fail, NEUTRAL → Neutral.
pub fn verdict_label(verdict: &str) -> Option<&'static str> {
    match verdict {
        "PASS" => Some("Pass"),
        "PARTIAL" => Some("Partial"),
        "FAIL" => Some("Fail"),
        "NEUTRAL" => Some("Neutral"),
        _ => None,
    }
}

fn page_fetch_label(state: &str) -> Option<&'static str> {
    Some(match state {
        "SUCCESSFUL" => "Successful",
        "SOFT_404" => "Soft 404",
        "BLOCKED_ROBOTS_TXT" => "Blocked by robots.txt",
        "NOT_FOUND" => "Not found (404)",
        "ACCESS_DENIED" => "Access denied (401)",
        "SERVER_ERROR" => "Server error (5xx)",
        "REDIRECT_ERROR" => "Redirect error",
        "ACCESS_FORBIDDEN" => "Access denied (403)",
        "BLOCKED_4XX" => "Blocked (4xx)",
        "INTERNAL_CRAWL_ERROR" => "Internal crawl error",
        "INVALID_URL" => "Invalid URL",
        _ => return None,
    })
}

fn indexing_label(state: &str) -> Option<&'static str> {
    Some(match state {
        "INDEXING_ALLOWED" => "Yes",
        "BLOCKED_BY_META_TAG" => "No: 'noindex' detected in 'robots' meta tag",
        "BLOCKED_BY_HTTP_HEADER" => "No: 'noindex' detected in 'X-Robots-Tag' http header",
        "BLOCKED_BY_ROBOTS_TXT" => "No: blocked by robots.txt",
        _ => return None,
    })
}

fn text(v: &Value) -> Option<String> {
    v.as_str().filter(|s| !s.is_empty()).map(str::to_string)
}

fn verdict(v: &Value) -> Option<String> {
    v.as_str().and_then(verdict_label).map(str::to_string)
}

fn joined(items: impl Iterator<Item = String>) -> Option<String> {
    let mut seen = Vec::<String>::new();
    for item in items {
        if !item.is_empty() && !seen.contains(&item) {
            seen.push(item);
        }
    }
    (!seen.is_empty()).then(|| seen.join("; "))
}

/// `Summary` aus der Antwort (INTEG-124).
pub fn derive_summary(result: &Value) -> &'static str {
    if result
        .pointer("/indexStatusResult/verdict")
        .and_then(Value::as_str)
        != Some("PASS")
    {
        return summary::NOT_ON_GOOGLE;
    }
    let has_issues = ["mobileUsabilityResult", "ampResult", "richResultsResult"]
        .iter()
        .any(|part| matches!(result[*part]["verdict"].as_str(), Some("FAIL" | "PARTIAL")));
    if has_issues {
        summary::ON_GOOGLE_WITH_ISSUES
    } else {
        summary::ON_GOOGLE
    }
}

/// Liest eine Antwort von `index:inspect` in die 17 Spalten (INTEG-123).
pub fn parse_response(response: &Value) -> InspectionFields {
    let r = &response["inspectionResult"];
    let index = &r["indexStatusResult"];
    let mobile = &r["mobileUsabilityResult"];
    let amp = &r["ampResult"];
    let rich = &r["richResultsResult"];
    let items = rich["detectedItems"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let types_with = |severity: &str| {
        joined(items.iter().filter_map(|d| {
            let hit = d["items"].as_array().is_some_and(|items| {
                items.iter().any(|item| {
                    item["issues"].as_array().is_some_and(|issues| {
                        issues
                            .iter()
                            .any(|i| i["severity"].as_str() == Some(severity))
                    })
                })
            });
            hit.then(|| text(&d["richResultType"])).flatten()
        }))
    };
    let list = |v: &Value, key: &str| {
        v.as_array()
            .and_then(|a| joined(a.iter().filter_map(|i| text(&i[key]))))
    };
    InspectionFields {
        summary: Some(derive_summary(r).to_string()),
        coverage: text(&index["coverageState"]),
        last_crawl: text(&index["lastCrawlTime"]),
        crawled_as: match index["crawledAs"].as_str() {
            Some("DESKTOP") => Some("Desktop".into()),
            Some("MOBILE") => Some("Mobile".into()),
            _ => None,
        },
        crawl_allowed: match index["robotsTxtState"].as_str() {
            Some("ALLOWED") => Some("Yes".into()),
            Some("DISALLOWED") => Some("No".into()),
            _ => None,
        },
        page_fetch: index["pageFetchState"]
            .as_str()
            .and_then(page_fetch_label)
            .map(str::to_string),
        indexing_allowed: index["indexingState"]
            .as_str()
            .and_then(indexing_label)
            .map(str::to_string),
        user_canonical: text(&index["userCanonical"]),
        google_canonical: text(&index["googleCanonical"]),
        mobile_usability: verdict(&mobile["verdict"]),
        mobile_usability_issues: list(&mobile["issues"], "issueType"),
        amp_results: verdict(&amp["verdict"]),
        amp_issues: list(&amp["issues"], "issueMessage"),
        rich_results: verdict(&rich["verdict"]),
        rich_results_types: joined(items.iter().filter_map(|d| text(&d["richResultType"]))),
        rich_results_types_errors: types_with("ERROR"),
        rich_results_warnings: types_with("WARNING"),
    }
}

/// Führt die URL Inspection für eine Menge Ziele aus.
pub struct Inspector<'a> {
    pub http: &'a GscHttp,
    pub ledger: &'a QuotaLedger,
    pub clock: Arc<dyn Clock>,
    pub options: &'a GscOptions,
    /// Properties des Kontos (für „Mehrere Properties“ und die Zuordnungsprüfung).
    pub properties: &'a [Property],
    /// Tageskontingent je Property (Standard 2 000).
    pub daily_limit: u64,
}

impl<'a> Inspector<'a> {
    /// Inspector mit Standardkontingent.
    pub fn new(
        http: &'a GscHttp,
        ledger: &'a QuotaLedger,
        clock: Arc<dyn Clock>,
        options: &'a GscOptions,
        properties: &'a [Property],
    ) -> Inspector<'a> {
        Inspector {
            http,
            ledger,
            clock,
            options,
            properties,
            daily_limit: INSPECTION_DAILY_LIMIT,
        }
    }

    /// Property für eine URL (INTEG-122) oder `None`, wenn die URL zu keiner Property gehört.
    pub fn property_for(&self, url: &str) -> Option<Property> {
        if self.options.multiple_properties {
            return sites::most_specific(url, self.properties).cloned();
        }
        let site = self.options.site.as_deref()?;
        let property = self
            .properties
            .iter()
            .find(|p| p.site_url == site)
            .cloned()
            .unwrap_or_else(|| Property::new(site, "siteOwner"));
        property.covers(url).then_some(property)
    }

    /// Prüft alle Ziele; das Ergebnis hat dieselbe Reihenfolge wie `targets`.
    pub async fn run(&self, targets: &[Target]) -> Result<Vec<Outcome>, ApiError> {
        let mut outcomes: Vec<Option<Outcome>> = vec![None; targets.len()];
        // Je Property die Ziele in Reihenfolge.
        let mut by_property: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (i, t) in targets.iter().enumerate() {
            if !t.is_html || (self.options.skip_non_indexable && !t.indexable) {
                outcomes[i] = Some(Outcome::Skipped);
                continue;
            }
            match self.property_for(&t.url) {
                Some(p) => by_property.entry(p.site_url).or_default().push(i),
                None => {
                    outcomes[i] = Some(Outcome::Error {
                        property: None,
                        message: Failure::Forbidden.message(),
                    })
                }
            }
        }
        let now = self.clock.now();
        let mut pending: Vec<(usize, String)> = Vec::new();
        for (property, indices) in &by_property {
            let granted = self.ledger.reserve_up_to(
                QUOTA_SOURCE,
                property,
                now,
                self.daily_limit,
                indices.len() as u64,
            )? as usize;
            for (n, &i) in indices.iter().enumerate() {
                if n < granted {
                    pending.push((i, property.clone()));
                } else {
                    outcomes[i] = Some(Outcome::QuotaExceeded {
                        property: property.clone(),
                    });
                }
            }
        }
        // In Entdeckungsreihenfolge senden.
        pending.sort_by_key(|(i, _)| *i);
        let exhausted: Mutex<HashSet<String>> = Mutex::default();
        let concurrency = self.options.limits.concurrency.clamp(1, 50) as usize;
        let results: Vec<(usize, Outcome)> = stream::iter(pending)
            .map(|(i, property)| {
                let exhausted = &exhausted;
                async move {
                    let outcome = self
                        .inspect_one(&targets[i].url, &property, exhausted)
                        .await;
                    (i, outcome)
                }
            })
            .buffer_unordered(concurrency)
            .collect()
            .await;
        for (i, outcome) in results {
            outcomes[i] = Some(outcome);
        }
        let exhausted = exhausted.into_inner().unwrap_or_else(|p| p.into_inner());
        for property in exhausted {
            self.ledger
                .exhaust(QUOTA_SOURCE, &property, self.clock.now(), self.daily_limit)?;
        }
        Ok(outcomes
            .into_iter()
            .map(|o| o.unwrap_or(Outcome::Skipped))
            .collect())
    }

    async fn inspect_one(
        &self,
        url: &str,
        property: &str,
        exhausted: &Mutex<HashSet<String>>,
    ) -> Outcome {
        let quota = || Outcome::QuotaExceeded {
            property: property.to_string(),
        };
        if exhausted
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .contains(property)
        {
            return quota();
        }
        let mut body = json!({ "inspectionUrl": url, "siteUrl": property });
        if let Some(language) = &self.options.language_code {
            body["languageCode"] = json!(language);
        }
        match self
            .http
            .post(
                Endpoint::Inspection,
                "/v1/urlInspection/index:inspect",
                &body,
                Some(property),
            )
            .await
        {
            Ok(response) => Outcome::Inspected {
                property: property.to_string(),
                fields: Box::new(parse_response(&response)),
            },
            Err(Failure::Quota) => {
                exhausted
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .insert(property.to_string());
                quota()
            }
            Err(e) => Outcome::Error {
                property: Some(property.to_string()),
                message: e.message(),
            },
        }
    }
}

/// Ziele aus der Crawl-Datei: interne URLs der Tabellen, im Listenmodus in Listenreihenfolge, sonst nach kleinster
/// Crawl-Tiefe und frühester Entdeckung (INTEG-120).
pub fn targets_from_file(file: &os_store::CrawlFile) -> Result<Vec<Target>, ApiError> {
    let fail = |_| ApiError::Transport("Crawl-Datei nicht lesbar".into());
    let conn = file.conn();
    let list: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM list_input WHERE url_id IS NOT NULL)",
            [],
            |r| r.get(0),
        )
        .map_err(fail)?;
    let sql = if list {
        "SELECT u.id, u.url, u.content_class, u.indexability FROM list_input l JOIN urls u ON u.id = l.url_id \
         WHERE l.outcome = 'ok' ORDER BY l.position"
    } else {
        "SELECT u.id, u.url, u.content_class, u.indexability FROM urls u WHERE u.is_internal = 1 \
         AND u.in_tables = 1 ORDER BY u.crawl_depth IS NULL, u.crawl_depth, u.id"
    };
    let mut stmt = conn.prepare(sql).map_err(fail)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Target {
                url_id: Some(UrlId(r.get(0)?)),
                url: r.get(1)?,
                is_html: r.get::<_, Option<String>>(2)?.as_deref() == Some("HTML"),
                indexable: r.get::<_, Option<String>>(3)?.as_deref() == Some("Indexable"),
            })
        })
        .map_err(fail)?;
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for row in rows {
        let t = row.map_err(fail)?;
        if seen.insert(t.url.clone()) {
            out.push(t);
        }
    }
    Ok(out)
}
