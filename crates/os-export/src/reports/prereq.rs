//! Voraussetzungen der Reports (OUT-112): Hinweise statt kommentarlos leerer Dateien.
//!
//! Fehlt eine Voraussetzung, bricht der Export nicht ab; er liefert eine Warnung je betroffenem Report
//! (CLI: `report 'Cookie Summary' requires cookie extraction`). Reports, deren Daten erst die Crawl-Analyse
//! liefert, nennen die betroffenen Filter und verweisen auf `os_analyze::analysis::ensure_analysis`, das die
//! CLI vor analyseabhängigen Exporten aufruft (P26); die Bibliothek führt die Analyse nicht selbst aus.

use os_analyze::names::config_value;
use os_analyze::views::FilterCount;
use os_model::{CrawlConfig, FilterState};
use serde_json::Value as Json;

use crate::catalog::reports::ReportRequirement;

/// Höchstzahl der genannten Beispiele je Hinweis.
const EXAMPLES: usize = 5;

/// Wie [`pending_analysis_filters`] über `os_analyze::views::filter_counts`, aber ohne zu zählen: Ob ein Filter
/// auf die Crawl-Analyse wartet, hängt nur von Konfiguration und Analysezustand ab. Der Export-Katalog fragt das
/// bei jedem Datenstand eines laufenden Crawls ab; eine volle Zählung aller Filter je Abfrage hielt dort dauerhaft
/// einen Kern und eine Leseverbindung belegt (M2-N25, UI-145, CRAWL-130).
pub fn pending_analysis_for_file(
    file: &os_store::CrawlFile,
    cfg: &CrawlConfig,
) -> Result<Vec<String>, os_analyze::AnalyzeError> {
    let analysis = os_analyze::state::AnalysisSnapshot::load(file)?;
    let cfg_json = serde_json::to_value(cfg).unwrap_or(Json::Null);
    let mut states = Vec::new();
    for tab in os_analyze::tabs::configured_for_file(cfg, file) {
        // Stub-Tabs und Stub-Filter zählt `filter_counts` nicht (dort `countable`).
        if tab.is_stub() {
            continue;
        }
        for def in tab.filters.iter().filter(|f| !f.is_stub()) {
            let mut state = os_analyze::state::filter_state(def, cfg, &cfg_json, &analysis);
            if state.yields_rows() {
                if let Some(required) = os_analyze::tabs::dynamic_api::orphan_state(
                    def, &tab, cfg, &cfg_json, &analysis,
                ) {
                    state = required;
                }
            }
            states.push(FilterCount {
                tab: tab.id.to_string(),
                filter: def.id.to_string(),
                label: os_analyze::names::label_with(def.id, &cfg_json),
                count: None,
                pct: None,
                state,
                version: 0,
            });
        }
    }
    Ok(pending_analysis_filters(&states))
}

/// Filter, die wegen der Crawl-Analyse leer sind (`NeedsCrawlAnalysis`/`Stale`), als `Tab:Filter`.
pub fn pending_analysis_filters(counts: &[FilterCount]) -> Vec<String> {
    counts
        .iter()
        .filter(|c| {
            matches!(
                c.state,
                FilterState::NeedsCrawlAnalysis | FilterState::Stale
            )
        })
        .map(|c| format!("{}:{}", c.tab, c.label))
        .collect()
}

/// Warnungen zu den Voraussetzungen eines Reports.
///
/// * `CrawlAnalysis`: `pending_analysis` sind die davon abhängigen, noch leeren Filter (leer = nichts zu melden).
/// * `Config`: Hinweis, wenn der Schlüssel in `cfg` nicht `true` ist.
/// * `Rendering`: Hinweis, wenn das Rendering aus ist.
pub fn requirement_warnings(
    name: &str,
    requires: &[ReportRequirement],
    cfg: &CrawlConfig,
    pending_analysis: &[String],
) -> Vec<String> {
    let cfg_json = serde_json::to_value(cfg).unwrap_or(Json::Null);
    let mut out = Vec::new();
    for req in requires {
        match req {
            ReportRequirement::CrawlAnalysis => {
                if !pending_analysis.is_empty() {
                    let shown: Vec<&str> = pending_analysis
                        .iter()
                        .take(EXAMPLES)
                        .map(String::as_str)
                        .collect();
                    let more = pending_analysis.len().saturating_sub(EXAMPLES);
                    let tail = if more > 0 {
                        format!(" and {more} more")
                    } else {
                        String::new()
                    };
                    out.push(format!(
                        "report '{}' requires the crawl analysis: {} filter(s) stay empty until it has run ({}{}); \
                         run the crawl analysis first",
                        name,
                        pending_analysis.len(),
                        shown.join(", "),
                        tail
                    ));
                }
            }
            ReportRequirement::Config { key, what } => {
                let on = matches!(config_value(&cfg_json, key), Some(Json::Bool(true)));
                if !on {
                    out.push(format!("report '{name}' requires {what}"));
                }
            }
            ReportRequirement::SitemapProcessed | ReportRequirement::OrphanSourceAvailable => {}
            ReportRequirement::Rendering => {
                if cfg.rendering.mode != os_model::RenderingMode::JavaScript {
                    out.push(format!("report '{name}' requires JavaScript rendering"));
                }
            }
        }
    }
    out
}

/// Verfügbarkeit einschließlich dateibezogener Voraussetzungen (OUT-057/099).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Availability {
    pub enabled: bool,
    pub reason: Option<String>,
}
pub fn availability(
    name: &str,
    requires: &[ReportRequirement],
    file: &os_store::CrawlFile,
    cfg: &CrawlConfig,
) -> Availability {
    let pending = if requires.contains(&ReportRequirement::CrawlAnalysis) {
        // Ohne Zählung: wird je Report aufgerufen, eine Vollzählung je Aufruf kostete N Zählungen je Export.
        pending_analysis_for_file(file, cfg)
            .unwrap_or_else(|_| vec!["analysis state unavailable".into()])
    } else {
        Vec::new()
    };
    availability_with_pending(name, requires, file, cfg, &pending)
}

/// Catalog callers can share the analysis-filter snapshot across all reports.
/// The snapshot must come from the same file/configuration as the requirements.
pub fn availability_with_pending(
    name: &str,
    requires: &[ReportRequirement],
    file: &os_store::CrawlFile,
    cfg: &CrawlConfig,
    pending_analysis: &[String],
) -> Availability {
    let mut warnings = requirement_warnings(name, requires, cfg, pending_analysis);
    warnings.extend(crate::catalog::bulk_web::availability_warnings(name, file));
    for req in requires {
        let missing = match req {
            ReportRequirement::SitemapProcessed => !file
                .conn()
                .query_row("SELECT EXISTS(SELECT 1 FROM sitemaps)", [], |r| {
                    r.get::<_, bool>(0)
                })
                .unwrap_or(false),
            ReportRequirement::OrphanSourceAvailable => {
                !(cfg.api.ga4.enabled
                    || cfg.api.gsc.enabled
                    || file
                        .conn()
                        .query_row("SELECT EXISTS(SELECT 1 FROM sitemaps)", [], |r| {
                            r.get::<_, bool>(0)
                        })
                        .unwrap_or(false))
            }
            _ => false,
        };
        if missing {
            warnings.push(format!("report '{name}' requires {req:?}"));
        }
    }
    Availability {
        enabled: warnings.is_empty(),
        reason: (!warnings.is_empty()).then(|| warnings.join("; ")),
    }
}
