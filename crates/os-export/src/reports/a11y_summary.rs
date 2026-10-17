//! Accessibility: Violations Summary (OUT-109).
//!
//! STUB(M2-D21): Alle Einträge liefern bis zur Umsetzung nur die Kopfzeile (K-14, `stub_run`). Das Paket ersetzt `run`
//! je Eintrag durch die Auswertung und ergänzt oder schärft Kopfzeile, Voraussetzungen und Aliase; der Katalog in
//! `catalog/mod.rs` bleibt unverändert, weil er nur `entries()` aufruft.

use crate::catalog::reports::ReportRequirement;
use crate::catalog::{stub_run, TableEntry};

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

// STUB(M2-D21)
const ENTRIES: &[TableEntry] = &[
    TableEntry {
        name: "Accessibility:Violations Summary",
        aliases: &[],
        file_stem: "Accessibility:Violations Summary",
        header: &["Issue", "Rule ID", "Guideline", "Impact", "Priority", "URLs", "% of Total", "Sample URL"],
        requires: &[ReportRequirement::Rendering, ReportRequirement::Config { key: "extraction.accessibility", what: "accessibility extraction" }],
        owner: "M2-D21",
        run: stub_run,
    },
];
