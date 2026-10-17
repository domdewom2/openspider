//! Teilschritte der Crawl-Analyse. Alle übrigen Schritte sind `NotApplicable`; die Stubs von M2 (`near_duplicates`,
//! `link_score`, `pagination`, `hreflang`, `links`) sind angemeldet und gehören den Fachpaketen D02, D05, D07, D08.

pub mod canonicals;
pub mod hreflang;
pub mod link_score;
pub mod links;
pub mod near_duplicates;
pub mod pagination;
pub mod sitemaps;

pub mod images;
