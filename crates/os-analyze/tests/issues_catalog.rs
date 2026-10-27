//! Issue-Katalog (UI-063/UI-064, ONPAGE-006): Gültigkeit, Herleitung, Benchmark, Registry-Abgleich,
//! `issues_from_counts`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use os_analyze::issues::{
    self, catalog, issues_from_counts, override_share, parse_catalog, validate, CatalogError,
};
use os_analyze::tabs;
use os_analyze::views::FilterCount;
use os_analyze::FilterKind;
use os_model::{FilterState, IssuePriority, IssueType};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn registry() -> toml::Table {
    let text = std::fs::read_to_string(repo_root().join("docs/plan/M1-filters.toml"))
        .expect("M1-filters.toml lesbar");
    text.parse().expect("gültiges TOML")
}

/// (Tab, Filter) aller Registry-Filter mit `issue = true`.
fn registry_issue_filters() -> BTreeSet<(String, String)> {
    let reg = registry();
    let mut out = BTreeSet::new();
    for tab in reg["tab"].as_array().unwrap() {
        for f in tab["filters"].as_array().unwrap() {
            if f.get("issue").and_then(|v| v.as_bool()) == Some(true) {
                out.insert((
                    tab["id"].as_str().unwrap().to_string(),
                    f["id"].as_str().unwrap().to_string(),
                ));
            }
        }
    }
    out
}

// spec: UI-063, UI-064
#[test]
fn catalog_ids_are_in_registry_and_all_issue_filters_are_in_catalog() {
    let reg = registry();
    let mut all: BTreeSet<(String, String)> = reg["tab"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|t| {
            let tab = t["id"].as_str().unwrap().to_string();
            t["filters"]
                .as_array()
                .unwrap()
                .iter()
                .map(move |f| (tab.clone(), f["id"].as_str().unwrap().to_string()))
        })
        .collect();
    let mut m2: toml::Table =
        std::fs::read_to_string(repo_root().join("docs/plan/M2-filters.toml"))
            .unwrap()
            .parse()
            .unwrap();
    // spec: TECH-251 / UI-064: expected expansion of the dynamic axe registry entry.
    let a11y = m2
        .get_mut("tab")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|tab| tab["id"].as_str() == Some("Accessibility"))
        .unwrap();
    a11y.get_mut("filters")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .extend(
            include_str!("../../../tests/fixtures/ui/fx-ui-a11y/rule-ids.txt")
                .lines()
                .map(|id| {
                    toml::Value::Table(
                        [
                            ("id".into(), toml::Value::String(id.into())),
                            ("issue".into(), toml::Value::Boolean(true)),
                        ]
                        .into_iter()
                        .collect(),
                    )
                }),
        );
    for tab in m2["tab"].as_array().unwrap() {
        for filter in tab["filters"].as_array().unwrap() {
            all.insert((
                tab["id"].as_str().unwrap().to_string(),
                filter["id"].as_str().unwrap().to_string(),
            ));
        }
    }
    let in_catalog: BTreeSet<(String, String)> = catalog()
        .iter()
        .map(|e| (e.tab.clone(), e.filter.clone()))
        .collect();

    for key in &in_catalog {
        assert!(
            all.contains(key),
            "Katalogeintrag {key:?} fehlt in M1-filters.toml"
        );
    }
    let wanted = registry_issue_filters();

    for key in &wanted {
        assert!(
            in_catalog.contains(key),
            "issue = true ohne Katalogeintrag: {key:?}"
        );
    }
    // Umgekehrt: Katalogeinträge nur für Filter mit issue = true.
    for key in in_catalog.difference(&wanted) {
        assert!(
            m2["tab"].as_array().unwrap().iter().any(|tab| {
                tab["id"].as_str() == Some(&key.0)
                    && tab["filters"].as_array().unwrap().iter().any(|f| {
                        f["id"].as_str() == Some(&key.1)
                            && f.get("issue").and_then(toml::Value::as_bool) == Some(true)
                    })
            }),
            "M2-Katalogeintrag ohne Registry-Befund: {key:?}"
        );
    }
}

// spec: UI-063, UI-064
#[test]
fn catalog_matches_tabs_all_registry() {
    let tabs = tabs::all();
    for e in catalog() {
        let tab = tabs
            .iter()
            .find(|t| t.id == e.tab)
            .unwrap_or_else(|| panic!("Tab {:?} (Eintrag {}) fehlt in tabs::all()", e.tab, e.id));
        let filter = tab
            .filters
            .iter()
            .find(|f| f.id == e.filter)
            .unwrap_or_else(|| panic!("Filter {:?} fehlt im Tab {}", e.filter, e.tab));
        let post_crawl = matches!(filter.kind, FilterKind::PostCrawl(_));
        assert_eq!(
            e.needs_crawl_analysis, post_crawl,
            "needs_crawl_analysis von {} passt nicht zur Art des Filters",
            e.id
        );
    }
}

// spec: UI-063
#[test]
fn catalog_passes_all_build_checks() {
    let problems = validate(catalog());
    assert!(problems.is_empty(), "Katalogprobleme: {problems:#?}");
    assert!(!catalog().is_empty());
}

// spec: UI-063
#[test]
fn types_and_priorities_are_valid_values() {
    let raw = |t: &str, p: &str| {
        format!(
            "[[issue]]\nid = \"x\"\ntab = \"URL\"\nfilter = \"Uppercase\"\ntype = \"{t}\"\npriority = \"{p}\"\n\
             type_rule = \"T-W2\"\npriority_rule = \"P-L1\"\ndoc_key = \"x\"\n"
        )
    };
    assert!(parse_catalog(&raw("Warning", "Low")).is_ok());
    let bad_type = parse_catalog(&raw("Bug", "Low"));
    assert!(
        matches!(bad_type, Err(CatalogError::Parse(_))),
        "{bad_type:?}"
    );
    let bad_prio = parse_catalog(&raw("Warning", "Urgent"));
    assert!(
        matches!(bad_prio, Err(CatalogError::Parse(_))),
        "{bad_prio:?}"
    );
}

// spec: UI-063
#[test]
fn rule_prefix_matches_type_and_priority_or_override_is_given() {
    for e in catalog() {
        let type_by_rule = match &e.type_rule[..3] {
            "T-I" => IssueType::Issue,
            "T-W" => IssueType::Warning,
            "T-O" => IssueType::Opportunity,
            other => panic!("{}: unbekannter Typ-Präfix {other}", e.id),
        };
        let prio_by_rule = match &e.priority_rule[..3] {
            "P-H" => IssuePriority::High,
            "P-M" => IssuePriority::Medium,
            "P-L" => IssuePriority::Low,
            other => panic!("{}: unbekannter Prioritäts-Präfix {other}", e.id),
        };
        let deviates = type_by_rule != e.issue_type || prio_by_rule != e.priority;
        assert_eq!(
            deviates,
            e.override_reason.is_some(),
            "{}: override_reason genau dann, wenn Typ/Priorität von den Regeln abweichen",
            e.id
        );
        if let Some(reason) = &e.override_reason {
            assert!(reason.starts_with("benchmark-parity"), "{}: {reason}", e.id);
        }
    }
}

// spec: UI-063
#[test]
fn negative_issue_with_warning_rule_and_no_reason_fails() {
    // T-W1 ergibt „Warning“; ein Eintrag mit type = Issue braucht eine Begründung (T-W1-Negativtest).
    let src = "[[issue]]\nid = \"x\"\ntab = \"Directives\"\nfilter = \"Noindex\"\ntype = \"Issue\"\n\
               priority = \"High\"\ntype_rule = \"T-W1\"\npriority_rule = \"P-H1\"\ndoc_key = \"x\"\n";
    let entries = parse_catalog(src).unwrap();
    let problems = validate(&entries);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].message.contains("override_reason fehlt"));

    // Mit Begründung ist derselbe Eintrag zulässig.
    let with_reason = src.replace(
        "doc_key = \"x\"\n",
        "doc_key = \"x\"\noverride_reason = \"benchmark-parity: Test\"\n",
    );
    assert!(validate(&parse_catalog(&with_reason).unwrap()).is_empty());

    // Eine nicht erlaubte Begründung wird abgelehnt, ebenso unbekannte Regelcodes.
    let bad_reason = src.replace(
        "doc_key = \"x\"\n",
        "doc_key = \"x\"\noverride_reason = \"weil ich es will\"\n",
    );
    assert_eq!(validate(&parse_catalog(&bad_reason).unwrap()).len(), 1);
    let bad_rule = src.replace("T-W1", "T-X9");
    assert!(!validate(&parse_catalog(&bad_rule).unwrap()).is_empty());
}

// spec: UI-063
#[test]
fn override_share_is_reported_and_below_ten_percent() {
    let share = override_share(catalog());
    let n = catalog()
        .iter()
        .filter(|e| e.override_reason.is_some())
        .count();
    println!(
        "Katalog: {} Einträge, {} mit override_reason ({:.1} %)",
        catalog().len(),
        n,
        share * 100.0
    );
    assert!(
        share < 0.10,
        "Anteil override_reason {:.1} % ≥ 10 %",
        share * 100.0
    );
}

// spec: UI-063
#[test]
fn there_is_exactly_one_classification_file() {
    let dir = repo_root().join("core/issues");
    let mut tomls: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".toml"))
        .collect();
    tomls.sort();
    assert_eq!(
        tomls,
        vec!["catalog.toml".to_string()],
        "zweite Klassifizierungstabelle in core/issues"
    );
}

// spec: UI-064
#[test]
fn catalog_header_carries_the_benchmark_note() {
    const NOTE: &str = "Klassifizierung abgeleitet aus öffentlicher Dokumentation der Referenz (Benchmark), begründet nach UI-063";
    let src = std::fs::read_to_string(repo_root().join("core/issues/catalog.toml")).unwrap();
    assert!(src.contains(NOTE));
    let readme = std::fs::read_to_string(repo_root().join("docs/issues/README.md")).unwrap();
    assert!(readme.contains(NOTE));
}

type Bench = (&'static str, &'static str, IssueType, IssuePriority);

/// Benchmark-Erwartungen aus UI-064 (Kategorien Response Codes … Links), soweit sie M1-Filter betreffen.
/// Namen in der Schreibweise der Registry; Schwellenfilter mit Platzhalter.
fn benchmark() -> Vec<Bench> {
    use IssuePriority::{High as H, Low as L, Medium as M};
    use IssueType::{Issue as I, Opportunity as O, Warning as W};
    vec![
        ("Response Codes", "Internal No Response", I, H),
        ("Response Codes", "Internal Client Error (4XX)", I, H),
        ("Response Codes", "Internal Server Error (5XX)", I, H),
        ("Response Codes", "Internal Redirect Loop", I, H),
        ("Response Codes", "Internal Blocked by Robots.txt", W, H),
        ("Response Codes", "Internal Redirect Chain", W, M),
        ("Response Codes", "Internal Redirection (3XX)", W, L),
        (
            "Response Codes",
            "Internal Redirection (Meta Refresh)",
            W,
            L,
        ),
        ("Response Codes", "External No Response", W, L),
        ("Response Codes", "External Client Error (4XX)", W, L),
        ("Response Codes", "External Server Error (5XX)", W, L),
        ("Security", "HTTP URLs", I, H),
        ("URL", "Multiple Slashes", I, L),
        ("URL", "Contains A Space", I, L),
        ("URL", "Non ASCII Characters", W, L),
        ("URL", "Uppercase", W, L),
        ("URL", "Parameters", W, L),
        ("URL", "Underscores", O, L),
        ("URL", "Over {url.max_chars} Characters", O, L),
        ("Page Titles", "Missing", I, H),
        ("Page Titles", "Multiple", I, H),
        ("Page Titles", "Duplicate", O, M),
        ("Page Titles", "Over {title.max_chars} Characters", O, M),
        ("Page Titles", "Below {title.min_chars} Characters", O, M),
        ("Meta Description", "Multiple", I, M),
        ("Meta Description", "Missing", O, L),
        ("Meta Description", "Duplicate", O, L),
        (
            "Meta Description",
            "Over {meta_description.max_chars} Characters",
            O,
            L,
        ),
        (
            "Meta Description",
            "Below {meta_description.min_chars} Characters",
            O,
            L,
        ),
        ("H1", "Missing", I, M),
        ("H1", "Multiple", W, M),
        ("H1", "Duplicate", O, L),
        ("H1", "Over {h1.max_chars} Characters", O, L),
        ("H2", "Missing", W, L),
        ("H2", "Multiple", W, L),
        ("H2", "Duplicate", O, L),
        ("H2", "Over {h2.max_chars} Characters", O, L),
        ("Content", "Exact Duplicates", I, H),
        ("Content", "Low Content Pages", O, M),
        ("Images", "Missing Alt Text", I, L),
        ("Images", "Missing Alt Attribute", I, L),
        ("Images", "Over {image.max_kb} KB", O, M),
        (
            "Images",
            "Alt Text Over {image.alt_max_chars} Characters",
            O,
            L,
        ),
        ("Canonicals", "Multiple Conflicting", I, H),
        ("Canonicals", "Non-Indexable Canonical", I, H),
        ("Canonicals", "Canonicalised", W, H),
        ("Canonicals", "Missing", W, M),
        ("Canonicals", "Multiple", W, L),
        ("Directives", "Noindex", W, H),
        ("Directives", "Nofollow", W, H),
        ("Directives", "None", W, H),
        ("Hreflang", "Non-200 Hreflang URLs", I, H),
        ("Hreflang", "Missing Return Links", I, H),
        ("Hreflang", "Incorrect Language & Region Codes", I, H),
        ("Links", "Pages Without Internal Outlinks", W, H),
        ("Links", "Internal Nofollow Outlinks", W, L),
        ("Links", "Pages With High Crawl Depth", O, M),
        ("Links", "Internal Outlinks With No Anchor Text", O, L),
    ]
}

// spec: UI-064
#[test]
fn catalog_matches_the_benchmark_snapshot() {
    let bench = benchmark();
    for (tab, filter, ty, prio) in &bench {
        let e = catalog()
            .iter()
            .find(|e| e.tab == *tab && e.filter == *filter)
            .unwrap_or_else(|| panic!("Benchmark-Eintrag {tab}: {filter} fehlt im Katalog"));
        assert_eq!(e.issue_type, *ty, "Typ von {tab}: {filter}");
        assert_eq!(e.priority, *prio, "Priorität von {tab}: {filter}");
    }
    // Der Snapshot deckt alle Katalogeinträge der Kategorien Response Codes … Links ab
    // (Sitemaps ist ein P1-Teilumfang von UI-064 und hat keinen Benchmark in dieser Liste).
    let in_benchmark: BTreeSet<(&str, &str)> = bench.iter().map(|(t, f, _, _)| (*t, *f)).collect();
    for e in catalog().iter().filter(|e| {
        e.tab != "Sitemaps" && registry_issue_filters().contains(&(e.tab.clone(), e.filter.clone()))
    }) {
        assert!(
            in_benchmark.contains(&(e.tab.as_str(), e.filter.as_str())),
            "{}: {} fehlt im Benchmark-Snapshot",
            e.tab,
            e.filter
        );
    }
}

// spec: UI-064
#[test]
fn derivation_columns_are_present_for_every_entry() {
    for e in catalog() {
        assert!(
            !e.type_rule.is_empty() && !e.priority_rule.is_empty(),
            "{}",
            e.id
        );
    }
}

// spec: UI-064, ONPAGE-006
#[test]
fn end_to_end_samples() {
    let sample = |tab: &str, filter: &str| {
        let e = issues::entry_for(tab, filter).unwrap_or_else(|| panic!("{tab}: {filter}"));
        (e.issue_type, e.priority)
    };
    assert_eq!(
        sample("Response Codes", "Internal Client Error (4XX)"),
        (IssueType::Issue, IssuePriority::High)
    );
    assert_eq!(
        sample("Page Titles", "Duplicate"),
        (IssueType::Opportunity, IssuePriority::Medium)
    );
    assert_eq!(
        sample("Images", "Missing Alt Text"),
        (IssueType::Issue, IssuePriority::Low)
    );
    // Namensvergleich case-insensitiv (K-08) und mit aufgelösten Schwellen.
    assert!(issues::entry_for("page titles", "over 65 characters").is_some());
    assert!(issues::entry_for("Page Titles", "Over 65 Characters").is_some());
    assert!(issues::entry_for("Page Titles", "Over  Characters").is_none());
    assert!(issues::entry_for("Internal", "HTML").is_none());
}

fn count(tab: &str, filter: &str, n: Option<u64>, state: FilterState) -> FilterCount {
    FilterCount {
        tab: tab.to_string(),
        filter: filter.to_string(),
        label: filter.to_string(),
        count: n,
        pct: n.map(|n| n as f64),
        state,
        version: 1,
    }
}

// spec: UI-063, UI-064
#[test]
fn issues_from_counts_lists_only_ready_filters_with_hits_in_ui_order() {
    let counts = vec![
        count("Internal", "HTML", Some(40), FilterState::Ready),
        count("Page Titles", "Duplicate", Some(2), FilterState::Ready),
        count("Page Titles", "Missing", Some(0), FilterState::Ready),
        count(
            "Response Codes",
            "Internal Client Error (4XX)",
            Some(1),
            FilterState::Ready,
        ),
        count("Images", "Missing Alt Text", Some(5), FilterState::Ready),
        count(
            "Images",
            "Missing Alt Attribute",
            Some(5),
            FilterState::Ready,
        ),
        count(
            "Page Titles",
            "Over 65 Characters",
            Some(3),
            FilterState::Ready,
        ),
        count(
            "Sitemaps",
            "Orphan URLs",
            None,
            FilterState::NeedsCrawlAnalysis,
        ),
        count(
            "Content",
            "Exact Duplicates",
            None,
            FilterState::NeedsConfig("extraction.hash".into()),
        ),
        count("Canonicals", "Missing", Some(9), FilterState::Stale),
    ];
    let rows = issues_from_counts(&counts);
    let names: Vec<(String, String)> = rows
        .iter()
        .map(|r| (r.tab.clone(), r.filter.clone()))
        .collect();
    assert_eq!(
        names,
        vec![
            (
                "Response Codes".to_string(),
                "Internal Client Error (4XX)".to_string()
            ),
            ("Images".to_string(), "Missing Alt Attribute".to_string()),
            ("Images".to_string(), "Missing Alt Text".to_string()),
            ("Page Titles".to_string(), "Over 65 Characters".to_string()),
            ("Page Titles".to_string(), "Duplicate".to_string()),
        ]
    );
    assert_eq!(rows[0].issue_type, IssueType::Issue);
    assert_eq!(rows[0].priority, IssuePriority::High);
    assert_eq!(rows[0].count, 1);
    assert_eq!(rows[0].id, "response-codes-internal-client-error-4xx");
    assert_eq!(rows[3].issue_type, IssueType::Opportunity);
    assert_eq!(rows[3].count, 3);
    assert_eq!(rows[3].pct, Some(3.0));
    assert!(issues_from_counts(&[]).is_empty());
}

// spec: UI-063
#[test]
fn doc_keys_and_ids_are_unique() {
    let ids: BTreeSet<&str> = catalog().iter().map(|e| e.id.as_str()).collect();
    let docs: BTreeSet<&str> = catalog().iter().map(|e| e.doc_key.as_str()).collect();
    assert_eq!(ids.len(), catalog().len());
    assert_eq!(docs.len(), catalog().len());
}
