//! Hreflang-Reports (OUT-090 bis OUT-096).
//!
//! STUB(M2-D07): Alle Einträge liefern bis zur Umsetzung nur die Kopfzeile (K-14, `stub_run`). Das Paket ersetzt `run`
//! je Eintrag durch die Auswertung und ergänzt oder schärft Kopfzeile, Voraussetzungen und Aliase; der Katalog in
//! `catalog/mod.rs` bleibt unverändert, weil er nur `entries()` aufruft.

use crate::catalog::reports::ReportRequirement;
use crate::catalog::{stub_run, TableEntry};

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

// STUB(M2-D07)
const ENTRIES: &[TableEntry] = &[
    TableEntry {
        name: "Hreflang:All Hreflang URLs",
        aliases: &[],
        file_stem: "Hreflang:All Hreflang URLs",
        header: &[
            "Address",
            "Hreflang Language",
            "Hreflang URL",
            "Annotation Source",
            "Status Code",
            "Indexability",
            "Is Self Reference",
        ],
        requires: &[],
        owner: "M2-D07",
        run: stub_run,
    },
    TableEntry {
        name: "Hreflang:Non-200 Hreflang URLs",
        aliases: &[],
        file_stem: "Hreflang Non-200 Hreflang URLs Report",
        header: &[
            "Address",
            "Hreflang Language",
            "Hreflang URL",
            "Annotation Source",
            "Status Code",
            "Indexability",
            "Is Self Reference",
            "Status",
        ],
        requires: &[],
        owner: "M2-D07",
        run: stub_run,
    },
    TableEntry {
        name: "Hreflang:Unlinked Hreflang URLs",
        aliases: &[],
        file_stem: "Hreflang Unlinked Hreflang URLs Report",
        header: &[
            "Address",
            "Hreflang Language",
            "Hreflang URL",
            "Annotation Source",
            "Status Code",
            "Indexability",
            "Is Self Reference",
        ],
        requires: &[ReportRequirement::CrawlAnalysis],
        owner: "M2-D07",
        run: stub_run,
    },
    TableEntry {
        name: "Hreflang:Missing Return Links",
        aliases: &[],
        file_stem: "Hreflang Missing Return Links Report",
        header: &[
            "Address",
            "Hreflang Language",
            "Hreflang URL",
            "Hreflang URL Status Code",
            "Hreflang URL Indexability",
            "Missing Return Link To",
        ],
        requires: &[],
        owner: "M2-D07",
        run: stub_run,
    },
    TableEntry {
        name: "Hreflang:Inconsistent Language & Region Return Links",
        aliases: &[],
        file_stem: "Hreflang Inconsistent Language & Region Return Links Report",
        header: &[
            "Address",
            "Hreflang URL",
            "Expected Language",
            "Actual Language",
        ],
        requires: &[],
        owner: "M2-D07",
        run: stub_run,
    },
    TableEntry {
        name: "Hreflang:Non Canonical Return Links",
        aliases: &[],
        file_stem: "Hreflang:Non Canonical Return Links",
        header: &[
            "Address",
            "Hreflang Language",
            "Hreflang URL",
            "Canonical Of Hreflang URL",
        ],
        requires: &[],
        owner: "M2-D07",
        run: stub_run,
    },
    TableEntry {
        name: "Hreflang:Noindex Return Links",
        aliases: &[],
        file_stem: "Hreflang Noindex Return Links Report",
        header: &[
            "Address",
            "Hreflang Language",
            "Hreflang URL",
            "Indexability Status",
        ],
        requires: &[],
        owner: "M2-D07",
        run: stub_run,
    },
];
