//! Structured Data: Exporte als RDF-Tripel (OUT-056).
//!
//! STUB(M2-D09): Alle Einträge liefern bis zur Umsetzung nur die Kopfzeile (K-14, `stub_run`). Das Paket ersetzt `run`
//! je Eintrag durch die Auswertung und ergänzt oder schärft Kopfzeile, Voraussetzungen und Aliase; der Katalog in
//! `catalog/mod.rs` bleibt unverändert, weil er nur `entries()` aufruft.

use crate::catalog::{stub_run, TableEntry};

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

// STUB(M2-D09)
const ENTRIES: &[TableEntry] = &[
    TableEntry {
        name: "Structured Data:Contains Structured Data Triples",
        aliases: &[],
        file_stem: "Structured Data:Contains Structured Data Triples",
        header: &["Address", "Format", "Subject", "Predicate", "Object", "Validation Type", "Severity", "Issue"],
        requires: &[],
        owner: "M2-D09",
        run: stub_run,
    },
    TableEntry {
        name: "Structured Data:Validation Errors Triples",
        aliases: &[],
        file_stem: "Structured Data:Validation Errors Triples",
        header: &["Address", "Format", "Subject", "Predicate", "Object", "Validation Type", "Severity", "Issue"],
        requires: &[],
        owner: "M2-D09",
        run: stub_run,
    },
    TableEntry {
        name: "Structured Data:Validation Warnings Triples",
        aliases: &[],
        file_stem: "Structured Data:Validation Warnings Triples",
        header: &["Address", "Format", "Subject", "Predicate", "Object", "Validation Type", "Severity", "Issue"],
        requires: &[],
        owner: "M2-D09",
        run: stub_run,
    },
    TableEntry {
        name: "Structured Data:Parse Errors Triples",
        aliases: &[],
        file_stem: "Structured Data:Parse Errors Triples",
        header: &["Address", "Format", "Subject", "Predicate", "Object", "Validation Type", "Severity", "Issue"],
        requires: &[],
        owner: "M2-D09",
        run: stub_run,
    },
    TableEntry {
        name: "Structured Data:Microdata URLs Triples",
        aliases: &[],
        file_stem: "Structured Data:Microdata URLs Triples",
        header: &["Address", "Format", "Subject", "Predicate", "Object", "Validation Type", "Severity", "Issue"],
        requires: &[],
        owner: "M2-D09",
        run: stub_run,
    },
    TableEntry {
        name: "Structured Data:JSON-LD URLs Triples",
        aliases: &[],
        file_stem: "Structured Data:JSON-LD URLs Triples",
        header: &["Address", "Format", "Subject", "Predicate", "Object", "Validation Type", "Severity", "Issue"],
        requires: &[],
        owner: "M2-D09",
        run: stub_run,
    },
    TableEntry {
        name: "Structured Data:RDFa URLs Triples",
        aliases: &[],
        file_stem: "Structured Data:RDFa URLs Triples",
        header: &["Address", "Format", "Subject", "Predicate", "Object", "Validation Type", "Severity", "Issue"],
        requires: &[],
        owner: "M2-D09",
        run: stub_run,
    },
    TableEntry {
        name: "Structured Data:Rich Result Feature Detected Triples",
        aliases: &[],
        file_stem: "Structured Data:Rich Result Feature Detected Triples",
        header: &["Address", "Format", "Subject", "Predicate", "Object", "Validation Type", "Severity", "Issue"],
        requires: &[],
        owner: "M2-D09",
        run: stub_run,
    },
];
