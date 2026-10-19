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
//! # `ignore_non_indexable` (CRAWL-089, K-56)
//!
//! „Pages With High Crawl Depth“ und „Pages Without Internal Outlinks“ schließen bei
//! `issues.ignore_non_indexable` (Standard an) nicht indexierbare Seiten aus; „All“ und die übrigen Filter nicht.
//!
//! # Präferenzen (TECH-144)
//!
//! Die Schwellen stammen aus `links.*` der Konfiguration, die in der Crawl-Datei liegt; eine Änderung wirkt
//! beim nächsten Zugriff ohne Recrawl.
//!
//! Link Score is published by the post-crawl step (TECH-145). JavaScript counters require rendering.

use os_model::{AnalysisStep, CrawlConfig, Value};

use crate::defs::{ColumnDef, ColumnKind, ColumnSql, FilterDef, FilterKind, SortKey, TabDef};

/// Anzahl der Seiten der Grundmenge (Nenner von `% of Total`).
macro_rules! total_pages {
    () => {
        "(SELECT COUNT(*) FROM urls t WHERE t.is_internal = 1 AND t.in_tables = 1 \
         AND t.content_class = 'HTML' AND t.status_code BETWEEN 200 AND 299)"
    };
}

/// `Unique Inlinks` ohne Selbstlink.
macro_rules! unique_inlinks {
    () => {
        "(c.unique_inlinks - CASE WHEN c.self_inlinks > 0 THEN 1 ELSE 0 END)"
    };
}

/// Grundmenge: interne HTML-Seiten mit Status 2xx (K-56).
const POPULATION: &str = "u.is_internal = 1 AND u.in_tables = 1 AND u.content_class = 'HTML' \
     AND u.status_code BETWEEN 200 AND 299";

/// Hohe Crawl-Tiefe (TECH-132); unerreichte Seiten (Tiefe leer) zählen nicht.
const HIGH_DEPTH: &str = "u.crawl_depth >= :high_crawl_depth";

/// Ohne interne Outlinks, Selbstlinks nicht gerechnet (TECH-133).
const NO_OUTLINKS: &str = "(c.outlinks - c.self_outlinks) = 0";

/// Interner Hyperlink mit `rel` nofollow, ugc oder sponsored (TECH-134); `links.rel` ist kleingeschrieben und
/// mit einem Leerzeichen zwischen den Token gespeichert. Seiten-nofollow (Meta, Header) zählt nicht.
const NOFOLLOW_OUTLINK: &str = "EXISTS (SELECT 1 FROM links l JOIN urls t ON t.id = l.target_id \
    WHERE l.source_id = u.id AND l.link_type = 'Hyperlink' AND l.crawlable = 1 AND t.is_internal = 1 AND l.rel IS NOT NULL \
    AND ((' ' || l.rel || ' ') LIKE '% nofollow %' OR (' ' || l.rel || ' ') LIKE '% ugc %' \
         OR (' ' || l.rel || ' ') LIKE '% sponsored %'))";

/// Interner Hyperlink, bei dem Ankertext und Alt-Text leer sind (TECH-135); `<area>` ohne `alt` zählt.
const NO_ANCHOR: &str = "EXISTS (SELECT 1 FROM links l JOIN urls t ON t.id = l.target_id \
    WHERE l.source_id = u.id AND l.link_type = 'Hyperlink' AND l.crawlable = 1 AND t.is_internal = 1 \
    AND COALESCE(l.anchor, '') COLLATE OS_ANCHOR = '' AND COALESCE(l.alt_text, '') COLLATE OS_ANCHOR = '')";

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

fn filter(id: &'static str, predicate: &'static str, ignore_non_indexable: bool) -> FilterDef {
    FilterDef {
        id,
        predicate,
        kind: FilterKind::Live,
        requires: Vec::new(),
        ignore_non_indexable,
        default_columns: &[],
    }
}

/// Schwellen aus den Link-Präferenzen.
fn params(cfg: &CrawlConfig) -> Vec<(&'static str, Value)> {
    vec![
        (
            "high_crawl_depth",
            Value::Int(i64::from(cfg.links.high_crawl_depth)),
        ),
        (
            "high_internal_outlinks",
            Value::Int(i64::from(cfg.links.high_internal_outlinks)),
        ),
        (
            "high_external_outlinks",
            Value::Int(i64::from(cfg.links.high_external_outlinks)),
        ),
        (
            "store_uncrawlable_links",
            Value::Bool(cfg.spider.store_uncrawlable_links),
        ),
        (
            "anchor_phrases",
            Value::Text(
                serde_json::to_string(&cfg.links.non_descriptive_anchors).expect("string list"),
            ),
        ),
    ]
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
            // Empty until the score step publishes its complete result.
            column("Link Score", SCORE_SQL, ColumnKind::Int)
                .with_needs(crate::defs::Needs::CrawlAnalysis(AnalysisStep::LinkScore)),
            column("Inlinks", "(c.inlinks - c.self_inlinks)", ColumnKind::Int),
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
                "(c.outlinks - c.self_outlinks)",
                ColumnKind::Int,
            ),
            column(
                "Unique Outlinks",
                "(c.unique_outlinks - CASE WHEN c.self_outlinks > 0 THEN 1 ELSE 0 END)",
                ColumnKind::Int,
            ),
            column("Unique JS Outlinks", "NULL", ColumnKind::Int),
            column("External Outlinks", "c.external_outlinks", ColumnKind::Int),
            column(
                "Unique External Outlinks",
                "c.unique_external_outlinks",
                ColumnKind::Int,
            ),
            column("Unique External JS Outlinks", "NULL", ColumnKind::Int),
        ],
        filters: vec![
            filter("All", "1", false),
            filter("Pages With High Crawl Depth", HIGH_DEPTH, true),
            filter("Pages Without Internal Outlinks", NO_OUTLINKS, true),
            filter("Internal Nofollow Outlinks", NOFOLLOW_OUTLINK, false),
            filter("Internal Outlinks With No Anchor Text", NO_ANCHOR, false),
            filter(
                "Non-Descriptive Anchor Text In Internal Outlinks",
                NON_DESCRIPTIVE_PAGE,
                false,
            ),
            filter("Pages With High External Outlinks", HIGH_EXTERNAL, false),
            filter("Pages With High Internal Outlinks", HIGH_INTERNAL, false),
            post_filter(
                "Follow & Nofollow Internal Inlinks To Page",
                "link.mixed_inlinks",
            ),
            post_filter("Internal Nofollow Inlinks Only", "link.nofollow_only"),
            filter("Outlinks To Localhost", LOCALHOST_PAGE, false),
            post_filter("Non-Indexable Page Inlinks Only", "link.non_indexable_only"),
            filter(
                "Pages With Uncrawlable Internal Outlinks",
                UNCRAWLABLE_PAGE,
                false,
            ),
        ],
        default_sort: vec![SortKey {
            column: "Address".to_string(),
            descending: false,
        }],
        params,
        hidden_population: None,
    }
}

/// Stored score is published atomically by the analysis step.
pub const SCORE_SQL: &str =
    "CASE WHEN NOT EXISTS (SELECT 1 FROM analysis_steps WHERE step = 'link_score' AND state <> 'done') THEN (SELECT v_int FROM url_ext WHERE url_id = u.id AND key = 'linkscore.score') END";
pub const NON_DESCRIPTIVE_LINK: &str = "l.link_type = 'Hyperlink' AND l.crawlable = 1 AND t.is_internal = 1 AND EXISTS (SELECT 1 FROM json_each(:anchor_phrases) p WHERE (CASE WHEN COALESCE(l.anchor, '') COLLATE OS_ANCHOR = '' THEN COALESCE(l.alt_text, '') ELSE l.anchor END) COLLATE OS_ANCHOR_PHRASE = p.value AND p.value COLLATE OS_ANCHOR_PHRASE <> '')";
const NON_DESCRIPTIVE_PAGE: &str = "EXISTS (SELECT 1 FROM links l JOIN urls t ON t.id = l.target_id, json_each(:anchor_phrases) p WHERE l.source_id = u.id AND l.link_type = 'Hyperlink' AND l.crawlable = 1 AND t.is_internal = 1 AND (CASE WHEN COALESCE(l.anchor, '') COLLATE OS_ANCHOR = '' THEN COALESCE(l.alt_text, '') ELSE l.anchor END) COLLATE OS_ANCHOR_PHRASE = p.value AND p.value COLLATE OS_ANCHOR_PHRASE <> '')";
const HIGH_EXTERNAL: &str = "u.id IN (SELECT l.source_id FROM links l JOIN urls t ON t.id = l.target_id WHERE l.link_type = 'Hyperlink' AND l.crawlable = 1 AND l.follow = 1 AND t.is_internal = 0 GROUP BY l.source_id HAVING COUNT(*) >= :high_external_outlinks)";
const HIGH_INTERNAL: &str = "u.id IN (SELECT l.source_id FROM links l JOIN urls t ON t.id = l.target_id WHERE l.link_type = 'Hyperlink' AND l.crawlable = 1 AND l.follow = 1 AND t.is_internal = 1 AND l.source_id <> l.target_id GROUP BY l.source_id HAVING COUNT(*) >= :high_internal_outlinks)";
pub const LOCALHOST_LINK: &str = "os_is_loopback(t.address) AND NOT (os_is_loopback((SELECT value FROM crawl_meta WHERE key = 'start_url')) AND os_authority(t.address) = os_authority((SELECT value FROM crawl_meta WHERE key = 'start_url')))";
const LOCALHOST_PAGE: &str = "EXISTS (SELECT 1 FROM links l JOIN urls t ON t.id = l.target_id WHERE l.source_id = u.id AND os_is_loopback(t.address) AND NOT (os_is_loopback((SELECT value FROM crawl_meta WHERE key = 'start_url')) AND os_authority(t.address) = os_authority((SELECT value FROM crawl_meta WHERE key = 'start_url'))))";
const UNCRAWLABLE_PAGE: &str = ":store_uncrawlable_links = 1 AND EXISTS (SELECT 1 FROM links l JOIN urls t ON t.id = l.target_id WHERE l.source_id = u.id AND l.link_type = 'Hyperlink' AND l.crawlable = 0 AND t.is_internal = 1)";

fn post_filter(id: &'static str, key: &str) -> FilterDef {
    FilterDef {
        kind: FilterKind::PostCrawl(AnalysisStep::Links),
        ..filter(
            id,
            crate::defs::intern(&format!(
                "u.id IN (SELECT url_id FROM url_ext WHERE key = '{key}' AND v_int = 1)"
            )),
            false,
        )
    }
}
struct ScoreSource;
impl crate::defs::ColumnSource for ScoreSource {
    fn columns(&self, _: &CrawlConfig) -> Vec<ColumnDef> {
        vec![column("Link Score", SCORE_SQL, ColumnKind::Int)
            .with_needs(crate::defs::Needs::CrawlAnalysis(AnalysisStep::LinkScore))]
    }
}
pub fn column_source() -> Option<Box<dyn crate::defs::ColumnSource>> {
    Some(Box::new(ScoreSource))
}

pub fn url_details(
    file: &os_store::CrawlFile,
    _: &CrawlConfig,
    id: os_model::UrlId,
) -> Result<Vec<(String, String)>, crate::ViewError> {
    use rusqlite::OptionalExtension;
    let score: Option<i64> = file
        .conn()
        .query_row(
            &format!("SELECT {SCORE_SQL} FROM urls u WHERE u.id = ?1"),
            [id.0],
            |r| r.get::<_, Option<i64>>(0),
        )
        .optional()?
        .flatten();
    Ok(vec![(
        "Link Score".into(),
        score.map(|s| s.to_string()).unwrap_or_default(),
    )])
}

/// Detail registry extension: keep anchor options and numeric kinds at the package boundary.
pub fn detail_source(
    kind: crate::details::DetailKind,
) -> Option<(&'static str, crate::details::DetailFn)> {
    (kind == crate::details::DetailKind::Anchors).then_some(("M2-D08", anchor_details))
}
fn anchor_details(
    file: &os_store::CrawlFile,
    _: &CrawlConfig,
    req: &crate::details::DetailReq<'_>,
) -> Result<crate::details::DetailRows, crate::ViewError> {
    let boolean = |camel: &str, snake: &str, default: bool| {
        req.params
            .get(camel)
            .or_else(|| req.params.get(snake))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(default)
    };
    let options = os_store::queries::anchors::AnchorOptions {
        outlinks: boolean("outlinks", "outlinks", false)
            || req
                .params
                .get("direction")
                .and_then(serde_json::Value::as_str)
                == Some("outlinks"),
        exclude_self: boolean("excludeSelf", "exclude_self", true),
        exclude_nofollow: boolean("excludeNofollow", "exclude_nofollow", false),
    };
    let data = os_store::queries::anchors::detail_rows_with(
        file,
        req.url_ids,
        req.offset,
        req.limit,
        options,
    )?;
    let columns = data
        .columns
        .into_iter()
        .enumerate()
        .map(|(i, name)| {
            (
                name,
                match i {
                    0 => ColumnKind::Text,
                    6 => ColumnKind::Float { decimals: 2 },
                    _ => ColumnKind::Int,
                },
            )
        })
        .collect();
    Ok(crate::details::DetailRows {
        columns,
        rows: data.rows,
        total: data.total,
        notes: Vec::new(),
    })
}
