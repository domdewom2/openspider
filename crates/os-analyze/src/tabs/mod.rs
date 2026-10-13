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

use crate::defs::TabDef;

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
