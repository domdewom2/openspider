//! Pagination (TECH-031 … TECH-041): Live-Fakten und drei Nachlauf-Filter.

use super::canonicals::{
    canonical_link_element_column, column, http_canonical_column, meta_robots_column, multi_column,
    x_robots_tag_column,
};
use crate::defs::{ColumnKind, FilterDef, FilterKind, TabDef};
use os_model::{AnalysisStep, CrawlConfig, Value};

pub const FLAG_UNLINKED: &str = "pagination_unlinked";
pub const FLAG_LOOP: &str = "pagination_loop";
pub const FLAG_SEQUENCE: &str = "pagination_sequence";

fn filter(id: &'static str, predicate: &'static str, post: bool) -> FilterDef {
    FilterDef {
        id,
        predicate,
        kind: if post {
            FilterKind::PostCrawl(AnalysisStep::Pagination)
        } else {
            FilterKind::Live
        },
        requires: Vec::new(),
        ignore_non_indexable: false,
        default_columns: &[],
    }
}

fn params(cfg: &CrawlConfig) -> Vec<(&'static str, Value)> {
    vec![(
        "pagination_store",
        Value::Int(i64::from(cfg.spider.pagination.store)),
    )]
}

/// Der Pagination-Tab behält Folgeseiten unabhängig von `in_tables` (TECH-031).
pub fn tab() -> TabDef {
    TabDef {
        id: "Pagination",
        hidden_population: None,
        population: ":pagination_store = 1 AND u.is_internal = 1 AND \
            ((u.content_class = 'HTML' AND u.status_code = 200) OR \
             (u.state = 'done' AND EXISTS (SELECT 1 FROM pagination p WHERE p.target_id = u.id)))",
        columns: vec![
            column("Address", "u.address", ColumnKind::Url, true),
            column("Occurrences", "(SELECT COUNT(*) FROM pagination p WHERE p.url_id = u.id)", ColumnKind::Int, true),
            column("Indexability", "u.indexability", ColumnKind::Text, true),
            column("Indexability Status", "u.indexability_status", ColumnKind::Text, true),
            multi_column("rel=\"next\"", "SELECT COALESCE(p.url, p.raw) FROM pagination p WHERE p.url_id = ?1 AND p.rel = 'next' ORDER BY p.position, p.source", ColumnKind::Url),
            multi_column("rel=\"prev\"", "SELECT COALESCE(p.url, p.raw) FROM pagination p WHERE p.url_id = ?1 AND p.rel = 'prev' ORDER BY p.position, p.source", ColumnKind::Url),
            canonical_link_element_column(), http_canonical_column(), meta_robots_column(), x_robots_tag_column(),
        ],
        filters: vec![
            filter("All", "1", false),
            filter("Contains Pagination", "EXISTS (SELECT 1 FROM pagination p WHERE p.url_id = u.id)", false),
            filter("First Page", "EXISTS (SELECT 1 FROM pagination p WHERE p.url_id = u.id AND p.rel = 'next') AND NOT EXISTS (SELECT 1 FROM pagination p WHERE p.url_id = u.id AND p.rel = 'prev')", false),
            filter("Paginated 2+ Pages", "EXISTS (SELECT 1 FROM pagination p WHERE p.url_id = u.id AND p.rel = 'prev')", false),
            filter("Pagination URL Not In Anchor Tag", "EXISTS (SELECT 1 FROM pagination p WHERE p.url_id = u.id) AND EXISTS (SELECT 1 FROM url_ext e WHERE e.url_id = u.id AND e.key = 'pagination.not_in_anchor' AND e.v_int = 1)", false),
            filter("Non-200 Pagination URLs", "EXISTS (SELECT 1 FROM pagination p JOIN urls t ON t.id = p.target_id WHERE p.url_id = u.id AND t.state = 'done' AND COALESCE(t.status_code, 0) <> 200)", false),
            filter("Unlinked Pagination URLs", "EXISTS (SELECT 1 FROM url_flags f WHERE f.url_id = u.id AND f.flag = 'pagination_unlinked')", true),
            filter("Non-Indexable", "u.indexability = 'Non-Indexable' AND EXISTS (SELECT 1 FROM pagination p WHERE p.url_id = u.id)", false),
            filter("Multiple Pagination URLs", "EXISTS (SELECT 1 FROM pagination p WHERE p.url_id = u.id GROUP BY p.rel HAVING COUNT(*) >= 2)", false),
            filter("Pagination Loop", "EXISTS (SELECT 1 FROM url_flags f WHERE f.url_id = u.id AND f.flag = 'pagination_loop')", true),
            filter("Sequence Error", "EXISTS (SELECT 1 FROM url_flags f WHERE f.url_id = u.id AND f.flag = 'pagination_sequence')", true),
        ],
        default_sort: Vec::new(), params,
    }
}
