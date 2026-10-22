//! Accessibility: one finding per violated rule, never per node (TECH-250–259).
use crate::defs::{intern, ColumnDef, ColumnKind, FilterDef, FilterKind, Requirement, TabDef};
use os_model::{CrawlConfig, Value};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Rule {
    pub id: String,
    pub issue: String,
    pub group: String,
    pub priority: String,
}
pub fn rules() -> &'static [Rule] {
    static RULES: std::sync::OnceLock<Vec<Rule>> = std::sync::OnceLock::new();
    RULES.get_or_init(|| {
        serde_json::from_str(include_str!("../../../os-render/resources/axe/rules.json"))
            .expect("bundled rule metadata")
    })
}
pub fn rule(id: &str) -> Option<&'static Rule> {
    rules().iter().find(|r| r.id == id)
}
pub const POPULATION:&str="u.is_internal=1 AND u.in_tables=1 AND u.content_class='HTML' AND u.status_code=200 AND EXISTS(SELECT 1 FROM a11y_rules a WHERE a.url_id=u.id)";
pub const GROUPS: [&str; 6] = [
    "Best Practice",
    "WCAG 2.0 A",
    "WCAG 2.0 AA",
    "WCAG 2.0 AAA",
    "WCAG 2.1 AA",
    "WCAG 2.2 AA",
];
fn filter(id: &'static str, predicate: &'static str) -> FilterDef {
    FilterDef {
        id,
        predicate,
        kind: FilterKind::Live,
        requires: vec![
            Requirement::Rendering,
            Requirement::Config("extraction.accessibility"),
        ],
        ignore_non_indexable: false,
        default_columns: &[],
    }
}
pub fn tab() -> TabDef {
    let mut columns = vec![];
    for (key, sql, kind) in [
        ("Address", "u.address", ColumnKind::Url),
        ("Content Type", "u.content_type", ColumnKind::Text),
        ("Status Code", "u.status_code", ColumnKind::Int),
        ("Status", "u.status", ColumnKind::Text),
        ("Indexability", "u.indexability", ColumnKind::Text),
        (
            "Indexability Status",
            "u.indexability_status",
            ColumnKind::Text,
        ),
    ] {
        columns.push(ColumnDef::new(key, sql, kind, true));
    }
    columns.push(ColumnDef::new(
        "All Violations",
        "(SELECT COUNT(*) FROM a11y_rules a WHERE a.url_id=u.id AND a.status='violation')",
        ColumnKind::Int,
        true,
    ));
    for group in GROUPS {
        let ids = rules()
            .iter()
            .filter(|r| r.group == group)
            .map(|r| format!("'{}'", r.id))
            .collect::<Vec<_>>()
            .join(",");
        columns.push(ColumnDef::dynamic(&format!("{group} Violations"),&format!("(SELECT COUNT(*) FROM a11y_rules a WHERE a.url_id=u.id AND a.status='violation' AND a.rule_id IN ({ids}))"),ColumnKind::Int,true));
    }
    columns.extend(super::pagespeed::accessibility_status_columns());
    columns.push(ColumnDef::new("Accessibility Score", "(SELECT ROUND(v_real) FROM api_values WHERE url_id=u.id AND source='psi' AND metric='Accessibility Score' AND level='url')", ColumnKind::Int, true));
    let mut filters = vec![filter("All", "1")];
    for r in rules() {
        filters.push(filter(intern(&r.id),intern(&format!("EXISTS(SELECT 1 FROM a11y_rules a WHERE a.url_id=u.id AND a.rule_id='{}' AND a.status='violation')",r.id))));
    }
    TabDef {
        id: "Accessibility",
        population: intern(&format!(":a11y_enabled=1 AND {POPULATION}")),
        hidden_population: None,
        columns,
        filters,
        default_sort: vec![],
        params: |cfg| {
            vec![(
                "a11y_enabled",
                Value::Int(i64::from(
                    cfg.extraction.accessibility
                        && cfg.rendering.mode == os_model::RenderingMode::JavaScript,
                )),
            )]
        },
    }
}

pub fn help_url(id: &str) -> String {
    format!("https://dequeuniversity.com/rules/axe/4.11/{id}")
}
pub fn impact(value: Option<&str>) -> String {
    match value {
        Some("critical") => "Critical",
        Some("serious") => "Serious",
        Some("moderate") => "Moderate",
        Some("minor") => "Minor",
        _ => "",
    }
    .into()
}

/// A parameterized SQL source shared by UI and streaming exports.
pub fn rule_details(
    file: &os_store::CrawlFile,
    _: &CrawlConfig,
    req: &crate::details::DetailReq<'_>,
) -> Result<crate::details::DetailRows, crate::ViewError> {
    details(file, req, false, false)
}
pub fn node_details(
    file: &os_store::CrawlFile,
    _: &CrawlConfig,
    req: &crate::details::DetailReq<'_>,
) -> Result<crate::details::DetailRows, crate::ViewError> {
    details(file, req, true, false)
}
fn details(
    file: &os_store::CrawlFile,
    req: &crate::details::DetailReq<'_>,
    nodes: bool,
    keys: bool,
) -> Result<crate::details::DetailRows, crate::ViewError> {
    let ids =
        serde_json::to_string(&req.url_ids.iter().map(|id| id.0).collect::<Vec<_>>()).expect("IDs");
    let result = req
        .params
        .get("result")
        .and_then(|v| v.as_str())
        .unwrap_or("All");
    let result = match result {
        "All" => "",
        "Violation" => "violation",
        "Incomplete" => "incomplete",
        "Inapplicable" => "inapplicable",
        "Passed" => "pass",
        other => other,
    };
    let selected = req
        .params
        .get("ruleId")
        .or_else(|| req.params.get("rule"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let selected = rules()
        .iter()
        .find(|r| r.id == selected || r.issue == selected)
        .map_or(selected, |r| r.id.as_str());
    let metadata = include_str!("../../../os-render/resources/axe/rules.json");
    let from = if nodes {
        "FROM a11y_nodes n JOIN a11y_rules a ON a.url_id=n.url_id AND a.rule_id=n.rule_id JOIN urls u ON u.id=a.url_id JOIN json_each(?4) m ON json_extract(m.value,'$.id')=a.rule_id"
    } else {
        "FROM a11y_rules a JOIN urls u ON u.id=a.url_id JOIN json_each(?4) m ON json_extract(m.value,'$.id')=a.rule_id"
    };
    let condition="WHERE a.url_id IN (SELECT value FROM json_each(?1)) AND (?2='' OR a.status=?2) AND (?3='' OR a.rule_id=?3)";
    let total = file.conn().query_row(
        &format!("SELECT COUNT(*) {from} {condition}"),
        rusqlite::params![ids, result, selected, metadata],
        |r| r.get(0),
    )?;
    let fields = if nodes {
        "u.address,json_extract(m.value,'$.issue'),n.target,substr(n.snippet,1,500)"
    } else {
        "u.address,json_extract(m.value,'$.issue'),CASE a.status WHEN 'violation' THEN 'Violation' WHEN 'incomplete' THEN 'Incomplete' WHEN 'pass' THEN 'Passed' ELSE 'Inapplicable' END,json_extract(m.value,'$.group'),CASE a.impact WHEN 'critical' THEN 'Critical' WHEN 'serious' THEN 'Serious' WHEN 'moderate' THEN 'Moderate' WHEN 'minor' THEN 'Minor' ELSE '' END,CASE WHEN a.status IN ('violation','incomplete') THEN (SELECT COUNT(*) FROM a11y_nodes n WHERE n.url_id=a.url_id AND n.rule_id=a.rule_id) END,'https://dequeuniversity.com/rules/axe/4.11/'||a.rule_id"
    };
    let fields = if keys {
        format!("{fields},n.id")
    } else {
        fields.to_string()
    };
    let mut names = if nodes {
        vec!["Address", "Issue", "Location on Page", "HTML"]
    } else {
        vec![
            "Address",
            "Issue",
            "Result",
            "Guideline",
            "Impact",
            "Nodes",
            "Help URL",
        ]
    };
    if keys {
        names.push("Node ID");
    }
    let default = if nodes {
        "u.address,a.rule_id,n.id"
    } else {
        "CASE a.impact WHEN 'critical' THEN 0 WHEN 'serious' THEN 1 WHEN 'moderate' THEN 2 WHEN 'minor' THEN 3 ELSE 4 END,6 DESC,2 COLLATE OS_TEXT,u.id,a.rule_id"
    };
    let mut order = vec![];
    for sort in req.sort {
        if let Some(i) = names.iter().position(|n| *n == sort.column) {
            order.push(format!(
                "{} {}",
                if sort.column == "Impact" { "CASE a.impact WHEN 'critical' THEN 0 WHEN 'serious' THEN 1 WHEN 'moderate' THEN 2 WHEN 'minor' THEN 3 ELSE 4 END".to_string() } else { (i+1).to_string() },
                if sort.descending { "DESC" } else { "ASC" }
            ));
        }
    }
    order.push(default.into());
    let mut stmt = file.conn().prepare(&format!(
        "SELECT {fields} {from} {condition} ORDER BY {} LIMIT ?5 OFFSET ?6",
        order.join(",")
    ))?;
    let rows = stmt
        .query_map(
            rusqlite::params![
                ids,
                result,
                selected,
                metadata,
                req.limit.min(500),
                req.offset.min(i64::MAX as u64)
            ],
            |r| {
                (0..names.len())
                    .map(|i| {
                        Ok(match r.get_ref(i)? {
                            rusqlite::types::ValueRef::Integer(v) => Value::Int(v),
                            rusqlite::types::ValueRef::Text(v) => {
                                Value::Text(String::from_utf8_lossy(v).into_owned())
                            }
                            _ => Value::Null,
                        })
                    })
                    .collect::<Result<Vec<_>, rusqlite::Error>>()
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(crate::details::DetailRows {
        columns: names
            .into_iter()
            .map(|s| {
                (
                    s.into(),
                    if s == "Nodes" {
                        ColumnKind::Int
                    } else {
                        ColumnKind::Text
                    },
                )
            })
            .collect(),
        rows,
        total,
        notes: vec![],
    })
}

pub struct Node {
    pub rule_id: String,
    pub impact: Option<String>,
    pub target: String,
    pub snippet: String,
}
pub fn node(
    file: &os_store::CrawlFile,
    url_id: i64,
    node_id: i64,
) -> Result<Option<Node>, os_store::StoreError> {
    use rusqlite::OptionalExtension;
    Ok(file.conn().query_row("SELECT n.rule_id,a.impact,n.target,n.snippet FROM a11y_nodes n JOIN a11y_rules a ON a.url_id=n.url_id AND a.rule_id=n.rule_id WHERE n.url_id=?1 AND n.id=?2",rusqlite::params![url_id,node_id],|r|{let snippet:String=r.get(3)?;Ok(Node{rule_id:r.get(0)?,impact:r.get(1)?,target:r.get(2)?,snippet:snippet.chars().take(500).collect()})}).optional()?)
}

/// Stable node IDs for the app's existing row-key extension, independent of sort/offset.
pub fn node_keys(
    file: &os_store::CrawlFile,
    req: &crate::details::DetailReq<'_>,
) -> Result<Vec<i64>, crate::ViewError> {
    Ok(details(file, req, true, true)?
        .rows
        .into_iter()
        .filter_map(|row| match row.last() {
            Some(Value::Int(id)) => Some(*id),
            _ => None,
        })
        .collect())
}
