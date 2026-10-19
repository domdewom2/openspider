//! Response Codes: Exporte der M2-Filter (OUT-050, OUT-067); die Inlinks-Exporte der drei Bereiche stehen seit M1 in `bulk_links`, D12 ergänzt hier weitere.
//!
//! Die Fehler-Inlinks verwenden die gemeinsame Linkauswertung und deren Fehler-Sortierung.

use crate::catalog::TableEntry;

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

const ENTRIES: &[TableEntry] = &[
    // noch keine Einträge
];

/// Vereinigung der drei Fehlerfilter mit der Sortierung nach Code, Ziel und Quelle (OUT-067).
pub fn error_inlinks(scope: &str, prefix: &str) -> super::bulk_links::BulkLinkEntry {
    // Spec-Kurzpfad zusätzlich anbieten; Katalogname und Dateiname bleiben kompatibel.
    let combined = scope == "Internal & External";
    super::bulk_links::BulkLinkEntry {
        name: format!("Response Codes:{scope}:All Error Inlinks"),
        aliases: if combined {
            vec!["Response Codes:All Error Inlinks".into()]
        } else {
            Vec::new()
        },
        selection: super::bulk_links::Selection::Filters {
            tab: "Response Codes".into(),
            filters: ["No Response", "Client Error (4XX)", "Server Error (5XX)"]
                .iter()
                .map(|s| format!("{prefix}{s}"))
                .collect(),
        },
        order: crate::links_export::LinkOrder::ErrorInlinks,
        requires_rendering: false,
    }
}
