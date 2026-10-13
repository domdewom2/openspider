//! Namensregister der Exporte: Tab-Exporte, Bulk-Exporte und Reports.
//!
//! * Tab-Exporte: `Tab:Filter` aus der Registry von `os-analyze` (`tabs::all()`), Auflösung über
//!   `os_analyze::names` (case-insensitiv, Levenshtein-Vorschläge).
//! * Bulk-Exporte: `bulk_links` (Links, Response-Codes-Inlinks) und `bulk_issues` (`Issues:All Issues`).
//! * Reports: `reports` (Crawl Overview, Redirects).
//!
//! Alle Namen einer Art stehen in [`list`]; [`resolve`] löst sie case-insensitiv auf und liefert bei einem
//! Tippfehler Vorschläge über alle Namen der Art (Levenshtein ≤ 3).

pub mod bulk_issues;
pub mod bulk_links;
pub mod reports;

use os_analyze::{tabs, NameError};

/// Art des Exports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportKind {
    /// Tab-Export (`--export-tabs "Tab:Filter"`).
    Tab,
    /// Bulk-Export (`--bulk-export`).
    Bulk,
    /// Report (`--save-report`).
    Report,
}

/// Aufgelöster Export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportRef {
    /// Tab und Filter in kanonischer Schreibweise.
    Tab {
        /// Tab.
        tab: String,
        /// Filter.
        filter: String,
    },
    /// Bulk-Export (kanonischer Name).
    Bulk(String),
    /// Report (kanonischer Name).
    Report(String),
}

/// Löst einen Namen (case-insensitiv) auf.
pub fn resolve(kind: ExportKind, name: &str) -> Result<ExportRef, NameError> {
    match kind {
        ExportKind::Tab => {
            let (tab, filter) = os_analyze::views::resolve_name(name)?;
            Ok(ExportRef::Tab { tab, filter })
        }
        ExportKind::Bulk => {
            if let Some(entry) = bulk_links::find(name) {
                return Ok(ExportRef::Bulk(entry.name.clone()));
            }
            if let Ok(issues) = bulk_issues::resolve(name) {
                return Ok(ExportRef::Bulk(issues.to_string()));
            }
            Err(NameError::Unknown {
                input: name.to_string(),
                suggestions: bulk_suggestions(name),
            })
        }
        ExportKind::Report => {
            reports::resolve(name).map(|def| ExportRef::Report(def.name.to_string()))
        }
    }
}

/// Alle gültigen Namen der Art, für Hilfetexte und Vorschläge. Tab-Namen tragen für Schwellen ein `X`
/// (`Page Titles:Over X Characters`).
pub fn list(kind: ExportKind) -> Vec<String> {
    match kind {
        ExportKind::Tab => tabs::all()
            .iter()
            .flat_map(|tab| {
                tab.filters
                    .iter()
                    .map(|f| format!("{}:{}", tab.id, placeholders_to_x(f.id)))
                    .collect::<Vec<_>>()
            })
            .collect(),
        ExportKind::Bulk => {
            let mut names = bulk_links::names();
            names.extend(bulk_issues::names());
            names
        }
        ExportKind::Report => reports::names(),
    }
}

/// Höchstabstand und -zahl der Vorschläge für Bulk-Namen (wie in `bulk_links`).
const MAX_DISTANCE: usize = 3;
const MAX_SUGGESTIONS: usize = 5;

/// Ähnliche Bulk-Namen über alle Kataloge (Links und Issues), nächste zuerst.
fn bulk_suggestions(input: &str) -> Vec<String> {
    let norm = |s: &str| {
        s.split(':')
            .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect::<Vec<_>>()
            .join(":")
            .to_lowercase()
    };
    let wanted = norm(input);
    let mut scored: Vec<(usize, String)> = list(ExportKind::Bulk)
        .into_iter()
        .filter_map(|n| {
            let d = bulk_links::levenshtein(&wanted, &norm(&n));
            (d <= MAX_DISTANCE).then_some((d, n))
        })
        .collect();
    scored.sort();
    scored.truncate(MAX_SUGGESTIONS);
    scored.into_iter().map(|(_, n)| n).collect()
}

/// `Over {title.max_chars} Characters` → `Over X Characters`.
fn placeholders_to_x(id: &str) -> String {
    let mut out = String::new();
    let mut rest = id;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}') else {
            break;
        };
        out.push_str(&rest[..open]);
        out.push('X');
        rest = &rest[open + close + 1..];
    }
    out.push_str(rest);
    out
}
