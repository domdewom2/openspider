//! Web-Daten: Quelltext, Text, Header und Cookies (OUT-045 bis OUT-049).
//!
//! STUB(M2-D18): Alle Einträge liefern bis zur Umsetzung nur die Kopfzeile (K-14, `stub_run`). Das Paket ersetzt `run`
//! je Eintrag durch die Auswertung und ergänzt oder schärft Kopfzeile, Voraussetzungen und Aliase; der Katalog in
//! `catalog/mod.rs` bleibt unverändert, weil er nur `entries()` aufruft.

use crate::catalog::reports::ReportRequirement;
use crate::catalog::{stub_run, TableEntry};

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

// STUB(M2-D18)
const ENTRIES: &[TableEntry] = &[
    TableEntry {
        name: "Web:All Page Source",
        aliases: &[],
        file_stem: "Web:All Page Source",
        header: &["Address", "File", "Source Type"],
        header_fn: None,
        requires: &[ReportRequirement::Config {
            key: "extraction.store_html",
            what: "HTML storage",
        }],
        owner: "M2-D18",
        run: stub_run,
    },
    TableEntry {
        name: "Web:All Page Text",
        aliases: &[],
        file_stem: "Web:All Page Text",
        header: &["Address", "File"],
        header_fn: None,
        requires: &[ReportRequirement::Config {
            key: "extraction.store_html",
            what: "HTML storage",
        }],
        owner: "M2-D18",
        run: stub_run,
    },
    TableEntry {
        name: "Web:All HTTP Request Headers",
        aliases: &[],
        file_stem: "Web:All HTTP Request Headers",
        header: &["Address", "Header Name", "Header Value"],
        header_fn: None,
        requires: &[ReportRequirement::Config {
            key: "extraction.http_headers",
            what: "HTTP header extraction",
        }],
        owner: "M2-D18",
        run: stub_run,
    },
    TableEntry {
        name: "Web:All HTTP Response Headers",
        aliases: &[],
        file_stem: "Web:All HTTP Response Headers",
        header: &["Address", "Header Name", "Header Value"],
        header_fn: None,
        requires: &[ReportRequirement::Config {
            key: "extraction.http_headers",
            what: "HTTP header extraction",
        }],
        owner: "M2-D18",
        run: stub_run,
    },
    TableEntry {
        name: "Web:All Cookies",
        aliases: &[],
        file_stem: "Web:All Cookies",
        header: &[
            "Address",
            "Cookie Name",
            "Cookie Value",
            "Domain",
            "Path",
            "Expires",
            "Max-Age",
            "Secure",
            "HttpOnly",
            "SameSite",
            "Type",
            "Session",
            "First-Party",
        ],
        requires: &[ReportRequirement::Config {
            key: "extraction.cookies",
            what: "cookie extraction",
        }],
        owner: "M2-D18",
        run: stub_run,
        header_fn: None,
    },
];
