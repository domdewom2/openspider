//! `tabs::all()` stimmt mit `docs/plan/M1-filters.toml` überein (Tabs, Filter-Kennungen, Reihenfolge, Art,
//! Voraussetzungen). Der Test bleibt dauerhaft aktiv: P20–P25 dürfen Kennungen nicht ändern.
//! Infrastrukturtest ohne Spec-Tag.

use std::path::Path;

use os_analyze::tabs;
use os_analyze::{FilterKind, Requirement};

fn registry() -> toml::Table {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/plan/M1-filters.toml");
    let text = std::fs::read_to_string(&path).expect("M1-filters.toml lesbar");
    text.parse().expect("M1-filters.toml ist gültiges TOML")
}

fn kind_name(kind: FilterKind) -> String {
    match kind {
        FilterKind::Live => "live".to_string(),
        FilterKind::PostCrawl(step) => format!("post_crawl:{step:?}"),
    }
}

#[test]
fn tabs_match_filter_registry() {
    let reg = registry();
    let expected = reg["tab"].as_array().expect("[[tab]]-Liste");
    let actual = tabs::all();

    assert_eq!(expected.len(), 16, "M1 hat 16 Tabs");
    assert_eq!(actual.len(), expected.len(), "Anzahl der Tabs");

    for (exp, tab) in expected.iter().zip(&actual) {
        let tab_id = exp["id"].as_str().unwrap();
        assert_eq!(tab.id, tab_id, "Tab-Reihenfolge");

        let exp_filters = exp["filters"].as_array().unwrap();
        let ids: Vec<&str> = tab.filters.iter().map(|f| f.id).collect();
        let exp_ids: Vec<&str> = exp_filters
            .iter()
            .map(|f| f["id"].as_str().unwrap())
            .collect();
        assert_eq!(
            ids, exp_ids,
            "Filter-Kennungen und -Reihenfolge von {tab_id}"
        );

        for (ef, f) in exp_filters.iter().zip(&tab.filters) {
            let id = ef["id"].as_str().unwrap();
            assert_eq!(
                kind_name(f.kind),
                ef["kind"].as_str().unwrap(),
                "Art von {tab_id}:{id}"
            );
            let exp_requires: Vec<&str> = ef
                .get("requires")
                .and_then(|r| r.as_array())
                .map(|r| r.iter().map(|k| k.as_str().unwrap()).collect())
                .unwrap_or_default();
            let requires: Vec<&str> = f
                .requires
                .iter()
                .map(|r| match r {
                    Requirement::Config(key) => *key,
                    Requirement::Rendering => "rendering",
                })
                .collect();
            assert_eq!(requires, exp_requires, "Voraussetzungen von {tab_id}:{id}");
        }
    }
}

/// Tabs, deren Paket den Platzhalter ersetzt hat: Sie haben eine echte Grundmenge (M1-P25: Hreflang, Links,
/// Sitemaps); „All“ bleibt die Grundmenge (`1` über der Population).
const IMPLEMENTED: &[&str] = &["Hreflang", "Links", "Sitemaps"];

#[test]
fn placeholder_tabs_have_address_column_and_population() {
    for tab in tabs::all() {
        if !IMPLEMENTED.contains(&tab.id) {
            assert_eq!(tab.population, "1", "Grundmenge von {}", tab.id);
        }
        assert_eq!(tab.columns.first().map(|c| c.key), Some("Address"));
        let all: Vec<_> = tab.filters.iter().filter(|f| f.id == "All").collect();
        for f in all {
            assert_eq!(
                f.predicate, "1",
                "Filter All von {} ist die Grundmenge",
                tab.id
            );
        }
    }
}

#[test]
fn filter_ids_are_unique_per_tab() {
    for tab in tabs::all() {
        let mut ids: Vec<&str> = tab.filters.iter().map(|f| f.id).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n, "doppelte Filter-Kennung in {}", tab.id);
    }
}
