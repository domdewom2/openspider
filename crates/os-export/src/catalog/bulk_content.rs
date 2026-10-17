//! Content: Duplikat-, Ähnlichkeits- und Rechtschreib-Exporte (OUT-052).
//!
//! STUB(M2-D02): Alle Einträge liefern bis zur Umsetzung nur die Kopfzeile (K-14, `stub_run`). Das Paket ersetzt `run`
//! je Eintrag durch die Auswertung und ergänzt oder schärft Kopfzeile, Voraussetzungen und Aliase; der Katalog in
//! `catalog/mod.rs` bleibt unverändert, weil er nur `entries()` aufruft.

use crate::catalog::reports::ReportRequirement;
use crate::catalog::{stub_run, TableEntry};

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

// STUB(M2-D02)
const ENTRIES: &[TableEntry] = &[
    TableEntry {
        name: "Content:Exact Duplicates Pairs",
        aliases: &["Content:Exact Duplicates"],
        file_stem: "Content:Exact Duplicates Pairs",
        header: &["Address", "Exact Duplicate Address", "Hash"],
        requires: &[],
        owner: "M2-D02",
        run: stub_run,
    },
    TableEntry {
        name: "Content:Near Duplicates Pairs",
        aliases: &["Content:Near Duplicates"],
        file_stem: "Content:Near Duplicates Pairs",
        header: &["Address", "Near Duplicate Address", "Similarity"],
        requires: &[ReportRequirement::CrawlAnalysis],
        owner: "M2-D02",
        run: stub_run,
    },
    TableEntry {
        name: "Content:Spelling Errors Details",
        aliases: &["Content:Spelling Errors"],
        file_stem: "Content:Spelling Errors Details",
        header: &["Address", "Language", "Type", "Error", "Detail", "Suggestion", "Page Section", "Rule ID"],
        requires: &[ReportRequirement::Config { key: "spelling.enabled", what: "spell checking" }],
        owner: "M2-D02",
        run: stub_run,
    },
    TableEntry {
        name: "Content:Grammar Errors Details",
        aliases: &["Content:Grammar Errors"],
        file_stem: "Content:Grammar Errors Details",
        header: &["Address", "Language", "Type", "Error", "Detail", "Suggestion", "Page Section", "Rule ID"],
        requires: &[ReportRequirement::Config { key: "grammar.enabled", what: "grammar checking" }],
        owner: "M2-D02",
        run: stub_run,
    },
    TableEntry {
        name: "Content:Spelling & Grammar Errors",
        aliases: &[],
        file_stem: "Content:Spelling & Grammar Errors",
        header: &["Address", "Language", "Type", "Error", "Detail", "Suggestion", "Page Section", "Rule ID"],
        requires: &[],
        owner: "M2-D02",
        run: stub_run,
    },
    TableEntry {
        name: "Content:Spelling & Grammar Errors Summary",
        aliases: &[],
        file_stem: "Content:Spelling & Grammar Errors Summary",
        header: &["Error", "Type", "URLs", "Instances"],
        requires: &[],
        owner: "M2-D02",
        run: stub_run,
    },
    TableEntry {
        name: "Content:Soft 404 Inlinks",
        aliases: &[],
        file_stem: "Content:Soft 404 Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D02",
        run: stub_run,
    },
];
