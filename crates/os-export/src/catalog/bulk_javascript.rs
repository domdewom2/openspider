//! JavaScript: Exporte je Filter und Konsolen-Log (OUT-058).
//!
//! STUB(M2-R02): Alle Einträge liefern bis zur Umsetzung nur die Kopfzeile (K-14, `stub_run`). Das Paket ersetzt `run`
//! je Eintrag durch die Auswertung und ergänzt oder schärft Kopfzeile, Voraussetzungen und Aliase; der Katalog in
//! `catalog/mod.rs` bleibt unverändert, weil er nur `entries()` aufruft.

use crate::catalog::reports::ReportRequirement;
use crate::catalog::{stub_run, TableEntry};

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

// STUB(M2-R02)
const ENTRIES: &[TableEntry] = &[
    TableEntry {
        name: "JavaScript:Blocked Resource Inlinks",
        aliases: &[],
        file_stem: "JavaScript:Blocked Resource Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[ReportRequirement::Rendering],
        owner: "M2-R02",
        run: stub_run,
    },
    TableEntry {
        name: "JavaScript:Contains JavaScript Links Inlinks",
        aliases: &[],
        file_stem: "JavaScript:Contains JavaScript Links Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[ReportRequirement::Rendering],
        owner: "M2-R02",
        run: stub_run,
    },
    TableEntry {
        name: "JavaScript:Contains JavaScript Content Inlinks",
        aliases: &[],
        file_stem: "JavaScript:Contains JavaScript Content Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[ReportRequirement::Rendering],
        owner: "M2-R02",
        run: stub_run,
    },
    TableEntry {
        name: "JavaScript:Noindex Only In Original HTML Inlinks",
        aliases: &[],
        file_stem: "JavaScript:Noindex Only In Original HTML Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[ReportRequirement::Rendering],
        owner: "M2-R02",
        run: stub_run,
    },
    TableEntry {
        name: "JavaScript:Nofollow Only In Original HTML Inlinks",
        aliases: &[],
        file_stem: "JavaScript:Nofollow Only In Original HTML Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[ReportRequirement::Rendering],
        owner: "M2-R02",
        run: stub_run,
    },
    TableEntry {
        name: "JavaScript:Canonical Only In Rendered HTML Inlinks",
        aliases: &[],
        file_stem: "JavaScript:Canonical Only In Rendered HTML Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[ReportRequirement::Rendering],
        owner: "M2-R02",
        run: stub_run,
    },
    TableEntry {
        name: "JavaScript:Canonical Mismatch Inlinks",
        aliases: &[],
        file_stem: "JavaScript:Canonical Mismatch Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[ReportRequirement::Rendering],
        owner: "M2-R02",
        run: stub_run,
    },
    TableEntry {
        name: "JavaScript:Page Title Only In Rendered HTML Inlinks",
        aliases: &[],
        file_stem: "JavaScript:Page Title Only In Rendered HTML Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[ReportRequirement::Rendering],
        owner: "M2-R02",
        run: stub_run,
    },
    TableEntry {
        name: "JavaScript:Pages With JavaScript Errors Inlinks",
        aliases: &[],
        file_stem: "JavaScript:Pages With JavaScript Errors Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[ReportRequirement::Rendering],
        owner: "M2-R02",
        run: stub_run,
    },
    TableEntry {
        name: "JavaScript:All Console Messages",
        aliases: &[],
        file_stem: "JavaScript:All Console Messages",
        header: &[
            "Address",
            "Level",
            "Message",
            "Source URL",
            "Line",
            "Column",
        ],
        requires: &[ReportRequirement::Rendering],
        owner: "M2-R02",
        run: stub_run,
    },
];
