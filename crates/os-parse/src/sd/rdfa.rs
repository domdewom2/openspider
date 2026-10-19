//! RDFa 1.1 / RDFa Lite schema.org extraction (TECH-182).
use super::model;
use crate::PageCtx;
use os_model::SdItem;
use scraper::ElementRef;
use serde_json::{json, Map, Value};
use std::collections::HashMap;

#[derive(Clone, Default)]
struct Context {
    vocab: Option<String>,
    prefixes: HashMap<String, String>,
}
impl Context {
    fn at(mut self, el: ElementRef<'_>) -> Self {
        if let Some(v) = el.value().attr("vocab") {
            self.vocab = (!v.is_empty()).then(|| v.into());
        }
        if let Some(p) = el.value().attr("prefix") {
            let parts: Vec<_> = p.split_whitespace().collect();
            for pair in parts.as_chunks::<2>().0 {
                self.prefixes
                    .insert(pair[0].trim_end_matches(':').into(), pair[1].into());
            }
        }
        self
    }
    fn iri(&self, raw: &str) -> Option<String> {
        let raw = raw.trim_matches(['[', ']']);
        if let Some((prefix, local)) = raw.split_once(':') {
            if let Some(base) = self.prefixes.get(prefix) {
                return Some(format!("{base}{local}"));
            }
        }
        if url::Url::parse(raw).is_ok() {
            return Some(raw.into());
        }
        self.vocab.as_ref().map(|v| format!("{v}{raw}"))
    }
}
fn schema(iri: &str) -> bool {
    iri.starts_with("https://schema.org/") || iri.starts_with("http://schema.org/")
}
pub fn extract(ctx: &PageCtx<'_>) -> Vec<SdItem> {
    let mut out = Vec::new();
    visit(
        ctx,
        ctx.doc.html().root_element(),
        Context::default(),
        &mut out,
        0,
    );
    out
}
fn visit(
    ctx: &PageCtx<'_>,
    el: ElementRef<'_>,
    inherited: Context,
    out: &mut Vec<SdItem>,
    depth: usize,
) {
    if depth > 256 {
        return;
    }
    let c = inherited.at(el);
    if el.value().attr("typeof").is_some() {
        match read(ctx, el, c.clone(), 0) {
            Ok(Some(v)) => {
                out.push(model::item(
                    "rdfa",
                    v,
                    json!({"path":crate::dom::xpath(el)}),
                ));
                return;
            }
            Err(e) => {
                out.push(model::error(
                    "rdfa",
                    &e,
                    out.len() + 1,
                    1,
                    1,
                    &el.html().chars().take(80).collect::<String>(),
                ));
                return;
            }
            Ok(None) => {}
        }
    }
    for child in el.children().filter_map(ElementRef::wrap) {
        visit(ctx, child, c.clone(), out, depth + 1);
    }
}
fn read(
    ctx: &PageCtx<'_>,
    el: ElementRef<'_>,
    c: Context,
    depth: usize,
) -> Result<Option<Value>, String> {
    if depth > 64 {
        return Ok(None);
    }
    let mut types = Vec::new();
    for raw in el.value().attr("typeof").unwrap_or("").split_whitespace() {
        let iri = c
            .iri(raw)
            .ok_or_else(|| format!("typeof contains an unresolved IRI: {raw}"))?;
        if url::Url::parse(&iri).is_err() {
            return Err(format!("typeof contains an unresolved IRI: {raw}"));
        }
        if schema(&iri) {
            types.push(iri);
        }
    }
    if types.is_empty() {
        return Ok(None);
    }
    let mut obj = Map::new();
    obj.insert("@type".into(), json!(types));
    if let Some(id) = el
        .value()
        .attr("about")
        .or_else(|| el.value().attr("resource"))
    {
        obj.insert(
            "@id".into(),
            json!(ctx
                .base
                .join(id)
                .map(|u| u.to_string())
                .unwrap_or_else(|_| id.into())),
        );
    }
    for child in el.children().filter_map(ElementRef::wrap) {
        properties(ctx, child, c.clone(), &mut obj, depth + 1)?;
    }
    Ok(Some(Value::Object(obj)))
}
fn properties(
    ctx: &PageCtx<'_>,
    el: ElementRef<'_>,
    inherited: Context,
    obj: &mut Map<String, Value>,
    depth: usize,
) -> Result<(), String> {
    if depth > 256 {
        return Ok(());
    }
    let c = inherited.at(el);
    let nested = if el.value().attr("typeof").is_some() {
        read(ctx, el, c.clone(), depth)?
    } else {
        None
    };
    if let Some(props) = el.value().attr("property") {
        for raw in props.split_whitespace() {
            if let Some(iri) = c.iri(raw).filter(|i| schema(i)) {
                model::add(
                    obj,
                    model::short_type(&iri),
                    nested
                        .clone()
                        .unwrap_or_else(|| json!(model::value(el, ctx.base, false))),
                );
            }
        }
    }
    if el.value().attr("typeof").is_none() {
        for child in el.children().filter_map(ElementRef::wrap) {
            properties(ctx, child, c.clone(), obj, depth + 1)?;
        }
    }
    Ok(())
}
