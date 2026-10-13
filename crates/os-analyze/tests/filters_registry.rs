//! `tabs::all()` stimmt mit `docs/plan/M1-filters.toml` überein (Tabs, Filter-Kennungen, Reihenfolge, Art,
//! Voraussetzungen). Der Test bleibt dauerhaft aktiv: Kennungen ändern sich nur mit der Registry.
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

#[test]
fn every_tab_is_a_real_definition_starting_with_the_address_column() {
    // Seit Welle 3 sind alle 16 Tabs echte Definitionen (keine Platzhalter mit einer Spalte und Grundmenge "1").
    for tab in tabs::all() {
        assert_eq!(
            tab.columns.first().map(|c| c.key),
            Some("Address"),
            "Address ist die erste Spalte von {}",
            tab.id
        );
        assert!(
            tab.columns.len() > 1,
            "{} hat nur eine Spalte (Platzhalter)",
            tab.id
        );
        assert!(!tab.population.is_empty(), "Grundmenge von {}", tab.id);
        assert_ne!(
            tab.population, "1",
            "{} hat noch die Grundmenge des Platzhalters",
            tab.id
        );
        let all: Vec<_> = tab.filters.iter().filter(|f| f.id == "All").collect();
        assert_eq!(all.len(), 1, "genau ein Filter All in {}", tab.id);
        assert!(
            !all[0].predicate.is_empty(),
            "Filter All von {} hat ein Prädikat",
            tab.id
        );
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
