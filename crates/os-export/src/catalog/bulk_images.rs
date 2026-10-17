//! Images: Exporte je Filter (OUT-053).
//!
//! STUB(M2-D03): Alle Einträge liefern bis zur Umsetzung nur die Kopfzeile (K-14, `stub_run`). Das Paket ersetzt `run`
//! je Eintrag durch die Auswertung und ergänzt oder schärft Kopfzeile, Voraussetzungen und Aliase; der Katalog in
//! `catalog/mod.rs` bleibt unverändert, weil er nur `entries()` aufruft.

use crate::catalog::reports::ReportRequirement;
use crate::catalog::{stub_run, TableEntry};

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

// STUB(M2-D03)
const ENTRIES: &[TableEntry] = &[
    TableEntry {
        name: "Images:All Image Inlinks",
        aliases: &[],
        file_stem: "Images:All Image Inlinks",
        header: crate::catalog::IMAGE_HEADER,
        requires: &[],
        owner: "M2-D03",
        run: stub_run,
    },
    TableEntry {
        name: "Images:Images Missing Alt Text Inlinks",
        aliases: &[],
        file_stem: "Images:Images Missing Alt Text Inlinks",
        header: crate::catalog::IMAGE_HEADER,
        requires: &[],
        owner: "M2-D03",
        run: stub_run,
    },
    TableEntry {
        name: "Images:Images Missing Alt Attribute Inlinks",
        aliases: &[],
        file_stem: "Images:Images Missing Alt Attribute Inlinks",
        header: crate::catalog::IMAGE_HEADER,
        requires: &[],
        owner: "M2-D03",
        run: stub_run,
    },
    TableEntry {
        name: "Images:Images With Alt Text Over X Characters Inlinks",
        aliases: &[],
        file_stem: "Images:Images With Alt Text Over X Characters Inlinks",
        header: crate::catalog::IMAGE_HEADER,
        requires: &[],
        owner: "M2-D03",
        run: stub_run,
    },
    TableEntry {
        name: "Images:Images Over X KB Inlinks",
        aliases: &[],
        file_stem: "Images:Images Over X KB Inlinks",
        header: crate::catalog::IMAGE_HEADER,
        requires: &[],
        owner: "M2-D03",
        run: stub_run,
    },
    TableEntry {
        name: "Images:Images Missing Size Attributes Inlinks",
        aliases: &[],
        file_stem: "Images:Images Missing Size Attributes Inlinks",
        header: crate::catalog::IMAGE_HEADER,
        requires: &[],
        owner: "M2-D03",
        run: stub_run,
    },
    TableEntry {
        name: "Images:Incorrectly Sized Images Inlinks",
        aliases: &[],
        file_stem: "Images:Incorrectly Sized Images Inlinks",
        header: crate::catalog::IMAGE_HEADER,
        requires: &[ReportRequirement::Rendering],
        owner: "M2-D03",
        run: stub_run,
    },
    TableEntry {
        name: "Images:Background Images Inlinks",
        aliases: &["Images:Background Images"],
        file_stem: "Images:Background Images Inlinks",
        header: crate::catalog::IMAGE_HEADER,
        requires: &[ReportRequirement::Rendering],
        owner: "M2-D03",
        run: stub_run,
    },
];
