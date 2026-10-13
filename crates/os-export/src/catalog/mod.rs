//! Namensregister der Exporte: Tab-Exporte, Bulk-Exporte und Reports.
//!
//! * Tab-Exporte: `Tab:Filter` aus der Registry von `os-analyze` (`tabs::all()`), Auflösung über
//!   `os_analyze::names` (case-insensitiv, Levenshtein-Vorschläge).
//! * Bulk-Exporte: `bulk_links` (P16: Links, Response-Codes-Inlinks); `bulk_issues` (P17) ergänzt sich in
//!   [`resolve`]/[`list`] an den mit `P17:` markierten Stellen.
//! * Reports: `reports` (P17), in dieser Fassung noch ohne Einträge.

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
            // P17: bulk_issues::find(name) vor dem Fehlerfall ergänzen.
            bulk_links::resolve(name).map(|e| ExportRef::Bulk(e.name.clone()))
        }
        ExportKind::Report => {
            // P17: reports::find(name) ergänzen.
            Err(NameError::Unknown {
                input: name.to_string(),
                suggestions: Vec::new(),
            })
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
            // P17: bulk_issues::names() anhängen.
            bulk_links::names()
        }
        ExportKind::Report => Vec::new(),
    }
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
