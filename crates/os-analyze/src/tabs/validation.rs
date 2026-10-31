//! Live-Befunde aus dem Original-HTML (TECH-281 … TECH-290).

use crate::defs::{
    register_coverage_domain, ColumnDef, ColumnKind, FilterDef, FilterKind, Requirement, TabDef,
};
use os_model::Value;

const HTML: &str = "u.content_class = 'HTML' AND u.status_code = 200";

fn filter(id: &'static str, predicate: &'static str, structural: bool) -> FilterDef {
    FilterDef {
        id,
        predicate,
        kind: FilterKind::Live,
        requires: if structural {
            vec![Requirement::Coverage {
                config: "advanced.html_validation",
                marker: "validation.checked",
            }]
        } else {
            Vec::new()
        },
        ignore_non_indexable: false,
        default_columns: &[],
    }
}

macro_rules! fact {
    ($key:literal, $condition:literal) => {
        concat!(
            "u.content_class = 'HTML' AND u.status_code = 200 AND EXISTS ",
            "(SELECT 1 FROM url_ext e WHERE e.url_id = u.id AND e.key = 'validation.",
            $key,
            "' AND ",
            $condition,
            ")"
        )
    };
}

/// Definition des Validation-Tabs, Größen unabhängig vom Validierungsschalter.
pub fn tab() -> TabDef {
    register_coverage_domain("Validation", "validation.checked", HTML);
    TabDef {
        id: "Validation",
        population: "u.is_internal = 1 AND u.in_tables = 1",
        columns: vec![
            ColumnDef::new("Address", "u.address", ColumnKind::Url, true),
            ColumnDef::new("Content Type", "u.content_type", ColumnKind::Text, true),
            ColumnDef::new("Status Code", "u.status_code", ColumnKind::Int, true),
            ColumnDef::new("Status", "u.status", ColumnKind::Text, true),
            ColumnDef::new("Indexability", "u.indexability", ColumnKind::Text, true),
            ColumnDef::new("Indexability Status", "u.indexability_status", ColumnKind::Text, true),
        ],
        filters: vec![
            filter("All", "1", false),
            filter("Invalid HTML Elements In <head>", fact!("invalid_head_elements", "e.v_text <> ''"), true),
            filter("<head> Not First In <html> Element", fact!("head_not_first", "e.v_text <> ''"), true),
            filter("Missing <head> Tag", fact!("head_count", "e.v_int = 0"), true),
            filter("Multiple <head> Tags", fact!("head_count", "e.v_int >= 2"), true),
            filter("Missing <body> Tag", fact!("body_count", "e.v_int = 0"), true),
            filter("Multiple <body> Tags", fact!("body_count", "e.v_int >= 2"), true),
            filter("<body> Element Preceding <html>", fact!("body_before_html", "e.v_text <> ''"), true),
            filter("HTML Document Over {validation.html_doc_max_bytes}MB",
                "u.content_class = 'HTML' AND u.status_code = 200 AND EXISTS \
                 (SELECT 1 FROM url_ext e WHERE e.url_id = u.id AND \
                 ((e.key = 'validation.body_bytes' AND e.v_int > :validation_html_doc_max_bytes) OR \
                 (e.key = 'validation.truncated' AND e.v_int = 1)))", false),
            filter("Resource Over {validation.resource_max_bytes}MB",
                "u.content_class IN ('JavaScript', 'CSS') AND EXISTS \
                 (SELECT 1 FROM url_ext e WHERE e.url_id = u.id AND \
                 ((e.key = 'validation.body_bytes' AND e.v_int > :validation_resource_max_bytes) OR \
                 (e.key = 'validation.truncated' AND e.v_int = 1)))", false),
        ],
        default_sort: Vec::new(),
        params: |cfg| vec![
            ("validation_html_doc_max_bytes", Value::Int(cfg.validation.html_doc_max_bytes.min(i64::MAX as u64) as i64)),
            ("validation_resource_max_bytes", Value::Int(cfg.validation.resource_max_bytes.min(i64::MAX as u64) as i64)),
        ],
        hidden_population: None,
    }
}
