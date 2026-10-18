//! Lesezeichenziele im HTML5-Baum (ONPAGE-079).
use std::collections::BTreeSet;
use std::sync::OnceLock;

use os_model::{ExtKey, Value};
use scraper::Selector;

/// JSON-Liste der exakten IDs und alten Ankernamen; auch eine leere Liste ist ein bekanntes Ergebnis.
pub fn run(ctx: &crate::PageCtx<'_>, page: &mut os_model::ParsedPage) {
    static SELECTOR: OnceLock<Selector> = OnceLock::new();
    let selector =
        SELECTOR.get_or_init(|| Selector::parse("[id], a[name]").expect("bookmark selector"));
    let mut targets = BTreeSet::new();
    for element in ctx
        .doc
        .html()
        .select(selector)
        .filter(|e| !crate::dom::in_template(*e))
    {
        if let Some(id) = element.value().attr("id") {
            targets.insert(id);
        }
        if element.value().name() == "a" {
            if let Some(name) = element.value().attr("name") {
                targets.insert(name);
            }
        }
    }
    page.ext.push((
        ExtKey::from_static("urlfilter.bookmark_targets"),
        Value::Text(serde_json::to_string(&targets).expect("string list")),
    ));
}
