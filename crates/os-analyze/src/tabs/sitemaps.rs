//! Tab „Sitemaps“ (TECH-211, TECH-212, TECH-213, TECH-214, TECH-215).
//!
//! Die Filter „URLs In Sitemap“, „URLs Not In Sitemap“, „Orphan URLs“ und „Non-Indexable URLs In Sitemap“
//! sind Nachlauf-Filter des Teilschritts „Sitemaps“ (TECH-005): Sie lesen die Marken `in_sitemap` und
//! `orphan` aus `url_flags`, die `analysis::steps::sitemaps` schreibt. Bis zum Lauf des Teilschritts (und
//! bei abgewähltem Teilschritt) zeigen sie ihren Zustand statt Zahlen. „All“ ist live.
//!
//! # Grundmenge (TECH-211)
//!
//! Interne URLs der Tabellen. Ohne verarbeitete Sitemap (keine Datei der Art `urlset` in `sitemaps`) ist der
//! Tab leer – damit meldet „URLs Not In Sitemap“ dann nichts. Sitemap-Dateien selbst stehen mit
//! `in_tables = 0` in `urls` und gehören nicht zur Grundmenge (TECH-217/218 sind P1).

use crate::defs::{ColumnDef, ColumnKind, ColumnSql, FilterDef, FilterKind, SortKey, TabDef};
use os_model::{AnalysisStep, CrawlConfig, Value};

/// Grundmenge: interne URLs, sofern mindestens eine Sitemap verarbeitet wurde.
const POPULATION: &str = "u.is_internal = 1 AND u.in_tables = 1 \
    AND EXISTS (SELECT 1 FROM sitemaps s WHERE s.kind = 'urlset')";

/// Die URL steht in mindestens einer verarbeiteten Sitemap (Marke des Teilschritts).
const IN_SITEMAP: &str =
    "EXISTS (SELECT 1 FROM url_flags f WHERE f.url_id = u.id AND f.flag = 'in_sitemap')";

/// Indexierbare, gecrawlte HTML-Seite, die in keiner Sitemap steht (TECH-213).
const NOT_IN_SITEMAP: &str = "u.is_internal = 1 AND u.state = 'done' AND u.content_class = 'HTML' \
    AND u.indexability = 'Indexable' \
    AND NOT EXISTS (SELECT 1 FROM url_flags f WHERE f.url_id = u.id AND f.flag = 'in_sitemap')";

/// Orphan-Marke des Teilschritts (TECH-214).
const ORPHAN: &str =
    "EXISTS (SELECT 1 FROM url_flags f WHERE f.url_id = u.id AND f.flag = 'orphan')";

/// Abgerufene, nicht indexierbare URL in einer Sitemap (TECH-215); nicht abgerufene URLs zählen nicht.
const NON_INDEXABLE_IN_SITEMAP: &str = "u.state = 'done' AND u.indexability = 'Non-Indexable' \
    AND EXISTS (SELECT 1 FROM url_flags f WHERE f.url_id = u.id AND f.flag = 'in_sitemap')";

fn column(key: &'static str, sql: &'static str, kind: ColumnKind) -> ColumnDef {
    ColumnDef {
        key,
        sql: ColumnSql::Expr(sql),
        kind,
        default_visible: true,
        extension: false,
        ..ColumnDef::META
    }
}

fn extension(key: &'static str, sql: &'static str, kind: ColumnKind) -> ColumnDef {
    ColumnDef {
        key,
        sql: ColumnSql::Expr(sql),
        kind,
        default_visible: false,
        extension: true,
        ..ColumnDef::META
    }
}

fn post_crawl(id: &'static str, predicate: &'static str, ignore_non_indexable: bool) -> FilterDef {
    FilterDef {
        id,
        predicate,
        kind: FilterKind::PostCrawl(AnalysisStep::Sitemaps),
        requires: Vec::new(),
        ignore_non_indexable,
        default_columns: &[],
    }
}

fn no_params(_: &CrawlConfig) -> Vec<(&'static str, Value)> {
    Vec::new()
}

/// Definition des Tabs „Sitemaps“.
pub fn tab() -> TabDef {
    TabDef {
        hidden_population: None,
        id: "Sitemaps",
        population: POPULATION,
        columns: vec![
            column("Address", "u.address", ColumnKind::Url),
            column("Content Type", "u.content_type", ColumnKind::Text),
            column("Status Code", "u.status_code", ColumnKind::Int),
            column("Status", "u.status", ColumnKind::Text),
            column("Indexability", "u.indexability", ColumnKind::Text),
            column(
                "Indexability Status",
                "u.indexability_status",
                ColumnKind::Text,
            ),
            // [OS-Erweiterung] (TECH-211): Anzahl der Sitemap-Dateien, in denen die URL steht.
            extension(
                "Sitemaps",
                "(SELECT COUNT(*) FROM sitemap_entries e WHERE e.url_id = u.id)",
                ColumnKind::Int,
            ),
            // [OS-Erweiterung] (TECH-214): bei Folge-Orphans die Adresse der Wurzel-Orphan-URL.
            extension(
                "Found Via",
                "(SELECT t.address FROM url_flags f JOIN urls t ON t.id = CAST(f.value AS INTEGER) \
                  WHERE f.url_id = u.id AND f.flag = 'orphan')",
                ColumnKind::Url,
            ),
        ],
        filters: vec![
            FilterDef {
                id: "All",
                predicate: "1",
                kind: FilterKind::Live,
                requires: Vec::new(),
                ignore_non_indexable: false,
                default_columns: &[],
            },
            post_crawl("URLs In Sitemap", IN_SITEMAP, false),
            post_crawl("URLs Not In Sitemap", NOT_IN_SITEMAP, false),
            post_crawl("Orphan URLs", ORPHAN, true),
            post_crawl(
                "Non-Indexable URLs In Sitemap",
                NON_INDEXABLE_IN_SITEMAP,
                false,
            ),
        ],
        default_sort: vec![SortKey {
            column: "Address".to_string(),
            descending: false,
        }],
        params: no_params,
    }
}
