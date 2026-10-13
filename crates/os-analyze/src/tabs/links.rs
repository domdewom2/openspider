//! Tab „Links“ (TECH-130, TECH-132, TECH-133, TECH-134, TECH-135, TECH-144).
//!
//! # Zählweise
//!
//! Die Zähler in `urls` zählen Hyperlink-Instanzen (`<a href>`, `<area href>`) mit interner Quelle und
//! enthalten Selbstlinks (ONPAGE-015). Der Tab zieht Selbstlinks ab (K-07, TECH-130): `Inlinks` =
//! `inlinks − self_inlinks`, `Unique Inlinks` ohne die Seite selbst, `Outlinks` und `Unique Outlinks`
//! ebenso. Eine Seite, die nur auf sich selbst verlinkt, hat damit 0 Outlinks (TECH-133).
//! Nofollow-Links zählen als Outlinks (TECH-133).
//!
//! # Präferenzen (TECH-144)
//!
//! Die Schwellen stammen aus `links.*` der Konfiguration, die in der Crawl-Datei liegt; eine Änderung wirkt
//! beim nächsten Zugriff ohne Recrawl.
//!
//! # Offen (P1)
//!
//! `Link Score` und die JavaScript-Spalten sind in M1 leer (TECH-145, TECH-103: kein Rendering).

use os_model::{CrawlConfig, Value};

use crate::defs::{ColumnDef, ColumnKind, ColumnSql, FilterDef, FilterKind, SortKey, TabDef};

/// Anzahl der Seiten der Grundmenge (Nenner von `% of Total`).
macro_rules! total_pages {
    () => {
        "(SELECT COUNT(*) FROM urls t WHERE t.is_internal = 1 AND t.in_tables = 1 \
         AND t.content_class = 'HTML' AND t.status_code = 200)"
    };
}

/// `Unique Inlinks` ohne Selbstlink.
macro_rules! unique_inlinks {
    () => {
        "(u.unique_inlinks - CASE WHEN u.self_inlinks > 0 THEN 1 ELSE 0 END)"
    };
}

/// Grundmenge: interne HTML-Seiten mit Status 200.
const POPULATION: &str =
    "u.is_internal = 1 AND u.in_tables = 1 AND u.content_class = 'HTML' AND u.status_code = 200";

/// Hohe Crawl-Tiefe (TECH-132); unerreichte Seiten (Tiefe leer) zählen nicht.
const HIGH_DEPTH: &str = "u.crawl_depth >= :high_crawl_depth";

/// Ohne interne Outlinks, Selbstlinks nicht gerechnet (TECH-133).
const NO_OUTLINKS: &str = "(u.outlinks - u.self_outlinks) = 0";

/// Interner Hyperlink mit `rel` nofollow, ugc oder sponsored (TECH-134); `links.rel` ist kleingeschrieben und
/// mit einem Leerzeichen zwischen den Token gespeichert. Seiten-nofollow (Meta, Header) zählt nicht.
const NOFOLLOW_OUTLINK: &str = "EXISTS (SELECT 1 FROM links l JOIN urls t ON t.id = l.target_id \
    WHERE l.source_id = u.id AND l.link_type = 'Hyperlink' AND t.is_internal = 1 AND l.rel IS NOT NULL \
    AND ((' ' || l.rel || ' ') LIKE '% nofollow %' OR (' ' || l.rel || ' ') LIKE '% ugc %' \
         OR (' ' || l.rel || ' ') LIKE '% sponsored %'))";

/// Interner Hyperlink, bei dem Ankertext und Alt-Text leer sind (TECH-135); `<area>` ohne `alt` zählt.
const NO_ANCHOR: &str = "EXISTS (SELECT 1 FROM links l JOIN urls t ON t.id = l.target_id \
    WHERE l.source_id = u.id AND l.link_type = 'Hyperlink' AND t.is_internal = 1 \
    AND TRIM(COALESCE(l.anchor, '')) = '' AND TRIM(COALESCE(l.alt_text, '')) = '')";

fn column(key: &'static str, sql: &'static str, kind: ColumnKind) -> ColumnDef {
    ColumnDef {
        key,
        sql: ColumnSql::Expr(sql),
        kind,
        default_visible: true,
        extension: false,
    }
}

fn filter(id: &'static str, predicate: &'static str, ignore_non_indexable: bool) -> FilterDef {
    FilterDef {
        id,
        predicate,
        kind: FilterKind::Live,
        requires: Vec::new(),
        ignore_non_indexable,
    }
}

/// Schwellen aus den Link-Präferenzen.
fn params(cfg: &CrawlConfig) -> Vec<(&'static str, Value)> {
    vec![(
        "high_crawl_depth",
        Value::Int(i64::from(cfg.links.high_crawl_depth)),
    )]
}

/// Definition des Tabs „Links“.
pub fn tab() -> TabDef {
    TabDef {
        id: "Links",
        population: POPULATION,
        columns: vec![
            column("Address", "u.address", ColumnKind::Url),
            column("Indexability", "u.indexability", ColumnKind::Text),
            column(
                "Indexability Status",
                "u.indexability_status",
                ColumnKind::Text,
            ),
            column("Crawl Depth", "u.crawl_depth", ColumnKind::Int),
            // P1 (TECH-145): leer, bis der Teilschritt „Link Score“ existiert.
            column("Link Score", "NULL", ColumnKind::Int),
            // [OS-Erweiterung]: steht in der Referenz im Internal-Tab (TECH-130, Zusatzspalte).
            ColumnDef {
                key: "Inlinks",
                sql: ColumnSql::Expr("(u.inlinks - u.self_inlinks)"),
                kind: ColumnKind::Int,
                default_visible: false,
                extension: true,
            },
            column("Unique Inlinks", unique_inlinks!(), ColumnKind::Int),
            column("Unique JS Inlinks", "NULL", ColumnKind::Int),
            column(
                "% of Total",
                concat!(
                    "CASE WHEN ",
                    total_pages!(),
                    " > 0 THEN 100.0 * ",
                    unique_inlinks!(),
                    " / ",
                    total_pages!(),
                    " END"
                ),
                ColumnKind::Float { decimals: 2 },
            ),
            column(
                "Outlinks",
                "(u.outlinks - u.self_outlinks)",
                ColumnKind::Int,
            ),
            column(
                "Unique Outlinks",
                "(u.unique_outlinks - CASE WHEN u.self_outlinks > 0 THEN 1 ELSE 0 END)",
                ColumnKind::Int,
            ),
            column("Unique JS Outlinks", "NULL", ColumnKind::Int),
            column("External Outlinks", "u.external_outlinks", ColumnKind::Int),
            column(
                "Unique External Outlinks",
                "u.unique_external_outlinks",
                ColumnKind::Int,
            ),
            column("Unique External JS Outlinks", "NULL", ColumnKind::Int),
        ],
        filters: vec![
            filter("All", "1", false),
            filter("Pages With High Crawl Depth", HIGH_DEPTH, true),
            filter("Pages Without Internal Outlinks", NO_OUTLINKS, false),
            filter("Internal Nofollow Outlinks", NOFOLLOW_OUTLINK, false),
            filter("Internal Outlinks With No Anchor Text", NO_ANCHOR, false),
        ],
        default_sort: vec![SortKey {
            column: "Address".to_string(),
            descending: false,
        }],
        params,
    }
}
