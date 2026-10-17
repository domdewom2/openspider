//! Accessibility: All Violations und je WCAG-Stufe (OUT-062).
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
        name: "Accessibility:All Violations",
        aliases: &[],
        file_stem: "Accessibility:All Violations",
        header: &[
            "Address",
            "Issue",
            "Rule ID",
            "Guideline",
            "Impact",
            "Priority",
            "Location on Page",
            "HTML",
            "Help URL",
        ],
        requires: &[
            ReportRequirement::Rendering,
            ReportRequirement::Config {
                key: "extraction.accessibility",
                what: "accessibility extraction",
            },
        ],
        owner: "M2-D21",
        run: stub_run,
        header_fn: None,
    },
    TableEntry {
        name: "Accessibility:WCAG 2.0 A",
        aliases: &[],
        file_stem: "Accessibility:WCAG 2.0 A",
        header: &[
            "Address",
            "Issue",
            "Rule ID",
            "Guideline",
            "Impact",
            "Priority",
            "Location on Page",
            "HTML",
            "Help URL",
        ],
        requires: &[
            ReportRequirement::Rendering,
            ReportRequirement::Config {
                key: "extraction.accessibility",
                what: "accessibility extraction",
            },
        ],
        owner: "M2-D21",
        run: stub_run,
        header_fn: None,
    },
    TableEntry {
        name: "Accessibility:WCAG 2.0 AA",
        aliases: &[],
        file_stem: "Accessibility:WCAG 2.0 AA",
        header: &[
            "Address",
            "Issue",
            "Rule ID",
            "Guideline",
            "Impact",
            "Priority",
            "Location on Page",
            "HTML",
            "Help URL",
        ],
        requires: &[
            ReportRequirement::Rendering,
            ReportRequirement::Config {
                key: "extraction.accessibility",
                what: "accessibility extraction",
            },
        ],
        owner: "M2-D21",
        run: stub_run,
        header_fn: None,
    },
    TableEntry {
        name: "Accessibility:WCAG 2.0 AAA",
        aliases: &[],
        file_stem: "Accessibility:WCAG 2.0 AAA",
        header: &[
            "Address",
            "Issue",
            "Rule ID",
            "Guideline",
            "Impact",
            "Priority",
            "Location on Page",
            "HTML",
            "Help URL",
        ],
        requires: &[
            ReportRequirement::Rendering,
            ReportRequirement::Config {
                key: "extraction.accessibility",
                what: "accessibility extraction",
            },
        ],
        owner: "M2-D21",
        run: stub_run,
        header_fn: None,
    },
    TableEntry {
        name: "Accessibility:WCAG 2.1 AA",
        aliases: &[],
        file_stem: "Accessibility:WCAG 2.1 AA",
        header: &[
            "Address",
            "Issue",
            "Rule ID",
            "Guideline",
            "Impact",
            "Priority",
            "Location on Page",
            "HTML",
            "Help URL",
        ],
        requires: &[
            ReportRequirement::Rendering,
            ReportRequirement::Config {
                key: "extraction.accessibility",
                what: "accessibility extraction",
            },
        ],
        owner: "M2-D21",
        run: stub_run,
        header_fn: None,
    },
    TableEntry {
        name: "Accessibility:WCAG 2.2 AA",
        aliases: &[],
        file_stem: "Accessibility:WCAG 2.2 AA",
        header: &[
            "Address",
            "Issue",
            "Rule ID",
            "Guideline",
            "Impact",
            "Priority",
            "Location on Page",
            "HTML",
            "Help URL",
        ],
        requires: &[
            ReportRequirement::Rendering,
            ReportRequirement::Config {
                key: "extraction.accessibility",
                what: "accessibility extraction",
            },
        ],
        owner: "M2-D21",
        run: stub_run,
        header_fn: None,
    },
    TableEntry {
        name: "Accessibility:Best Practice",
        aliases: &[],
        file_stem: "Accessibility:Best Practice",
        header: &[
            "Address",
            "Issue",
            "Rule ID",
            "Guideline",
            "Impact",
            "Priority",
            "Location on Page",
            "HTML",
            "Help URL",
        ],
        requires: &[
            ReportRequirement::Rendering,
            ReportRequirement::Config {
                key: "extraction.accessibility",
                what: "accessibility extraction",
            },
        ],
        owner: "M2-D21",
        run: stub_run,
        header_fn: None,
    },
    TableEntry {
        name: "Accessibility:Incomplete",
        aliases: &[],
        file_stem: "Accessibility:Incomplete",
        header: &[
            "Address",
            "Issue",
            "Rule ID",
            "Guideline",
            "Impact",
            "Priority",
            "Location on Page",
            "HTML",
            "Help URL",
        ],
        requires: &[
            ReportRequirement::Rendering,
            ReportRequirement::Config {
                key: "extraction.accessibility",
                what: "accessibility extraction",
            },
        ],
        owner: "M2-D21",
        run: stub_run,
        header_fn: None,
    },
];
