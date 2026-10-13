//! Namensauflösung `Tab:Filter` und dynamische Filterlabels (M1-P14, ONPAGE-001/002/190), geprüft gegen die
//! kanonischen Kennungen aus `docs/plan/M1-filters.toml`.

use std::path::Path;

use os_analyze::names;
use os_analyze::views::resolve_name;
use os_analyze::NameError;
use os_model::CrawlConfig;

fn registry() -> toml::Table {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/plan/M1-filters.toml");
    std::fs::read_to_string(path)
        .expect("M1-filters.toml lesbar")
        .parse()
        .expect("gültiges TOML")
}

/// Ersetzt `{pfad}` durch `value`.
fn fill(id: &str, value: &str) -> String {
    let mut out = String::new();
    let mut rest = id;
    while let Some(open) = rest.find('{') {
        let close = rest[open..].find('}').unwrap();
        out.push_str(&rest[..open]);
        out.push_str(value);
        rest = &rest[open + close + 1..];
    }
    out.push_str(rest);
    out
}

// spec: ONPAGE-001
#[test]
fn every_canonical_name_resolves_case_insensitively_with_threshold_as_number_or_x() {
    let reg = registry();
    let mut checked = 0;
    for tab in reg["tab"].as_array().unwrap() {
        let tab_id = tab["id"].as_str().unwrap();
        for f in tab["filters"].as_array().unwrap() {
            let id = f["id"].as_str().unwrap();
            let expected = (tab_id.to_string(), id.to_string());
            for variant in [id.to_string(), fill(id, "7"), fill(id, "X"), fill(id, "x")] {
                for input in [
                    format!("{tab_id}:{variant}"),
                    format!("{}:{}", tab_id.to_lowercase(), variant.to_lowercase()),
                    format!("{}:{}", tab_id.to_uppercase(), variant.to_uppercase()),
                    format!("  {tab_id}  :  {variant}  "),
                ] {
                    assert_eq!(
                        resolve_name(&input).as_ref(),
                        Ok(&expected),
                        "Eingabe {input:?}"
                    );
                }
            }
            checked += 1;
        }
    }
    assert!(
        checked > 100,
        "alle Filter der Registry geprüft ({checked})"
    );
}

// spec: ONPAGE-001
#[test]
fn unknown_names_get_suggestions_within_distance_three() {
    let Err(NameError::Unknown { input, suggestions }) = resolve_name("Page Titles:Mising") else {
        panic!("muss scheitern");
    };
    assert_eq!(input, "Page Titles:Mising");
    assert_eq!(
        suggestions.first().map(String::as_str),
        Some("Page Titles:Missing")
    );

    // Falscher Tab-Name.
    let Err(NameError::Unknown { suggestions, .. }) = resolve_name("Page Title:Missing") else {
        panic!()
    };
    assert!(
        suggestions.contains(&"Page Titles:Missing".to_string()),
        "{suggestions:?}"
    );

    // Schwelle mit Tippfehler; Platzhalter erscheint als X.
    let Err(NameError::Unknown { suggestions, .. }) = resolve_name("Page Titles:Over 61 Charcters")
    else {
        panic!()
    };
    assert!(
        suggestions.contains(&"Page Titles:Over X Characters".to_string()),
        "{suggestions:?}"
    );

    // Nichts Ähnliches → keine Vorschläge; ohne Doppelpunkt ebenfalls ein Fehler.
    let Err(NameError::Unknown { suggestions, .. }) =
        resolve_name("Völlig:Anders und viel zu lang")
    else {
        panic!()
    };
    assert!(suggestions.is_empty());
    assert!(resolve_name("Internal").is_err());
    let Err(NameError::Unknown { suggestions, .. }) = resolve_name("Internl") else {
        panic!()
    };
    assert_eq!(
        suggestions.first().map(String::as_str),
        Some("Internal:All")
    );
}

// spec: ONPAGE-001, ONPAGE-002
#[test]
fn labels_resolve_placeholders_against_the_config() {
    let reg = registry();
    let cfg = CrawlConfig::default();
    let mut with_label = 0;
    for tab in reg["tab"].as_array().unwrap() {
        for f in tab["filters"].as_array().unwrap() {
            if let Some(default_label) = f.get("default_label").and_then(|l| l.as_str()) {
                let id = f["id"].as_str().unwrap();
                assert_eq!(names::label(id, &cfg), default_label, "Label von {id}");
                with_label += 1;
            }
        }
    }
    assert_eq!(with_label, 9, "neun Filter mit Schwellen-Platzhalter");

    let mut cfg = CrawlConfig::default();
    cfg.title.max_chars = 70;
    cfg.image.max_kb = 200;
    assert_eq!(
        names::label("Over {title.max_chars} Characters", &cfg),
        "Over 70 Characters"
    );
    assert_eq!(names::label("Over {image.max_kb} KB", &cfg), "Over 200 KB");
    assert_eq!(names::label("Ohne Platzhalter", &cfg), "Ohne Platzhalter");
    assert_eq!(
        names::label("Over {gibt.es.nicht} X", &cfg),
        "Over {gibt.es.nicht} X"
    );
}
