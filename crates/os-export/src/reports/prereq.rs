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
            ReportRequirement::Rendering => {
                if cfg.rendering.mode != os_model::RenderingMode::JavaScript {
                    out.push(format!("report '{name}' requires JavaScript rendering"));
                }
            }
        }
    }
    out
}
