//! Stored SD items become a format/item/property/value tree (TECH-189).
use crate::dto::*;
use os_analyze::{
    issues::Lang,
    tabs::structured_data::{finding_link, finding_text},
};
use os_model::CrawlConfig;
use serde_json::Value;

fn node(next: &mut u32, format: &str, path: String, label: String) -> SdNode {
    *next += 1;
    SdNode {
        id: *next,
        format: format.into(),
        type_path: label,
        props: vec![],
        children: vec![],
        issues: vec![],
        path,
        source_line: None,
        source_col: None,
        property_order: vec![],
    }
}
fn build(
    v: &Value,
    format: &str,
    path: String,
    label: String,
    next: &mut u32,
    depth: usize,
) -> SdNode {
    let mut n = node(next, format, path.clone(), label);
    n.source_line = v
        .pointer("/_os_source/line")
        .and_then(Value::as_u64)
        .and_then(|n| n.try_into().ok());
    n.source_col = v
        .pointer("/_os_source/column")
        .and_then(Value::as_u64)
        .and_then(|n| n.try_into().ok());
    if depth > 64 {
        return n;
    }
    if let Some(obj) = v.as_object() {
        let mut order = v
            .get("_os_order")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for key in obj.keys().filter(|k| !k.starts_with("_os_")) {
            if !order.contains(key) {
                order.push(key.clone());
            }
        }
        n.property_order = order.clone();
        for key in order.iter().filter(|k| !k.starts_with("_os_")) {
            let Some(value) = obj.get(key) else { continue };
            let child_path = if path.is_empty() {
                key.clone()
            } else {
                format!("{path}.{key}")
            };
            n.children.push(build(
                value,
                format,
                child_path,
                key.clone(),
                next,
                depth + 1,
            ));
            if !value.is_object() && !value.is_array() {
                n.props.push(NameValue {
                    name: key.clone(),
                    value: Some(value.clone()),
                });
            }
        }
    } else if let Some(items) = v.as_array() {
        for (i, item) in items.iter().enumerate() {
            n.children.push(build(
                item,
                format,
                format!("{path}[{i}]"),
                format!("[{i}]"),
                next,
                depth + 1,
            ));
        }
    } else {
        n.props.push(NameValue {
            name: "value".into(),
            value: Some(v.clone()),
        });
    }
    n
}
fn attach(node: &mut SdNode, issue: SdIssueDto) {
    // Exactly one occurrence per finding. Missing-property findings stay on the closest existing ancestor.
    if let Some(child) = node.children.iter_mut().find(|child| {
        issue.path == child.path
            || issue.path.starts_with(&format!("{}.", child.path))
            || issue.path.starts_with(&format!("{}[", child.path))
    }) {
        attach(child, issue);
    } else {
        node.issues.push(issue);
    }
}
pub fn tree(
    items: Vec<os_store::queries::detail_ext::SdItem>,
    cfg: &CrawlConfig,
    lang: Lang,
) -> Result<SdTree, CmdError> {
    let sd = &cfg.extraction.structured_data;
    if !(sd.json_ld || sd.microdata || sd.rdfa) {
        return Err(super::unavailable("extraction.structured_data"));
    }
    let mut next = 0;
    let mut groups = vec![];
    for (format, enabled) in [
        ("json-ld", sd.json_ld),
        ("microdata", sd.microdata),
        ("rdfa", sd.rdfa),
    ] {
        if !enabled {
            continue;
        }
        let mut group = node(&mut next, format, String::new(), format.into());
        for item in items.iter().filter(|item| item.format == format) {
            let label = item.type_path.clone();
            let json = &item.json;
            let value: Value = serde_json::from_str(json).map_err(|e| {
                CmdError::new(ErrorCode::Store, "err-sd-json").with_detail(e.to_string())
            })?;
            let parse = value.get("_os_parse_error");
            let mut root = if parse.is_some() {
                node(&mut next, format, String::new(), "Parse Error".into())
            } else {
                build(&value, format, String::new(), label, &mut next, 0)
            };
            for issue in &item.issues {
                let os_store::queries::detail_ext::SdIssue {
                    validation,
                    feature,
                    severity,
                    property,
                    code,
                } = issue;
                let (validation, feature, severity, property, code) = (
                    validation.clone(),
                    feature.clone(),
                    severity.clone(),
                    property.clone(),
                    code.clone(),
                );
                let err = parse.filter(|_| validation == "parse");
                let message = err
                    .and_then(|p| p.get("message"))
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .unwrap_or_else(|| {
                        finding_text(
                            &code,
                            property.as_deref(),
                            feature.as_deref(),
                            Some(&value),
                            lang,
                        )
                    });
                let line = err
                    .and_then(|p| p.get("line"))
                    .and_then(Value::as_u64)
                    .and_then(|n| n.try_into().ok());
                let col = err
                    .and_then(|p| p.get("column"))
                    .and_then(Value::as_u64)
                    .and_then(|n| n.try_into().ok());
                let snippet = err
                    .and_then(|p| p.get("snippet"))
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                attach(
                    &mut root,
                    SdIssueDto {
                        severity,
                        validation: validation.clone(),
                        feature: feature.clone(),
                        property: property.clone(),
                        doc_url: if validation == "rich_results" {
                            os_parse::sd::validate::rich::rules()
                                .features
                                .iter()
                                .find(|f| Some(f.name.as_str()) == feature.as_deref())
                                .and_then(|f| f.doc.clone())
                        } else {
                            finding_link(&code, property.as_deref())
                        },
                        code: if err.is_some() {
                            "sd.parse.invalid_json".into()
                        } else {
                            code
                        },
                        message,
                        path: property.unwrap_or_default(),
                        line,
                        col,
                        snippet,
                    },
                );
            }
            if let Some(error) = parse {
                root.source_line = error
                    .get("line")
                    .and_then(Value::as_u64)
                    .and_then(|n| n.try_into().ok());
                root.source_col = error
                    .get("column")
                    .and_then(Value::as_u64)
                    .and_then(|n| n.try_into().ok());
            }
            group.children.push(root);
        }
        if !group.children.is_empty() {
            groups.push(group);
        }
    }
    Ok(SdTree { nodes: groups })
}
