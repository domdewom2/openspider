//! Tab „External“ (ONPAGE-025, ONPAGE-026).
//!
//! Grundmenge: alle externen URLs, die in den Tabellen erscheinen (`in_tables = 1`, K-40): über Hyperlinks
//! oder Ressourcen-Referenzen interner Seiten gefunden, nur geprüft (Statuscode), nie weiter gecrawlt. Ist
//! „Externe Links crawlen“ aus, aber „Speichern“ an, stehen die URLs ohne Status in der Liste; die robots.txt
//! externer Hosts hat `in_tables = 0` und fehlt.
//!
//! Paritätsspalten in der Reihenfolge der Spec; `Response Time`, `Redirect URL` und `Redirect Type` sind
//! `[OS-Erweiterung]` (ausgeblendet, im Export „alle Spalten“ hinter den Referenzspalten).
//!
//! Klassenfilter: Parität (ONPAGE-023, A-49): `Flash` = Klasse `Plugins`; `Other` führt zusätzlich Medien,
//! Schriften und XML.

use crate::defs::{ColumnKind, FilterDef, FilterKind, TabDef};
use crate::tabs::internal::{class_filters, col, ext_col, unknown_filter, REDIRECT_TYPE};

/// Grundmenge der externen URLs.
const POPULATION: &str = "u.is_internal = 0 AND u.in_tables = 1";

/// `Status`; ist „Externe Links crawlen“ aus, stehen die URLs ohne Abruf in der Liste (A-11): Status
/// „Not Checked“.
const STATUS: &str = "COALESCE(u.status, CASE WHEN u.not_crawled_reason = 'external_not_checked' \
     THEN 'Not Checked' END)";

fn filter(id: &'static str, predicate: &'static str) -> FilterDef {
    FilterDef {
        id,
        predicate,
        kind: FilterKind::Live,
        requires: Vec::new(),
        ignore_non_indexable: false,
    }
}

/// Definition des Tabs „External“.
pub fn tab() -> TabDef {
    let columns = vec![
        col("Address", "u.address", ColumnKind::Url, true),
        col("Content", "u.content_type", ColumnKind::Text, true),
        col("Status Code", "u.status_code", ColumnKind::Int, true),
        col("Status", STATUS, ColumnKind::Text, true),
        col("Crawl Depth", "u.crawl_depth", ColumnKind::Int, true),
        // Link-Instanzen von internen Seiten: Hyperlinks und Ressourcen-Referenzen
        col("Inlinks", "u.inlinks_all", ColumnKind::Int, true),
        ext_col("Response Time", "u.response_time_ms", ColumnKind::Seconds3),
        ext_col("Redirect URL", "u.redirect_url", ColumnKind::Url),
        ext_col("Redirect Type", REDIRECT_TYPE, ColumnKind::Text),
    ];

    let mut filters = vec![filter("All", "1")];
    filters.extend(class_filters());
    filters.push(filter("Flash", "u.content_class = 'Plugins'"));
    filters.push(filter(
        "Other",
        "u.content_class IN ('Other', 'Media', 'Fonts', 'XML')",
    ));
    filters.push(unknown_filter());

    TabDef {
        id: "External",
        population: POPULATION,
        columns,
        filters,
        default_sort: Vec::new(),
        params: |_| Vec::new(),
    }
}
