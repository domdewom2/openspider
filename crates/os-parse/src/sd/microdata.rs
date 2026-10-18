//! HTML Microdata extraction (TECH-181), bounded traversal with itemref cycle detection.
use super::model;
use crate::PageCtx;
use os_model::SdItem;
use scraper::ElementRef;
use serde_json::{json, Map, Value};
use std::collections::HashSet;

pub fn extract(ctx: &PageCtx<'_>) -> Vec<SdItem> {
    let sel = scraper::Selector::parse("[itemscope]").expect("selector");
    ctx.doc
        .html()
        .select(&sel)
        .filter(|e| e.value().attr("itemprop").is_none())
        .enumerate()
        .map(|(n, el)| match read(ctx, el, &mut HashSet::new(), 0) {
            Ok(v) => model::item(
                "microdata",
                v,
                json!({"item":n+1,"path":crate::dom::xpath(el)}),
            ),
            Err(e) => model::error(
                "microdata",
                &e,
                n + 1,
                1,
                1,
                &el.html().chars().take(80).collect::<String>(),
            ),
        })
        .collect()
}
fn read(
    ctx: &PageCtx<'_>,
    el: ElementRef<'_>,
    active: &mut HashSet<ego_tree::NodeId>,
    depth: usize,
) -> Result<Value, String> {
    if depth > 64 || !active.insert(el.id()) {
        return Ok(json!({}));
    }
    let mut obj = Map::new();
    let types: Vec<_> = el
        .value()
        .attr("itemtype")
        .unwrap_or("")
        .split_whitespace()
        .collect();
    if types.iter().any(|t| url::Url::parse(t).is_err()) {
        return Err("itemtype must contain absolute IRIs".into());
    }
    if !types.is_empty() {
        obj.insert("@type".into(), json!(types));
    }
    if let Some(id) = el.value().attr("itemid") {
        obj.insert(
            "@id".into(),
            json!(ctx
                .base
                .join(id)
                .map(|u| u.to_string())
                .unwrap_or_else(|_| id.into())),
        );
    }
    let mut visited = HashSet::new();
    for child in el.children().filter_map(ElementRef::wrap) {
        walk(ctx, child, &mut obj, active, &mut visited, depth)?;
    }
    if let Some(refs) = el.value().attr("itemref") {
        let selector = scraper::Selector::parse("[id]").expect("selector");
        for id in refs.split_whitespace() {
            if let Some(target) = ctx
                .doc
                .html()
                .select(&selector)
                .find(|e| e.value().attr("id") == Some(id))
            {
                walk(ctx, target, &mut obj, active, &mut visited, depth)?;
            }
        }
    }
    active.remove(&el.id());
    Ok(Value::Object(obj))
}
fn walk(
    ctx: &PageCtx<'_>,
    el: ElementRef<'_>,
    obj: &mut Map<String, Value>,
    active: &mut HashSet<ego_tree::NodeId>,
    visited: &mut HashSet<ego_tree::NodeId>,
    depth: usize,
) -> Result<(), String> {
    if depth > 256 {
        return Ok(());
    }
    if !visited.insert(el.id()) || active.contains(&el.id()) {
        return Ok(());
    }
    let scope = el.value().attr("itemscope").is_some();
    if let Some(props) = el.value().attr("itemprop") {
        let v = if scope {
            read(ctx, el, active, depth + 1)?
        } else {
            json!(model::value(el, ctx.base, true))
        };
        for prop in props.split_whitespace() {
            model::add(obj, prop, v.clone());
        }
    }
    if !scope {
        for child in el.children().filter_map(ElementRef::wrap) {
            walk(ctx, child, obj, active, visited, depth + 1)?;
        }
    }
    Ok(())
}
