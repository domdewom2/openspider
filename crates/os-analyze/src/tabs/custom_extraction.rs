//! Custom Extraction columns, crawl-wide multiplicity and per-extractor filters.
use crate::defs::{
    intern, ColumnDef, ColumnKind, ColumnSource, ColumnSql, FilterDef, FilterKind, SortKey, TabDef,
};
use os_model::CrawlConfig;

const POPULATION: &str = "u.is_internal = 1 AND u.in_tables = 1 AND u.content_class = 'HTML' AND u.status_code BETWEEN 200 AND 299";

/// Definition of the Custom Extraction tab (INTEG-053).
pub fn tab() -> TabDef {
    TabDef {
        id: "Custom Extraction",
        population: POPULATION,
        hidden_population: None,
        columns: vec![
            ColumnDef::new("Address", "u.address", ColumnKind::Url, true),
            ColumnDef::new("Content", "u.content_type", ColumnKind::Text, true),
            ColumnDef::new("Status Code", "u.status_code", ColumnKind::Int, true),
            ColumnDef::new("Status", "u.status", ColumnKind::Text, true),
        ],
        filters: vec![FilterDef::placeholder("All", FilterKind::Live, &[])],
        default_sort: vec![SortKey {
            column: "Address".into(),
            descending: false,
        }],
        params: |_| Vec::new(),
    }
}

/// Default extractor names are stable across views and exports.
pub fn extractor_name(cfg: &CrawlConfig, index: usize) -> String {
    let name = &cfg.custom_extraction.extractors[index].name;
    if name.trim().is_empty() {
        format!("Extractor {}", index + 1)
    } else {
        name.trim().to_owned()
    }
}

struct ExtractionColumns {
    internal: bool,
}
impl ColumnSource for ExtractionColumns {
    fn columns(&self, cfg: &CrawlConfig) -> Vec<ColumnDef> {
        let mut columns: Vec<_> = cfg.custom_extraction.extractors.iter().take(100).enumerate().map(|(i, _)| {
            let name = intern(&extractor_name(cfg, i));
            ColumnDef {
                key: name,
                sql: ColumnSql::MultiCrawlWide { base: name, list_sql: intern(&format!("SELECT value FROM custom_extraction_values WHERE url_id = ?1 AND def_index = {i} ORDER BY number")) },
                kind: ColumnKind::Text, ..ColumnDef::META
            }
        }).collect();
        if !columns.is_empty() {
            columns.push(ColumnDef::new("Extraction Warnings", "(SELECT v_text FROM url_ext WHERE url_id = u.id AND key = 'custom_extract.warnings')", ColumnKind::Text, true));
        }
        columns
    }
    fn filters(&self, cfg: &CrawlConfig) -> Vec<FilterDef> {
        if self.internal {
            return Vec::new();
        }
        cfg.custom_extraction.extractors.iter().take(100).enumerate().map(|(i, _)| {
            let name = intern(&extractor_name(cfg, i));
            FilterDef {
                id: name,
                predicate: intern(&format!("EXISTS (SELECT 1 FROM custom_extraction_values e WHERE e.url_id = u.id AND e.def_index = {i} AND e.value <> '')")),
                default_columns: intern_columns(name),
                kind: FilterKind::Live, requires: Vec::new(), ignore_non_indexable: false,
            }
        }).collect()
    }
}

// Filter column lists are bounded by configured names, not pages or cell values.
fn intern_columns(name: &'static str) -> &'static [&'static str] {
    use std::{
        collections::HashMap,
        sync::{Mutex, OnceLock},
    };
    static LISTS: OnceLock<Mutex<HashMap<&'static str, &'static [&'static str]>>> = OnceLock::new();
    let mut lists = LISTS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    lists.entry(name).or_insert_with(|| {
        Box::leak(
            vec![
                "Address",
                "Content",
                "Status Code",
                "Status",
                name,
                "Extraction Warnings",
            ]
            .into_boxed_slice(),
        )
    })
}

/// Dynamic columns and filters for the extraction tab.
pub fn column_source() -> Option<Box<dyn ColumnSource>> {
    Some(Box::new(ExtractionColumns { internal: false }))
}
/// The same columns in Internal, without adding extraction-specific tab filters.
pub fn internal_column_source() -> Option<Box<dyn ColumnSource>> {
    Some(Box::new(ExtractionColumns { internal: true }))
}
