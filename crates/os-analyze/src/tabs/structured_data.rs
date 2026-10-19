//! Structured Data population, extraction filters and type counts (TECH-183–186).
use crate::defs::{ColumnDef, ColumnKind, ColumnSql, FilterDef, FilterKind, Requirement, TabDef};
use os_model::{CrawlConfig, Value};

const CONFIG: &[&str] = &[
    "extraction.structured_data.json_ld",
    "extraction.structured_data.microdata",
    "extraction.structured_data.rdfa",
];
macro_rules! active { () => { "((s.format='json-ld' AND :sd_json=1) OR (s.format='microdata' AND :sd_micro=1) OR (s.format='rdfa' AND :sd_rdfa=1))" }; }
macro_rules! types { () => { concat!("SELECT s.id, t.key, CASE WHEN t.value LIKE 'https://schema.org/%' THEN substr(t.value,20) WHEN t.value LIKE 'http://schema.org/%' THEN substr(t.value,19) ELSE t.value END AS name FROM sd_items s, json_each(s.json, CASE WHEN json_type(s.json, '$._os_types')='array' THEN '$._os_types' ELSE '$.\"@type\"' END) t WHERE s.url_id=u.id AND ", active!(), " AND json_extract(s.json,'$._os_parse_error') IS NULL") }; }
// Multi-column queries bind only ?1; the column source inserts literal format flags.
// The composite minimum retains the position of the first occurrence even across repeated types.
const TYPE_LIST: &str = "SELECT CASE WHEN t.value LIKE 'https://schema.org/%' THEN substr(t.value,20) WHEN t.value LIKE 'http://schema.org/%' THEN substr(t.value,19) ELSE t.value END AS name FROM sd_items s, json_each(s.json, CASE WHEN json_type(s.json, '$._os_types')='array' THEN '$._os_types' ELSE '$.\"@type\"' END) t WHERE s.url_id=?1 AND json_extract(s.json,'$._os_parse_error') IS NULL GROUP BY name ORDER BY MIN(printf('%020d:%020d',s.id,COALESCE(t.key,0)))";
fn filter(id: &'static str, predicate: &'static str, issue: bool) -> FilterDef {
    FilterDef {
        id,
        predicate,
        kind: FilterKind::Live,
        requires: vec![Requirement::AnyConfig(CONFIG)],
        ignore_non_indexable: issue,
        default_columns: &[],
    }
}
fn params(cfg: &CrawlConfig) -> Vec<(&'static str, Value)> {
    let c = &cfg.extraction.structured_data;
    vec![
        (
            "sd_enabled",
            Value::Int(i64::from(c.json_ld || c.microdata || c.rdfa)),
        ),
        ("sd_json", Value::Int(i64::from(c.json_ld))),
        ("sd_micro", Value::Int(i64::from(c.microdata))),
        ("sd_rdfa", Value::Int(i64::from(c.rdfa))),
    ]
}
pub fn tab() -> TabDef {
    let mut columns = vec![ColumnDef::new("Address", "u.url", ColumnKind::Url, true)];
    for (name, validation, severity) in [
        ("Errors", "schema_org", "error"),
        ("Warnings", "schema_org", "warning"),
        ("Rich Result Errors", "rich_results", "error"),
        ("Rich Result Warnings", "rich_results", "warning"),
    ] {
        columns.push(ColumnDef::dynamic(name, &format!("(SELECT COUNT(*) FROM sd_issues i WHERE i.url_id=u.id AND i.validation='{validation}' AND i.severity='{severity}')"), ColumnKind::Int, true));
    }
    columns.push(ColumnDef::new("Rich Result Features", "(SELECT group_concat(feature, ', ') FROM (SELECT DISTINCT feature FROM sd_issues WHERE url_id=u.id AND feature IS NOT NULL ORDER BY id))", ColumnKind::Text, true));
    columns.push(ColumnDef::new(
        "Total Types",
        concat!("(SELECT COUNT(*) FROM (", types!(), "))"),
        ColumnKind::Int,
        true,
    ));
    columns.push(ColumnDef::new(
        "Unique Types",
        concat!("(SELECT COUNT(DISTINCT name) FROM (", types!(), "))"),
        ColumnKind::Int,
        true,
    ));
    TabDef {
        id: "Structured Data", population: ":sd_enabled=1 AND u.is_internal=1 AND u.in_tables=1 AND u.content_class='HTML' AND u.status_code=200", columns,
        filters: vec![filter("All", "1", false),
            filter("Contains Structured Data", concat!("EXISTS(SELECT 1 FROM sd_items s WHERE s.url_id=u.id AND ", active!(), ")"), false),
            filter("Missing Structured Data", concat!("NOT EXISTS(SELECT 1 FROM sd_items s WHERE s.url_id=u.id AND ", active!(), ")"), true),
            filter("JSON-LD URLs", " :sd_json=1 AND EXISTS(SELECT 1 FROM sd_items s WHERE s.url_id=u.id AND s.format='json-ld' AND json_extract(s.json,'$._os_parse_error') IS NULL AND NOT EXISTS(SELECT 1 FROM sd_issues i WHERE i.url_id=s.url_id AND i.item_id=s.id AND i.validation='parse'))", false),
            filter("Microdata URLs", ":sd_micro=1 AND EXISTS(SELECT 1 FROM sd_items s WHERE s.url_id=u.id AND s.format='microdata' AND json_extract(s.json,'$._os_parse_error') IS NULL AND NOT EXISTS(SELECT 1 FROM sd_issues i WHERE i.url_id=s.url_id AND i.item_id=s.id AND i.validation='parse'))", false),
            filter("RDFa URLs", ":sd_rdfa=1 AND EXISTS(SELECT 1 FROM sd_items s WHERE s.url_id=u.id AND s.format='rdfa' AND json_extract(s.json,'$._os_parse_error') IS NULL AND NOT EXISTS(SELECT 1 FROM sd_issues i WHERE i.url_id=s.url_id AND i.item_id=s.id AND i.validation='parse'))", false),
            filter("Parse Errors", concat!("EXISTS(SELECT 1 FROM sd_issues i JOIN sd_items s ON s.id=i.item_id WHERE i.url_id=u.id AND i.validation='parse' AND ", active!(), ")"), true)],
        default_sort: Vec::new(), params, hidden_population: None,
    }
}

/// The type series depends on the enabled formats, so its SQL is configured once per config.
pub fn column_source() -> Option<Box<dyn crate::defs::ColumnSource>> {
    Some(Box::new(TypeColumns))
}
struct TypeColumns;
impl crate::defs::ColumnSource for TypeColumns {
    fn columns(&self, cfg: &CrawlConfig) -> Vec<ColumnDef> {
        let c = &cfg.extraction.structured_data;
        let active = format!(" AND ((s.format='json-ld' AND {}) OR (s.format='microdata' AND {}) OR (s.format='rdfa' AND {}))", i32::from(c.json_ld), i32::from(c.microdata), i32::from(c.rdfa));
        let sql = TYPE_LIST.replace(" GROUP BY", &format!("{active} GROUP BY"));
        vec![ColumnDef {
            key: "Type",
            sql: ColumnSql::Multi {
                base: "Type",
                list_sql: crate::defs::intern(&sql),
            },
            kind: ColumnKind::Text,
            ..ColumnDef::META
        }]
    }
}
