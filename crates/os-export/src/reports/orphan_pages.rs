//! Orphan Pages (OUT-099).
//!
//! STUB(M2-D13): Alle Einträge liefern bis zur Umsetzung nur die Kopfzeile (K-14, `stub_run`). Das Paket ersetzt `run`
//! je Eintrag durch die Auswertung und ergänzt oder schärft Kopfzeile, Voraussetzungen und Aliase; der Katalog in
//! `catalog/mod.rs` bleibt unverändert, weil er nur `entries()` aufruft.

use crate::catalog::reports::ReportRequirement;
use crate::catalog::{stub_run, TableEntry};

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

// STUB(M2-D13)
const ENTRIES: &[TableEntry] = &[
    TableEntry {
        name: "Orphan Pages",
        aliases: &[],
        file_stem: "Orphan Pages",
        header: &["Address", "Source", "Status Code", "Indexability"],
        requires: &[ReportRequirement::CrawlAnalysis],
        owner: "M2-D13",
        run: stub_run,
    },
];
