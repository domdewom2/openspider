//! Namensregister der Exporte: Tab-Exporte, Bulk-Exporte und Reports.
//!
//! Die Einträge liefern `bulk_links` (P16) sowie `bulk_issues` und `reports` (P17).
//! STUB(M1-P16): Stub kennt keine Namen.

pub mod bulk_issues;
pub mod bulk_links;
pub mod reports;

use os_analyze::NameError;

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
// STUB(M1-P16)
pub fn resolve(_kind: ExportKind, name: &str) -> Result<ExportRef, NameError> {
    Err(NameError::Unknown {
        input: name.to_string(),
        suggestions: Vec::new(),
    })
}

/// Alle gültigen Namen der Art, für Hilfetexte und Vorschläge.
// STUB(M1-P16)
pub fn list(_kind: ExportKind) -> Vec<String> {
    Vec::new()
}
