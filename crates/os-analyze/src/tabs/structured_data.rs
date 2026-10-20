//! Structured Data population, extraction filters and type counts (TECH-183–186); validation filters, finding counts,
//! Rich Result features and finding texts (TECH-187–189, M2-D20).
use crate::defs::{ColumnDef, ColumnKind, ColumnSql, FilterDef, FilterKind, Requirement, TabDef};
use crate::issues::Lang;
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
/// Findings of one validation kind and severity on items of enabled formats (TECH-187/188); count ≥ 1 is the filter.
macro_rules! findings { ($validation:literal, $severity:literal) => { concat!("SELECT 1 FROM sd_issues i JOIN sd_items s ON s.id=i.item_id WHERE i.url_id=u.id AND i.validation='", $validation, "' AND i.severity='", $severity, "' AND ", active!()) }; }

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
    for (name, sql) in [
        (
            "Errors",
            concat!(
                "(SELECT COUNT(*) FROM (",
                findings!("schema_org", "error"),
                "))"
            ),
        ),
        (
            "Warnings",
            concat!(
                "(SELECT COUNT(*) FROM (",
                findings!("schema_org", "warning"),
                "))"
            ),
        ),
        (
            "Rich Result Errors",
            concat!(
                "(SELECT COUNT(*) FROM (",
                findings!("rich_results", "error"),
                "))"
            ),
        ),
        (
            "Rich Result Warnings",
            concat!(
                "(SELECT COUNT(*) FROM (",
                findings!("rich_results", "warning"),
                "))"
            ),
        ),
    ] {
        columns.push(ColumnDef::new(name, sql, ColumnKind::Int, true));
    }
    // Detected features are page facts (`sd.google.feature`, no item) in detection order.
    columns.push(ColumnDef::new("Rich Result Features", "(SELECT group_concat(feature, ', ') FROM (SELECT feature FROM sd_issues WHERE url_id=u.id AND validation='rich_results' AND code='sd.google.feature' ORDER BY id))", ColumnKind::Text, true));
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
            filter("Parse Errors", concat!("EXISTS(SELECT 1 FROM sd_issues i JOIN sd_items s ON s.id=i.item_id WHERE i.url_id=u.id AND i.validation='parse' AND ", active!(), ")"), true),
            filter("Validation Errors", concat!("EXISTS(", findings!("schema_org", "error"), ")"), true),
            filter("Validation Warnings", concat!("EXISTS(", findings!("schema_org", "warning"), ")"), true),
            filter("Rich Result Validation Errors", concat!("EXISTS(", findings!("rich_results", "error"), ")"), true),
            filter("Rich Result Validation Warnings", concat!("EXISTS(", findings!("rich_results", "warning"), ")"), true)],
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

// --- Finding texts (TECH-189; own wording) ----------------------------------------------------------------------

/// Validation kind in reports: `Schema.org` or `Google Rich Results` (OUT-100/101).
pub fn validation_label(validation: &str) -> &'static str {
    match validation {
        "schema_org" => "Schema.org",
        "rich_results" => "Google Rich Results",
        _ => "Parse",
    }
}

/// Validation kind in the detail view: `Schema.org` or `Google: <Feature>` (TECH-189).
pub fn detail_validation_label(validation: &str, feature: Option<&str>) -> String {
    match (validation, feature) {
        ("rich_results", Some(f)) => format!("Google: {f}"),
        _ => validation_label(validation).to_owned(),
    }
}

/// `Error` or `Warning`.
pub fn severity_label(severity: &str) -> &'static str {
    match severity {
        "error" => "Error",
        "warning" => "Warning",
        _ => "Info",
    }
}

fn short_name(raw: &str) -> &str {
    ["https://schema.org/", "http://schema.org/", "schema:"]
        .iter()
        .find_map(|p| raw.strip_prefix(p))
        .unwrap_or(raw)
}

/// Splits `offers[0].priceCurrency` into `(name, index)` segments.
fn segments(path: &str) -> Vec<(&str, Option<usize>)> {
    path.split('.')
        .map(|seg| match seg.split_once('[') {
            Some((name, rest)) => (name, rest.trim_end_matches(']').parse().ok()),
            None => (seg, None),
        })
        .collect()
}

/// Value at a finding path in the stored item (`sd_items.json`); `@type` segments yield the type string.
fn value_at<'v>(item: &'v serde_json::Value, path: &str) -> Option<&'v serde_json::Value> {
    let mut cur = item;
    for (name, index) in segments(path) {
        let obj = cur.as_object()?;
        let v = obj.get(name).or_else(|| {
            obj.iter()
                .find(|(k, _)| !k.starts_with('@') && short_name(k) == name)
                .map(|(_, v)| v)
        })?;
        cur = match (v, index) {
            (serde_json::Value::Array(a), Some(i)) => a.get(i)?,
            (v, Some(0) | None) => v,
            _ => return None,
        };
    }
    Some(cur)
}

fn first_type(node: &serde_json::Value) -> Option<String> {
    let t = node.get("@type")?;
    let t = t.as_str().or_else(|| t.as_array()?.first()?.as_str())?;
    Some(
        short_name(t)
            .rsplit(['/', '#'])
            .next()
            .unwrap_or(t)
            .to_owned(),
    )
}

/// Short type of the node that holds the finding (`offers[0].priceCurrency` → `Offer`); for type findings the node
/// itself.
pub fn finding_type(item: &serde_json::Value, path: &str) -> Option<String> {
    let parent = match path.rsplit_once('.') {
        Some((parent, _)) => value_at(item, parent)?,
        None => item,
    };
    first_type(parent)
}

/// Message of a finding with its placeholders replaced (`Property `priceCurrency` is missing (required for
/// Merchant listing).`). `item` is the stored item JSON; without it, type and value placeholders stay generic.
pub fn finding_text(
    code: &str,
    path: Option<&str>,
    feature: Option<&str>,
    item: Option<&serde_json::Value>,
    lang: Lang,
) -> String {
    let path = path.unwrap_or("");
    let last = path.rsplit('.').next().unwrap_or(path);
    let property = last.split('[').next().unwrap_or(last);
    let feature = feature.unwrap_or("");
    let ty = item
        .and_then(|i| finding_type(i, path))
        .unwrap_or_else(|| "?".into());
    let value = item
        .and_then(|i| value_at(i, path))
        .and_then(|v| v.as_str().map(str::to_owned).or_else(|| first_type(v)))
        .map(|v| short_name(&v).to_owned())
        .unwrap_or_else(|| "?".into());
    let group = property
        .split('|')
        .map(|p| format!("`{p}`"))
        .collect::<Vec<_>>()
        .join(", ");
    let de = lang == Lang::De;
    match code {
        "sd.schema.unknown_type" if de => format!("Typ `{value}` ist im schema.org-Vokabular nicht definiert."),
        "sd.schema.unknown_type" => format!("Type `{value}` is not defined in the schema.org vocabulary."),
        "sd.schema.unknown_property" if de => {
            format!("Eigenschaft `{property}` ist im schema.org-Vokabular nicht definiert.")
        }
        "sd.schema.unknown_property" => {
            format!("Property `{property}` is not defined in the schema.org vocabulary.")
        }
        "sd.schema.property_not_allowed" if de => {
            format!("Eigenschaft `{property}` ist für den Typ `{ty}` nicht vorgesehen.")
        }
        "sd.schema.property_not_allowed" => {
            format!("Property `{property}` is not expected on type `{ty}`.")
        }
        "sd.schema.value_type" if de => format!(
            "Der Wert von `{property}` hat den Typ `{value}`, den diese Eigenschaft nicht erwartet."
        ),
        "sd.schema.value_type" => {
            format!("The value of `{property}` has type `{value}`, which this property does not expect.")
        }
        "sd.schema.value_format" if de => {
            format!("Der Wert von `{property}` entspricht nicht dem erwarteten Datentyp.")
        }
        "sd.schema.value_format" => {
            format!("The value of `{property}` does not match the expected data type.")
        }
        "sd.schema.deprecated_vocabulary" if de => {
            "Das Element nutzt das abgekündigte Vokabular data-vocabulary.org; schema.org verwenden.".into()
        }
        "sd.schema.deprecated_vocabulary" => {
            "The item uses the retired data-vocabulary.org vocabulary; use schema.org instead.".into()
        }
        "sd.google.missing_required" if property.contains('|') && de => {
            format!("Mindestens eine der Eigenschaften {group} ist Pflicht für {feature}.")
        }
        "sd.google.missing_required" if property.contains('|') => {
            format!("At least one of {group} is required for {feature}.")
        }
        "sd.google.missing_required" if de => {
            format!("Eigenschaft `{property}` fehlt (Pflicht für {feature}).")
        }
        "sd.google.missing_required" => {
            format!("Property `{property}` is missing (required for {feature}).")
        }
        "sd.google.missing_recommended" if de => {
            format!("Eigenschaft `{property}` fehlt (empfohlen für {feature}).")
        }
        "sd.google.missing_recommended" => {
            format!("Property `{property}` is missing (recommended for {feature}).")
        }
        "sd.google.invalid_value" if de => {
            format!("Der Wert von `{property}` ist für {feature} ungültig.")
        }
        "sd.google.invalid_value" => format!("The value of `{property}` is invalid for {feature}."),
        other => other.to_owned(),
    }
}

/// schema.org reference page of the property a schema.org finding refers to (none for unknown names).
pub fn finding_link(code: &str, path: Option<&str>) -> Option<String> {
    if !matches!(
        code,
        "sd.schema.property_not_allowed" | "sd.schema.value_type" | "sd.schema.value_format"
    ) {
        return None;
    }
    let last = path?.rsplit('.').next()?.split('[').next()?;
    Some(format!("https://schema.org/{last}"))
}
