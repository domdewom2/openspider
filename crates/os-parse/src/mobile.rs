//! DOM-Fakten für Mobile (TECH-236/237), unabhängig von der PSI-Anbindung.
use crate::{alternates, dom, PageCtx};
use os_model::{ExtKey, ParsedPage, Value};

fn plugin_url(value: &str) -> bool {
    let path = value
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    [".swf", ".xap", ".class", ".jar"]
        .iter()
        .any(|ext| path.ends_with(ext))
}

/// Prüft den bereits aufgebauten Baum; Skripttexte zählen nicht als Elemente.
pub fn run(ctx: &PageCtx<'_>, page: &mut ParsedPage) {
    let failed = ctx
        .doc
        .html()
        .tree
        .nodes()
        .filter_map(scraper::ElementRef::wrap)
        .any(|el| {
            if dom::in_template(el) {
                return false;
            }
            let e = el.value();
            match e.name() {
                "applet" => true,
                "object" | "embed" => {
                    let mime = e
                        .attr("type")
                        .unwrap_or_default()
                        .trim()
                        .to_ascii_lowercase();
                    let class = e
                        .attr("classid")
                        .unwrap_or_default()
                        .trim()
                        .to_ascii_lowercase();
                    matches!(
                        mime.as_str(),
                        "application/x-shockwave-flash" | "application/futuresplash"
                    ) || mime.starts_with("application/x-silverlight")
                        || mime.starts_with("application/x-java-applet")
                        || [
                            "d27cdb6e-ae6d-11cf-96b8-444553540000",
                            "dfeaf541-f3e1-4c24-acac-99c30715084a",
                            "8ad9c840-044e-11d1-b3e9-00805f499d93",
                            "cafeefac-",
                        ]
                        .iter()
                        .any(|id| {
                            class
                                .strip_prefix("clsid:")
                                .unwrap_or(&class)
                                .starts_with(id)
                        })
                        || ["src", "data"]
                            .iter()
                            .any(|a| e.attr(a).is_some_and(plugin_url))
                }
                "param" => {
                    e.attr("name")
                        .is_some_and(|n| n.eq_ignore_ascii_case("movie"))
                        && e.attr("value").is_some_and(|v| {
                            v.split(['?', '#'])
                                .next()
                                .unwrap_or_default()
                                .to_ascii_lowercase()
                                .ends_with(".swf")
                        })
                }
                _ => false,
            }
        });
    page.ext.push((
        ExtKey::from_static("mobile.plugins"),
        Value::Int(if failed { 0 } else { 100 }),
    ));
    if ctx.cfg.crawl.spider.mobile_alternate.store {
        let html = alternates::extract(ctx.doc, ctx.base, ctx.cfg);
        let headers = alternates::from_headers(ctx.url, ctx.headers, ctx.cfg);
        let targets: Vec<_> = html
            .iter()
            .chain(&headers)
            .filter_map(|l| l.resolved.as_ref())
            .collect();
        if let Some(first) = targets.first() {
            page.ext.push((
                ExtKey::from_static("mobile.alternate"),
                Value::Text((*first).clone()),
            ));
        }
        page.ext.push((
            ExtKey::from_static("mobile.alternate_count"),
            Value::Int(targets.len() as i64),
        ));
    }
}
