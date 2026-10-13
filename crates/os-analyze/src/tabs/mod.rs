//! Die Tabs von M1 in fester UI-Reihenfolge. P00b legt alle Dateien als Platzhalter an; die Pakete P20–P25
//! ersetzen je ihre Dateien, ohne Kennungen zu ändern (`tests/filters_registry.rs`).

pub mod canonicals;
pub mod content;
pub mod directives;
pub mod duplicates;
pub mod external;
pub mod h1;
pub mod h2;
pub mod hreflang;
pub mod images;
pub mod internal;
pub mod links;
pub mod meta_description;
pub mod page_titles;
pub mod response_codes;
pub mod security;
pub mod sitemaps;
pub mod url;

use crate::defs::{SortKey, TabDef};

/// Alle Tabs in UI-Reihenfolge: internal, external, security, response_codes, url, page_titles,
/// meta_description, h1, h2, content, images, canonicals, directives, hreflang, links, sitemaps.
pub fn all() -> Vec<TabDef> {
    vec![
        internal::tab(),
        external::tab(),
        security::tab(),
        response_codes::tab(),
        url::tab(),
        page_titles::tab(),
        meta_description::tab(),
        h1::tab(),
        h2::tab(),
        content::tab(),
        images::tab(),
        canonicals::tab(),
        directives::tab(),
        hreflang::tab(),
        links::tab(),
        sitemaps::tab(),
    ]
}

/// Spalte, nach der der Filter `Duplicate` der Element-Tabs gruppiert (ONPAGE-092, -102, -122, -132).
const DUPLICATE_GROUP_COLUMN: [(&str, &str); 4] = [
    ("Page Titles", "Title 1"),
    ("Meta Description", "Meta Description 1"),
    ("H1", "H1-1"),
    ("H2", "H2-1"),
];

/// Standardsortierung eines einzelnen Filters (ONPAGE-001). Die Ansicht fragt sie ab, wenn die Abfrage keine
/// Sortierung angibt, und zwar **vor** `TabDef.default_sort`; `None` heißt: der Filter hat keine eigene
/// Standardsortierung, es gilt `TabDef.default_sort` und danach die Entdeckungsreihenfolge (`UrlId`, K-17).
///
/// Page Titles, Meta Description, H1 und H2 haben kein `default_sort`; nur ihr Filter `Duplicate` gruppiert
/// gleiche Werte direkt untereinander (Sortierung nach dem ersten Wert, ONPAGE-092 und Gegenstücke), alle
/// anderen Filter erscheinen in Entdeckungsreihenfolge.
pub fn filter_default_sort(tab_id: &str, filter_id: &str) -> Option<Vec<SortKey>> {
    if filter_id != "Duplicate" {
        return None;
    }
    DUPLICATE_GROUP_COLUMN
        .iter()
        .find(|(tab, _)| *tab == tab_id)
        .map(|(_, column)| {
            vec![SortKey {
                column: (*column).to_string(),
                descending: false,
            }]
        })
}
