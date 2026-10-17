//! Custom Search: Exporte je Suchfilter (OUT-060).
//!
//! STUB(M2-D14): Alle Einträge liefern bis zur Umsetzung nur die Kopfzeile (K-14, `stub_run`). Das Paket ersetzt `run`
//! je Eintrag durch die Auswertung und ergänzt oder schärft Kopfzeile, Voraussetzungen und Aliase; der Katalog in
//! `catalog/mod.rs` bleibt unverändert, weil er nur `entries()` aufruft.

use crate::catalog::{stub_run, TableEntry};

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

// STUB(M2-D14)
const ENTRIES: &[TableEntry] = &[TableEntry {
    name: "Custom Search:All Filters",
    aliases: &[],
    file_stem: "Custom Search:All Filters",
    header: &[
        "Address",
        "Filter",
        "Occurrences",
        "Status Code",
        "Indexability",
    ],
    requires: &[],
    owner: "M2-D14",
    run: stub_run,
    header_fn: None,
}];

/// Erweiterungspunkt für M2-D14.
// STUB(M2-D14)
pub fn dynamic_entries(_cfg: &os_model::CrawlConfig) -> Vec<super::DynamicEntry> {
    Vec::new()
}
