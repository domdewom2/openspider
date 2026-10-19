//! JSON-LD extraction without network context fetching (TECH-180).
use super::model::{self, Ordered};
use crate::PageCtx;
use os_model::SdItem;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

pub fn extract(ctx: &PageCtx<'_>) -> Vec<SdItem> {
    let sel = scraper::Selector::parse("script").expect("selector");
    let mut result = Vec::new();
    let mut roots = Vec::new();
    let mut search_from = 0;
    for (block, el) in ctx
        .doc
        .html()
        .select(&sel)
        .filter(|e| {
            e.value()
                .attr("type")
                .is_some_and(|t| t.trim().eq_ignore_ascii_case("application/ld+json"))
        })
        .enumerate()
    {
        let raw = el.text().collect::<String>();
        let source_offset = ctx.html[search_from..]
            .find(&raw)
            .map(|p| p + search_from)
            .unwrap_or(search_from);
        search_from = (source_offset + raw.len()).min(ctx.html.len());
        // Replace comment delimiters with spaces so parser positions remain relative to script content.
        let mut text = raw.clone();
        let start = text.len() - text.trim_start().len();
        let end = text.trim_end().len();
        if text[start..].starts_with("<!--") && text[..end].ends_with("-->") {
            text.replace_range(end - 3..end, "   ");
            text.replace_range(start..start + 4, "    ");
        }
        match serde_json::from_str::<Ordered>(&text) {
            Ok(Ordered(v)) => {
                if let Err(message) = check(&v, true) {
                    result.push(model::error(
                        "json-ld",
                        message,
                        block + 1,
                        1,
                        1,
                        &raw.chars().take(41).collect::<String>(),
                    ));
                    continue;
                }
                let line = ctx.html[..source_offset]
                    .bytes()
                    .filter(|b| *b == b'\n')
                    .count()
                    + 1;
                flatten(v, None, block + 1, line, &mut roots);
            }
            Err(e) => {
                let offset = text
                    .split_inclusive('\n')
                    .take(e.line().saturating_sub(1))
                    .map(str::len)
                    .sum::<usize>()
                    + e.column().saturating_sub(1);
                let before = raw.char_indices().take_while(|(p, _)| *p < offset).count();
                let snippet = raw
                    .chars()
                    .skip(before.saturating_sub(40))
                    .take(before.min(40) + 41)
                    .collect::<String>();
                result.push(model::error(
                    "json-ld",
                    &e.to_string(),
                    block + 1,
                    e.line(),
                    e.column(),
                    &snippet,
                ));
            }
        }
    }
    let mut ids = HashMap::new();
    for (v, _, _) in &roots {
        collect_ids(v, &mut ids);
    }
    let mut valid = Vec::new();
    for (mut v, block, line) in roots {
        resolve(&mut v, &ids, &mut HashSet::new(), 0);
        valid.push(model::item(
            "json-ld",
            v,
            json!({"block":block,"line":line}),
        ));
    }
    // Errors must remain in source order among valid blocks.
    result.extend(valid);
    result.sort_by_key(|i| {
        let v: Value = serde_json::from_str(&i.json).expect("internal JSON");
        v.pointer("/_os_source/block")
            .or_else(|| v.pointer("/_os_parse_error/block"))
            .and_then(Value::as_u64)
            .unwrap_or(0)
    });
    result
}
fn check(v: &Value, root: bool) -> Result<(), &'static str> {
    match v {
        Value::Object(o) => {
            if let Some(t) = o.get("@type") {
                if !t.is_string() && !t.as_array().is_some_and(|a| a.iter().all(Value::is_string)) {
                    return Err("@type must be a string or an array of strings");
                }
            }
            for (k, v) in o {
                if !k.starts_with("_os_") {
                    check(v, false)?;
                }
            }
        }
        Value::Array(a) => {
            for v in a {
                if root && !v.is_object() {
                    return Err("JSON-LD array entries must be objects");
                }
                check(v, false)?;
            }
        }
        _ if root => return Err("JSON-LD root must be an object or array"),
        _ => {}
    }
    Ok(())
}
fn flatten(
    v: Value,
    context: Option<Value>,
    block: usize,
    line: usize,
    out: &mut Vec<(Value, usize, usize)>,
) {
    match v {
        Value::Array(a) => {
            for v in a {
                flatten(v, context.clone(), block, line, out);
            }
        }
        Value::Object(mut o) => {
            let context = o.get("@context").cloned().or(context);
            if let Some(graph) = o.remove("@graph") {
                flatten(graph, context.clone(), block, line, out);
                if !o.contains_key("@type") {
                    return;
                }
            }
            if let Some(c) = context {
                o.entry("@context").or_insert(c);
            }
            let mut v = Value::Object(o);
            expand(&mut v, None);
            out.push((v, block, line));
        }
        _ => {}
    }
}
fn expand(v: &mut Value, inherited: Option<String>) {
    match v {
        Value::Object(o) => {
            let vocab = o
                .get("@context")
                .and_then(|c| {
                    c.as_str()
                        .or_else(|| c.get("@vocab").and_then(Value::as_str))
                })
                .map(|s| format!("{}/", s.trim_end_matches('/')))
                .or(inherited);
            if let Some(t) = o.get_mut("@type") {
                let expand_type = |v: &mut Value| {
                    if let Some(s) = v.as_str() {
                        if !s.contains(':') {
                            if let Some(vocab) = &vocab {
                                *v = json!(format!("{vocab}{s}"));
                            }
                        }
                    }
                };
                if let Some(a) = t.as_array_mut() {
                    for v in a {
                        expand_type(v);
                    }
                } else {
                    expand_type(t);
                }
            }
            for (k, v) in o {
                if !k.starts_with("_os_") && k != "@context" {
                    expand(v, vocab.clone());
                }
            }
        }
        Value::Array(a) => {
            for v in a {
                expand(v, inherited.clone());
            }
        }
        _ => {}
    }
}
fn collect_ids(v: &Value, ids: &mut HashMap<String, Value>) {
    match v {
        Value::Object(o) => {
            if let Some(id) = o.get("@id").and_then(Value::as_str) {
                if o.keys()
                    .any(|k| !k.starts_with("_os_") && k != "@id" && k != "@context")
                {
                    ids.insert(id.into(), v.clone());
                }
            }
            for (k, v) in o {
                if !k.starts_with("_os_") {
                    collect_ids(v, ids);
                }
            }
        }
        Value::Array(a) => {
            for v in a {
                collect_ids(v, ids);
            }
        }
        _ => {}
    }
}
fn resolve(v: &mut Value, ids: &HashMap<String, Value>, seen: &mut HashSet<String>, depth: usize) {
    if depth > 64 {
        return;
    }
    let id = v.get("@id").and_then(Value::as_str).map(str::to_owned);
    if let Some(id) = &id {
        if !seen.insert(id.clone()) {
            return;
        }
        if let (Some(target), Some(o)) = (ids.get(id).and_then(Value::as_object), v.as_object_mut())
        {
            let mut order = o
                .get("_os_order")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if let Some(target_order) = target.get("_os_order").and_then(Value::as_array) {
                for key in target_order {
                    if !order.contains(key) {
                        order.push(key.clone());
                    }
                }
            }
            for (k, value) in target {
                o.entry(k.clone()).or_insert_with(|| value.clone());
            }
            o.insert("_os_order".into(), Value::Array(order));
        }
    }
    match v {
        Value::Object(o) => {
            for (k, v) in o {
                if !k.starts_with("_os_") {
                    resolve(v, ids, seen, depth + 1);
                }
            }
        }
        Value::Array(a) => {
            for v in a {
                resolve(v, ids, seen, depth + 1);
            }
        }
        _ => {}
    }
    if let Some(id) = id {
        seen.remove(&id);
    }
}
