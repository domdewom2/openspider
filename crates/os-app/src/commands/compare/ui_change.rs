//! TECH-301: bounded paired projections from the ordinary tab registry, with the same numeric deltas as exports.
use super::compare_error;
use crate::dto::{CmdError, ErrorCode, SortDir, SortKey};
use os_analyze::{
    compare::{Comparison, Side},
    views::{self, ColumnSelection, ViewQuery},
};
use os_model::{CancelToken, UrlId, Value};
use serde_json::{json, Value as Json};
use std::collections::HashMap;

const ELEMENTS: [(&str, &str); 13] = [
    ("indexability", "Indexability"),
    ("page_titles", "Title 1"),
    ("meta_description", "Meta Description 1"),
    ("h1", "H1-1"),
    ("word_count", "Word Count"),
    ("crawl_depth", "Crawl Depth"),
    ("inlinks", "Inlinks"),
    ("unique_inlinks", "Unique Inlinks"),
    ("internal_outlinks", "Outlinks"),
    ("unique_internal_outlinks", "Unique Outlinks"),
    ("external_outlinks", "External Outlinks"),
    ("unique_external_outlinks", "Unique External Outlinks"),
    ("structured_data_unique_types", "Unique Types"),
];
pub(super) fn filter_name(id: &str) -> &str {
    match id {
        "indexability" => "Indexability",
        "page_titles" => "Page Titles",
        "meta_description" => "Meta Description",
        "h1" => "H1",
        "word_count" => "Word Count",
        "crawl_depth" => "Crawl Depth",
        "inlinks" => "Inlinks",
        "unique_inlinks" => "Unique Inlinks",
        "internal_outlinks" => "Internal Outlinks",
        "unique_internal_outlinks" => "Unique Internal Outlinks",
        "external_outlinks" => "External Outlinks",
        "unique_external_outlinks" => "Unique External Outlinks",
        "structured_data_unique_types" => "Structured Data Unique Types",
        "content" => "Content",
        _ => id,
    }
}
type Facts = HashMap<u32, HashMap<String, Value>>;
fn numeric(id: &str) -> bool {
    !["indexability", "page_titles", "meta_description", "h1"].contains(&id)
}
fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Int(n) => Some(*n as f64),
        Value::Float(n) => Some(*n),
        _ => None,
    }
}
fn normalized(v: &Value) -> Value {
    match v {
        Value::Text(s) => Value::Text(s.split_whitespace().collect::<Vec<_>>().join(" ")),
        _ => v.clone(),
    }
}
fn batch(
    c: &Comparison,
    side: Side,
    ids: &[UrlId],
    tab: &str,
    columns: Vec<String>,
) -> Result<Facts, CmdError> {
    let s = c.side(side);
    let registry = os_analyze::tabs::configured_for_file(&s.config, &s.file);
    let mut q = ViewQuery::new(tab, "All");
    q.columns = ColumnSelection::Named(columns);
    q.limit = 500;
    let p = views::query_url_ids_in(&registry, &s.file, &s.config, &q, ids, true)
        .map_err(super::super::catalog::view_error)?;
    Ok(p.row_keys
        .into_iter()
        .zip(p.rows)
        .map(|(id, row)| (id.0 as u32, p.columns.iter().cloned().zip(row).collect()))
        .collect())
}
fn merge_types(
    c: &Comparison,
    side: Side,
    ids: &[UrlId],
    facts: &mut Facts,
) -> Result<(), CmdError> {
    for (id, row) in batch(
        c,
        side,
        ids,
        "Structured Data",
        vec!["Unique Types".into(), "Type".into()],
    )? {
        let mut names: Vec<_> = row
            .iter()
            .filter_map(|(k, v)| {
                if k.starts_with("Type ") {
                    if let Value::Text(s) = v {
                        Some(s.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
        names.sort();
        names.dedup();
        let out = facts.entry(id).or_default();
        out.extend(row);
        out.insert("Unique Type Names".into(), Value::Text(names.join(", ")));
    }
    Ok(())
}
#[derive(Clone, Copy)]
pub(super) struct PageRequest<'a> {
    pub filter: &'a str,
    pub change: Option<&'a str>,
    pub offset: u64,
    pub limit: u32,
    pub content_threshold_pct: u8,
    pub sort: Option<&'a SortKey>,
}
pub(super) fn page(
    c: &mut Comparison,
    elements: &[String],
    req: PageRequest<'_>,
    cancel: &CancelToken,
) -> Result<Json, CmdError> {
    project_page(c, elements, req, cancel, None)
}

// The first pass retains only sort values and IDs. Only the requested page is
// projected again, so sorting never buffers every row's selected element cells.
fn project_page(
    c: &mut Comparison,
    elements: &[String],
    req: PageRequest<'_>,
    cancel: &CancelToken,
    selected: Option<Vec<u32>>,
) -> Result<Json, CmdError> {
    let filter = elements
        .iter()
        .find(|id| filter_name(id) == req.filter)
        .map_or(req.filter, |id| id.as_str());
    let active: Vec<_> = ELEMENTS
        .iter()
        .filter(|(id, _)| elements.iter().any(|e| e == id))
        .collect();
    let content = elements.iter().any(|e| e == "content");
    if filter != "All"
        && !(content && filter == "content")
        && !active.iter().any(|(id, _)| *id == filter)
    {
        return Err(CmdError::invalid_input("err-compare-input", "filter"));
    }
    if req
        .change
        .is_some_and(|v| !["changed", "notChanged"].contains(&v))
    {
        return Err(CmdError::invalid_input("err-compare-input", "class"));
    }
    let mut columns = vec!["Address".to_string()];
    let mut projection = vec!["Address".to_string()];
    for (id, name) in &active {
        columns.extend([format!("{name} (Current)"), format!("{name} (Previous)")]);
        if *name != "Unique Types" {
            projection.push(name.to_string());
        }
        if numeric(id) {
            columns.extend([format!("{name} Change"), format!("{name} Change %")]);
        }
        let extra = match *id {
            "indexability" => Some("Indexability Status"),
            "structured_data_unique_types" => Some("Unique Type Names"),
            _ => None,
        };
        if let Some(n) = extra {
            columns.extend([format!("{n} (Current)"), format!("{n} (Previous)")]);
            if *id == "indexability" {
                projection.push(n.into());
            }
        }
    }
    if content {
        columns.push("Content Change %".into());
    }
    let cur_html = c
        .filter_set(Side::Current, "Internal", "HTML")
        .map_err(compare_error)?
        .and_then(|f| f.ids.clone())
        .unwrap_or_default();
    let prev_html = c
        .filter_set(Side::Previous, "Internal", "HTML")
        .map_err(compare_error)?
        .and_then(|f| f.ids.clone())
        .unwrap_or_default();
    let ids: Vec<_> = selected.unwrap_or_else(|| {
        c.pairing()
            .paired_current
            .iter()
            .filter(|id| {
                cur_html.contains(*id)
                    && c.pairing()
                        .previous_of(*id)
                        .is_some_and(|p| prev_html.contains(p))
            })
            .collect()
    });
    let sort_column = req
        .sort
        .map(|sort| {
            columns
                .iter()
                .position(|column| column == &sort.column)
                .ok_or_else(|| CmdError::invalid_input("err-compare-input", "sort"))
        })
        .transpose()?;
    let mut order = Vec::new();
    let mut cells = Vec::new();
    let mut keys = Vec::new();
    let mut partners = Vec::new();
    let mut total = 0u64;
    let mut changed_counts = std::collections::BTreeMap::<String, u64>::new();
    let mut changed_total = 0u64;
    let mut content_available = true;
    for chunk in ids.chunks(500) {
        cancel
            .checkpoint()
            .map_err(|_| CmdError::new(ErrorCode::Cancelled, "err-cancelled"))?;
        let curr: Vec<_> = chunk.iter().map(|id| UrlId(i64::from(*id))).collect();
        let prev: Vec<_> = chunk
            .iter()
            .map(|id| UrlId(i64::from(c.pairing().previous_of(*id).expect("paired"))))
            .collect();
        let mut cf = batch(c, Side::Current, &curr, "Internal", projection.clone())?;
        let mut pf = batch(c, Side::Previous, &prev, "Internal", projection.clone())?;
        if active
            .iter()
            .any(|(id, _)| *id == "structured_data_unique_types")
        {
            merge_types(c, Side::Current, &curr, &mut cf)?;
            merge_types(c, Side::Previous, &prev, &mut pf)?;
        }
        for id in chunk {
            let partner = c.pairing().previous_of(*id).expect("paired");
            let value = |facts: &Facts, id: u32, name: &str| {
                facts
                    .get(&id)
                    .and_then(|f| f.get(name))
                    .cloned()
                    .unwrap_or(Value::Null)
            };
            let mut row = vec![value(&cf, *id, "Address")];
            let mut changed = false;
            let mut any_changed = false;
            for (element, name) in &active {
                let cur = value(&cf, *id, name);
                let prev = value(&pf, partner, name);
                let extra = match *element {
                    "indexability" => Some("Indexability Status"),
                    "structured_data_unique_types" => Some("Unique Type Names"),
                    _ => None,
                };
                let different = normalized(&cur) != normalized(&prev)
                    || extra.is_some_and(|n| {
                        normalized(&value(&cf, *id, n)) != normalized(&value(&pf, partner, n))
                    });
                if different {
                    *changed_counts
                        .entry(filter_name(element).into())
                        .or_default() += 1;
                    any_changed = true;
                }
                if (filter == "All" || filter == *element) && different {
                    changed = true;
                }
                row.extend([cur.clone(), prev.clone()]);
                if numeric(element) {
                    let diff = number(&cur).zip(number(&prev)).map(|(x, y)| x - y);
                    let pct = diff
                        .zip(number(&prev))
                        .and_then(|(d, p)| (p != 0.).then_some(d / p * 100.));
                    row.extend([
                        diff.map_or(Value::Null, Value::Float),
                        pct.map_or(Value::Null, Value::Float),
                    ]);
                }
                if let Some(n) = extra {
                    row.extend([value(&cf, *id, n), value(&pf, partner, n)]);
                }
            }
            if content {
                let percent = content_percent(c, *id, partner)?;
                if percent.is_none() {
                    content_available = false;
                }
                if percent.is_some_and(|p| p > f64::from(req.content_threshold_pct)) {
                    *changed_counts.entry("Content".into()).or_default() += 1;
                    any_changed = true;
                }
                if (filter == "All" || filter == "content")
                    && percent.is_some_and(|p| p > f64::from(req.content_threshold_pct))
                {
                    changed = true;
                }
                row.push(percent.map_or(Value::Null, Value::Float));
            }
            if any_changed {
                changed_total += 1;
            }
            if req.change == Some("changed") && !changed
                || req.change == Some("notChanged") && changed
            {
                continue;
            }
            if let Some(column) = sort_column {
                order.push((row[column].clone(), *id));
            } else if total >= req.offset && cells.len() < (req.limit as usize) {
                cells.push(row);
                keys.push(*id);
                partners.push(partner);
            }
            total += 1;
        }
    }
    if let Some(sort) = req.sort {
        order.sort_by(|(a, id_a), (b, id_b)| {
            let cmp = sort_values(a, b);
            let cmp = if sort.dir == SortDir::Desc {
                cmp.reverse()
            } else {
                cmp
            };
            cmp.then_with(|| id_a.cmp(id_b))
        });
        cancel
            .checkpoint()
            .map_err(|_| CmdError::new(ErrorCode::Cancelled, "err-cancelled"))?;
        let selected = order
            .into_iter()
            .skip(usize::try_from(req.offset).unwrap_or(usize::MAX))
            .take(req.limit as usize)
            .map(|(_, id)| id)
            .collect();
        let page = project_page(
            c,
            elements,
            PageRequest {
                sort: None,
                offset: 0,
                ..req
            },
            cancel,
            Some(selected),
        )?;
        return Ok(
            json!({"columns":columns,"cells":page["cells"],"rowKeys":page["rowKeys"],"partnerKeys":page["partnerKeys"],"total":total,"side":"Current","filtersEnabled":true,"changedCounts":changed_counts,"changedTotal":changed_total,"contentAvailable":content_available}),
        );
    }
    Ok(
        json!({"columns":columns,"cells":cells,"rowKeys":keys,"partnerKeys":partners,"total":total,"side":"Current","filtersEnabled":true,"changedCounts":changed_counts,"changedTotal":changed_total,"contentAvailable":content_available}),
    )
}

// Null sorts as +infinity, including empty Change % cells with Previous=0.
// Reverse the complete ordering for descending; IDs break ties in either direction.
fn sort_values(a: &Value, b: &Value) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a, b) {
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Null, _) => Ordering::Greater,
        (_, Value::Null) => Ordering::Less,
        (Value::Text(a), Value::Text(b)) => {
            os_store::collation::compare(a, b).then_with(|| a.cmp(b))
        }
        _ => number(a).unwrap_or(0.).total_cmp(&number(b).unwrap_or(0.)),
    }
}

fn content_percent(c: &Comparison, current: u32, previous: u32) -> Result<Option<f64>, CmdError> {
    let read = |side: Side, id: u32| {
        os_store::queries::app_support::comparison_signature(
            c.side(side).file.conn(),
            UrlId(i64::from(id)),
        )
        .map_err(|e| compare_error(e.into()))
    };
    let (Some(a), Some(b)) = (
        read(Side::Current, current)?,
        read(Side::Previous, previous)?,
    ) else {
        return Ok(None);
    };
    if a == b {
        return Ok(Some(0.));
    }
    if a.is_empty() || b.is_empty() {
        return Ok(Some(100.));
    }
    if a.len() != 128 || b.len() != 128 {
        return Ok(None);
    }
    let same = a.iter().zip(&b).filter(|(x, y)| x == y).count() as f64;
    Ok(Some(((1. - same / 128.) * 10000.).round() / 100.))
}
