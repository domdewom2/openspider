//! Custom JavaScript: extraction columns and filters over persisted executions.
use crate::defs::{
    intern, ColumnDef, ColumnKind, ColumnSource, ColumnSql, FilterDef, FilterKind, Needs,
    Requirement, SortKey, TabDef,
};
use os_model::{CrawlConfig, JsSnippetKind};

const BASE: &[&str] = &["Address", "Content Type", "Status Code", "Status"];

pub fn tab() -> TabDef {
    TabDef {
        id: "Custom JavaScript",
        population: "u.is_internal = 1 AND u.in_tables = 1 AND u.status_code = 200 AND u.content_class <> 'PDF'",
        hidden_population: None,
        columns: vec![
            ColumnDef::new("Address", "u.address", ColumnKind::Url, true),
            ColumnDef::new("Content Type", "u.content_type", ColumnKind::Text, true),
            ColumnDef::new("Status Code", "u.status_code", ColumnKind::Int, true),
            ColumnDef::new("Status", "u.status", ColumnKind::Text, true),
        ],
        filters: vec![FilterDef {
            id: "All", predicate: "1", default_columns: &[], kind: FilterKind::Live,
            requires: vec![Requirement::Rendering], ignore_non_indexable: false,
        }],
        default_sort: vec![SortKey { column: "Address".into(), descending: false }],
        params: |_| Vec::new(),
    }
}
fn name(cfg: &CrawlConfig, i: usize) -> String {
    let s = &cfg.custom_js.snippets[i].name;
    if s.trim().is_empty() {
        format!("Snippet {}", i + 1)
    } else {
        s.trim().to_owned()
    }
}
struct JsColumns {
    internal: bool,
}
impl ColumnSource for JsColumns {
    fn columns(&self, cfg: &CrawlConfig) -> Vec<ColumnDef> {
        let mut columns = Vec::new();
        for (i, snippet) in cfg.custom_js.snippets.iter().take(100).enumerate() {
            if snippet.kind != JsSnippetKind::Extraction {
                continue;
            }
            let name = intern(&name(cfg, i));
            columns.push(ColumnDef {
                key: name,
                sql: ColumnSql::MultiCrawlWide { base: name, list_sql: intern(&format!(
                    "SELECT COALESCE(value, '') FROM custom_js_values WHERE url_id = ?1 AND def_index = {i} ORDER BY number"
                )) },
                kind: ColumnKind::Text, needs: Some(Needs::Rendering), ..ColumnDef::META
            });
            columns.push(ColumnDef::dynamic(&format!("{name} Error"), &format!(
                "(SELECT error FROM custom_js_values WHERE url_id = u.id AND def_index = {i} ORDER BY number LIMIT 1)"
            ), ColumnKind::Text, true).with_needs(Needs::Rendering));
        }
        columns
    }
    fn filters(&self, cfg: &CrawlConfig) -> Vec<FilterDef> {
        if self.internal {
            return Vec::new();
        }
        cfg.custom_js.snippets.iter().take(100).enumerate()
            .filter(|(_, s)| s.kind == JsSnippetKind::Extraction).map(|(i, _)| {
                let name = intern(&name(cfg, i));
                FilterDef {
                    id: name, predicate: intern(&format!(
                        "EXISTS (SELECT 1 FROM custom_js_values j WHERE j.url_id = u.id AND j.def_index = {i})"
                    )), default_columns: filter_columns(name), kind: FilterKind::Live,
                    requires: vec![Requirement::Rendering], ignore_non_indexable: false,
                }
            }).collect()
    }
    fn keeps_filter(&self, _: &CrawlConfig, id: &str) -> bool {
        self.internal || id != "<Snippetname> (je Extraktions-Snippet, dynamisch)"
    }
}
fn filter_columns(name: &'static str) -> &'static [&'static str] {
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
        let mut keys = BASE.to_vec();
        keys.extend([name, intern(&format!("{name} Error"))]);
        Box::leak(keys.into_boxed_slice())
    })
}
pub fn column_source() -> Option<Box<dyn ColumnSource>> {
    Some(Box::new(JsColumns { internal: false }))
}
pub fn internal_column_source() -> Option<Box<dyn ColumnSource>> {
    Some(Box::new(JsColumns { internal: true }))
}
