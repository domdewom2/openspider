use os_analyze::issues::{self, catalog, doc_page, Lang};
use os_model::{IssuePriority, IssueType};

/// Benchmark-Erwartung aus UI-064, Kategorie Search Console (Filternamen nach INTEG-116/125, K2-15).
fn benchmark() -> Vec<(&'static str, IssueType, IssuePriority)> {
    use IssuePriority::{High as H, Low as L, Medium as M};
    use IssueType::{Issue as I, Warning as W};
    vec![
        ("Page Is Not Mobile Friendly", I, H),
        ("Orphan URLs", I, M),
        ("URL is on Google, But Has Issues", I, M),
        ("AMP URL Is Invalid", I, M),
        ("Indexable URL Not Indexed", W, H),
        ("User-Declared Canonical Not Selected", W, M),
        ("No Search Analytics Data", W, L),
        ("Non-Indexable with Search Analytics Data", W, L),
        ("URL Is Not on Google", W, L),
    ]
}

// spec: UI-064 [P1]
#[test]
fn search_console_category_matches_the_benchmark_and_has_bilingual_documentation() {
    let entries: Vec<_> = catalog()
        .iter()
        .filter(|e| e.tab == "Search Console")
        .collect();
    let expected = benchmark();
    assert_eq!(entries.len(), expected.len());
    for (filter, issue_type, priority) in expected {
        let entry = entries
            .iter()
            .find(|e| e.filter.eq_ignore_ascii_case(filter))
            .unwrap_or_else(|| panic!("{filter} fehlt im Katalog"));
        assert_eq!(entry.issue_type, issue_type, "{filter}");
        assert_eq!(entry.priority, priority, "{filter}");
        // Herleitung nach UI-063 ohne Ausnahme: fremdgemeldete Fehler T-I3, Fremddaten T-W3.
        assert!(entry.override_reason.is_none(), "{filter}");
        assert!(
            ["T-I3", "T-W3"].contains(&entry.type_rule.as_str()),
            "{filter}"
        );
        assert!(!entry.needs_crawl_analysis, "{filter}");
        for lang in Lang::ALL {
            let t = issues::texts(&entry.doc_key, *lang, &[]).unwrap();
            assert!(!t.what.is_empty() && !t.why.is_empty() && !t.fix.is_empty());
        }
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/issues")
            .join(format!("{}.md", entry.doc_key));
        let page = doc_page(entry);
        if std::env::var_os("UPDATE_GOLDEN").is_some() {
            std::fs::write(&path, &page).unwrap();
        }
        assert_eq!(std::fs::read_to_string(path).unwrap(), page);
    }
}

// spec: UI-064 [P1]
#[test]
fn every_catalog_entry_points_to_a_live_search_console_filter() {
    let tab = os_analyze::tabs::all()
        .into_iter()
        .find(|t| t.id == "Search Console")
        .unwrap();
    assert!(!tab.is_stub());
    for entry in catalog().iter().filter(|e| e.tab == "Search Console") {
        let filter = tab
            .filters
            .iter()
            .find(|f| f.id.eq_ignore_ascii_case(&entry.filter))
            .unwrap_or_else(|| panic!("{}", entry.filter));
        assert!(!filter.is_stub(), "{}", entry.filter);
    }
}
