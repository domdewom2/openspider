//! Issue-Texte (UI-065-Kernanteil, UI-064): Vollständigkeit, Längen, Platzhalter, Doku-Seiten.

use std::path::{Path, PathBuf};

use os_analyze::issues::{self, catalog, doc_page, raw_texts, texts, Lang, ALLOWED_PLACEHOLDERS};
use os_model::Value;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn chars(s: &str) -> usize {
    s.chars().count()
}

/// Erster Platzhalter im Muster `\{[a-z_]+\}`.
fn find_placeholder(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            let mut j = i + 1;
            while j < bytes.len() && (bytes[j].is_ascii_lowercase() || bytes[j] == b'_') {
                j += 1;
            }
            if j > i + 1 && j < bytes.len() && bytes[j] == b'}' {
                return Some(s[i..=j].to_string());
            }
        }
        i += 1;
    }
    None
}

// spec: UI-064
#[test]
fn every_catalog_entry_has_what_why_fix_in_de_and_en() {
    for e in catalog() {
        for lang in Lang::ALL {
            let t = raw_texts(&e.doc_key, *lang)
                .unwrap_or_else(|| panic!("{}: Texte ({}) fehlen", e.id, lang.code()));
            assert!(!t.what.trim().is_empty(), "{} {} what", e.id, lang.code());
            assert!(!t.why.trim().is_empty(), "{} {} why", e.id, lang.code());
            assert!(!t.fix.trim().is_empty(), "{} {} fix", e.id, lang.code());
        }
    }
}

// spec: UI-064
#[test]
fn text_files_have_no_orphan_entries() {
    for lang in Lang::ALL {
        let name = format!("core/issues/texts/{}.toml", lang.code());
        let table: toml::Table = std::fs::read_to_string(repo_root().join(&name))
            .unwrap()
            .parse()
            .unwrap();
        let ids: Vec<&String> = table["issues"].as_table().unwrap().keys().collect();
        assert_eq!(ids.len(), catalog().len(), "{name}: Anzahl der Einträge");
        for id in ids {
            assert!(
                catalog().iter().any(|e| &e.id == id),
                "{name}: Text ohne Katalogeintrag {id}"
            );
        }
    }
}

// spec: UI-064
#[test]
fn lengths_after_replacement_are_within_limits() {
    let mut bad = Vec::new();
    for e in catalog() {
        for lang in Lang::ALL {
            let t = texts(&e.doc_key, *lang, &[]).unwrap();
            for (name, text, range) in [
                ("what", &t.what, 40..=300),
                ("why", &t.why, 40..=400),
                ("fix", &t.fix, 40..=600),
            ] {
                let n = chars(text);
                if !range.contains(&n) {
                    bad.push(format!("{} {} {name}: {n} Zeichen", e.id, lang.code()));
                }
            }
        }
    }
    assert!(bad.is_empty(), "Längenverstöße: {bad:#?}");
}

// spec: UI-064
#[test]
fn no_unreplaced_placeholder_with_defaults_or_changed_thresholds() {
    // Geänderte Werte für alle Schwellen, die im Katalog vorkommen.
    let changed: Vec<(&str, Value)> = vec![
        ("title.max_chars", Value::Int(65)),
        ("title.min_chars", Value::Int(33)),
        ("meta_description.max_chars", Value::Int(160)),
        ("meta_description.min_chars", Value::Int(66)),
        ("url.max_chars", Value::Int(120)),
        ("h1.max_chars", Value::Int(72)),
        ("h2.max_chars", Value::Int(74)),
        ("image.max_kb", Value::Int(150)),
        ("image.alt_max_chars", Value::Int(125)),
        ("content.low_word_count", Value::Int(250)),
        ("links.high_crawl_depth", Value::Int(6)),
    ];
    for e in catalog() {
        for lang in Lang::ALL {
            for params in [&[][..], &changed[..]] {
                let t = texts(&e.doc_key, *lang, params).unwrap();
                for (part, text) in [("what", &t.what), ("why", &t.why), ("fix", &t.fix)] {
                    assert_eq!(
                        find_placeholder(text),
                        None,
                        "{} {} {part}: unersetzter Platzhalter",
                        e.id,
                        lang.code()
                    );
                }
            }
        }
    }
    // Rohtexte nennen nur erlaubte Platzhalter, die der Eintrag in text_params deklariert.
    for e in catalog() {
        for lang in Lang::ALL {
            let t = raw_texts(&e.doc_key, *lang).unwrap();
            for text in [&t.what, &t.why, &t.fix] {
                let mut rest = text.as_str();
                while let Some(ph) = find_placeholder(rest) {
                    let name = ph.trim_matches(|c| c == '{' || c == '}').to_string();
                    assert!(
                        ALLOWED_PLACEHOLDERS.contains(&name.as_str()),
                        "{}: {ph}",
                        e.id
                    );
                    assert!(
                        e.text_params.iter().any(|(p, _)| *p == name),
                        "{}: {ph} ohne text_params",
                        e.id
                    );
                    rest = &rest[rest.find(&ph).unwrap() + ph.len()..];
                }
            }
        }
    }
}

// spec: UI-064
#[test]
fn changed_threshold_appears_in_the_text() {
    let params = [("title.max_chars", Value::Int(65))];
    for lang in Lang::ALL {
        let t = texts("page-titles-over-max-chars", *lang, &params).unwrap();
        assert!(t.what.contains("65"), "{}: {}", lang.code(), t.what);
        assert!(t.fix.contains("65"), "{}: {}", lang.code(), t.fix);
        let default = texts("page-titles-over-max-chars", *lang, &[]).unwrap();
        assert!(default.what.contains("60"), "{}", default.what);
    }
    // Der benannte SQL-Parameter (`:name`, `_` statt `.`) wird ebenso gefunden.
    let sql_style = [(":title_min_chars", Value::Int(35))];
    let t = texts("page-titles-below-min-chars", Lang::En, &sql_style).unwrap();
    assert!(t.what.contains("35"), "{}", t.what);
    // Float-Werte ohne Nachkommastellen erscheinen als Ganzzahl.
    let t = texts(
        "images-over-max-kb",
        Lang::En,
        &[("image.max_kb", Value::Float(150.0))],
    )
    .unwrap();
    assert!(
        t.what.contains("150 KB") && !t.what.contains("150.0"),
        "{}",
        t.what
    );
}

// spec: UI-064
#[test]
fn every_threshold_filter_carries_its_value_in_the_text() {
    // Filter mit Platzhalter im Namen: der Text nennt den Schwellenwert (Standardwert) in beiden Sprachen.
    for e in catalog().iter().filter(|e| !e.text_params.is_empty()) {
        for lang in Lang::ALL {
            let t = texts(&e.doc_key, *lang, &[]).unwrap();
            let all = format!("{} {} {}", t.what, t.why, t.fix);
            let (_, path) = &e.text_params[0];
            let default = match path.as_str() {
                "title.max_chars" => "60",
                "title.min_chars" => "30",
                "meta_description.max_chars" => "155",
                "meta_description.min_chars" => "70",
                "url.max_chars" => "115",
                "h1.max_chars" | "h2.max_chars" => "70",
                "image.max_kb" | "image.alt_max_chars" => "100",
                "content.low_word_count" => "200",
                "links.high_crawl_depth" => "4",
                other => panic!("{}: unerwarteter Pfad {other}", e.id),
            };
            assert!(
                all.contains(default),
                "{} {}: Standardwert {default} fehlt",
                e.id,
                lang.code()
            );
        }
    }
}

// spec: UI-064
#[test]
fn unknown_doc_key_has_no_texts() {
    assert!(texts("does-not-exist", Lang::De, &[]).is_none());
    assert!(raw_texts("does-not-exist", Lang::En).is_none());
}

fn docs_dir() -> PathBuf {
    repo_root().join("docs/issues")
}

// spec: UI-064
#[test]
fn doc_pages_exist_and_match_the_generated_content() {
    let update = std::env::var("UPDATE_GOLDEN").is_ok_and(|v| v == "1");
    for e in catalog() {
        let path = docs_dir().join(format!("{}.md", e.doc_key));
        let expected = doc_page(e);
        if update {
            std::fs::write(&path, &expected).unwrap();
        }
        let actual = std::fs::read_to_string(&path).unwrap_or_else(|_| {
            panic!(
                "{} fehlt (UPDATE_GOLDEN=1 erzeugt die Seiten)",
                path.display()
            )
        });
        assert_eq!(
            actual,
            expected,
            "{} weicht ab (UPDATE_GOLDEN=1)",
            path.display()
        );
        assert!(actual.starts_with(&format!("# {}: ", e.tab)));
        assert!(actual.contains("## Deutsch") && actual.contains("## English"));
    }
}

// spec: UI-064
#[test]
fn doc_directory_has_no_stale_pages() {
    for entry in std::fs::read_dir(docs_dir()).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "README.md" {
            continue;
        }
        let key = name
            .strip_suffix(".md")
            .unwrap_or_else(|| panic!("unerwartete Datei {name}"));
        assert!(
            issues::entry_by_doc_key(key).is_some(),
            "Doku-Seite ohne Katalogeintrag: {name}"
        );
    }
}

// spec: UI-064
#[test]
fn filter_labels_resolve_thresholds() {
    let e = issues::entry_by_doc_key("page-titles-over-max-chars").unwrap();
    assert_eq!(issues::filter_label(e, &[]), "Over 60 Characters");
    assert_eq!(
        issues::filter_label(e, &[("title.max_chars", Value::Int(65))]),
        "Over 65 Characters"
    );
    let plain = issues::entry_by_doc_key("url-uppercase").unwrap();
    assert_eq!(issues::filter_label(plain, &[]), "Uppercase");
}
