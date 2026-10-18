//! Custom Search: only searched documents, with live filters and result columns.

use os_model::{CrawlConfig, SearchFilter};

use crate::defs::{intern, ColumnDef, ColumnKind, ColumnSource, FilterDef, FilterKind, TabDef};

/// Effective name; empty names use their one-based configuration position.
pub fn filter_name(filter: &SearchFilter, index: usize) -> String {
    if filter.name.trim().is_empty() {
        format!("Filter {}", index + 1)
    } else {
        filter.name.trim().to_owned()
    }
}

/// Successful matches only: evaluation errors never satisfy a negative filter.
pub fn predicate(index: usize, contains: bool) -> String {
    format!("EXISTS (SELECT 1 FROM custom_search_hits h WHERE h.url_id=u.id AND h.def_index={index} AND h.error IS NULL AND h.count {} 0)", if contains { ">" } else { "=" })
}

/// Definition of the tab, before configuration-dependent columns are appended.
pub fn tab() -> TabDef {
    TabDef {
        id: "Custom Search",
        population:
            "u.is_internal=1 AND EXISTS (SELECT 1 FROM custom_search_hits h WHERE h.url_id=u.id)",
        hidden_population: None,
        columns: vec![
            ColumnDef::new("Address", "u.address", ColumnKind::Url, true),
            ColumnDef::new("Content", "u.content_type", ColumnKind::Text, true),
            ColumnDef::new("Status Code", "u.status_code", ColumnKind::Int, true),
            ColumnDef::new("Status", "u.status", ColumnKind::Text, true),
        ],
        filters: vec![FilterDef::placeholder("All", FilterKind::Live, &[])],
        default_sort: Vec::new(),
        params: |_| Vec::new(),
    }
}

/// Configured column/filter provider used by views, counts and exports.
pub fn column_source() -> Option<Box<dyn ColumnSource>> {
    Some(Box::new(SearchColumns))
}

struct SearchColumns;
impl ColumnSource for SearchColumns {
    fn columns(&self, cfg: &CrawlConfig) -> Vec<ColumnDef> {
        cfg.custom_search.filters.iter().enumerate().map(|(i, f)| {
            let value = if f.contains { "h.count" } else { "CASE WHEN h.count>0 THEN 'Contains' ELSE 'Does Not Contain' END" };
            ColumnDef::dynamic(&filter_name(f, i), &format!("(SELECT CASE WHEN h.error IS NOT NULL THEN h.error ELSE {value} END FROM custom_search_hits h WHERE h.url_id=u.id AND h.def_index={i})"), if f.contains { ColumnKind::Int } else { ColumnKind::Text }, true)
        }).collect()
    }

    fn filters(&self, cfg: &CrawlConfig) -> Vec<FilterDef> {
        cfg.custom_search
            .filters
            .iter()
            .enumerate()
            .map(|(i, f)| FilterDef {
                id: intern(&filter_name(f, i)),
                predicate: intern(&predicate(i, f.contains)),
                kind: FilterKind::Live,
                requires: Vec::new(),
                ignore_non_indexable: false,
                default_columns: &[],
            })
            .collect()
    }
}
