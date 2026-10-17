//! Weitere Tab-Bulk-Exporte (OUT-066): für jeden Filter der Haupt-Tabs, deren Befund an der Ziel-URL hängt, ein Export
//! `<Tab>:<Filter> Inlinks` im Standard-Linkformat.
//!
//! Die Liste entsteht aus `os_analyze::tabs::all()` (Stub-Tabs und Stub-Filter eingeschlossen), damit der Registry-Test
//! „Anzahl Inlinks-Exporte = Anzahl registrierter Filter dieser Tabs“ von selbst gilt, auch wenn Fachpakete Filter
//! ergänzen. Pagination hat eine eigene Gruppe (`bulk_pagination`), die Filter von Links, Canonicals, Directives,
//! Images, Security, Sitemaps und Response Codes ebenfalls.
//!
//! STUB(M2-X01): Alle Einträge liefern bis zur Umsetzung nur die Kopfzeile (K-14, `stub_run`); M2-X01 ersetzt `run`.

use std::sync::OnceLock;

use os_analyze::defs::intern;

use crate::catalog::{stub_run, TableEntry};

/// Tabs, deren Filter einen generischen Inlinks-Export bekommen (OUT-066).
pub const TABS: &[&str] = &[
    "Page Titles",
    "Meta Description",
    "Meta Keywords",
    "H1",
    "H2",
    "Hreflang",
    "Mobile",
    "Validation",
    "Link Metrics",
    "Analytics",
    "Search Console",
    "PageSpeed",
];

/// Die Einträge in Tab- und Filterreihenfolge von `tabs::all()`.
pub fn entries() -> &'static [TableEntry] {
    static ENTRIES: OnceLock<Vec<TableEntry>> = OnceLock::new();
    ENTRIES.get_or_init(|| {
        let mut out = Vec::new();
        for tab in os_analyze::tabs::all() {
            if !TABS.contains(&tab.id) {
                continue;
            }
            for filter in &tab.filters {
                // Die Spaltenkennung trägt Platzhalter (`Over {title.max_chars} Characters`); der Name trägt `X`.
                let name = intern(&format!(
                    "{}:{} Inlinks",
                    tab.id,
                    super::placeholders_to_x(filter.id)
                ));
                out.push(TableEntry {
                    name,
                    aliases: &[],
                    file_stem: name,
                    header: super::LINK_HEADER,
                    header_fn: None,
                    requires: &[],
                    owner: "M2-X01",
                    run: stub_run,
                });
            }
        }
        out
    })
}
