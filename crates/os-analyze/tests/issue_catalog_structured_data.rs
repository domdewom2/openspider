use os_analyze::issues::{self, catalog, doc_page, Lang};
use os_model::{IssuePriority, IssueType};

// spec: UI-064 [P1]
#[test]
fn structured_data_category_matches_spec_benchmark_and_has_bilingual_documentation() {
    use IssuePriority::{High as H, Low as L};
    use IssueType::{Issue as I, Opportunity as O};
    // Benchmark-Erwartung UI-064, Kategorie Structured Data; „Missing“ heißt im Register „Missing Structured Data“
    // (Klärung K2-15).
    let mut expected = vec![
        ("Validation Errors", I, H),
        ("Rich Result Validation Errors", I, H),
        ("Parse Errors", I, H),
        ("Missing Structured Data", O, L),
        ("Validation Warnings", O, L),
        ("Rich Result Validation Warnings", O, L),
    ];
    let mut actual: Vec<_> = catalog()
        .iter()
        .filter(|e| e.tab == "Structured Data")
        .map(|e| (e.filter.as_str(), e.issue_type, e.priority))
        .collect();
    expected.sort_by_key(|e| e.0);
    actual.sort_by_key(|e| e.0);
    assert_eq!(actual, expected);
    for entry in catalog().iter().filter(|e| e.tab == "Structured Data") {
        assert!(!entry.needs_crawl_analysis, "{}", entry.filter);
        assert!(!entry.type_rule.is_empty() && !entry.priority_rule.is_empty());
        for lang in Lang::ALL {
            let t = issues::texts(&entry.doc_key, *lang, &[]).unwrap();
            assert!(!t.what.is_empty() && !t.why.is_empty() && !t.fix.is_empty());
        }
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/issues")
            .join(format!("{}.md", entry.doc_key));
        assert_eq!(std::fs::read_to_string(path).unwrap(), doc_page(entry));
    }
}

// spec: UI-064 [P1]
#[test]
fn validation_entries_follow_the_derivation_codes_for_validation_findings() {
    // Herleitung UI-064 (AMP, Structured Data, Validation): Validierungsfehler T-I2/P-H2, Warnungen T-O2/P-L1.
    for (filter, type_rule, priority_rule) in [
        ("Validation Errors", "T-I2", "P-H2"),
        ("Rich Result Validation Errors", "T-I2", "P-H2"),
        ("Validation Warnings", "T-O2", "P-L1"),
        ("Rich Result Validation Warnings", "T-O2", "P-L1"),
    ] {
        let e = catalog()
            .iter()
            .find(|e| e.tab == "Structured Data" && e.filter == filter)
            .unwrap();
        assert_eq!(
            (e.type_rule.as_str(), e.priority_rule.as_str()),
            (type_rule, priority_rule)
        );
    }
}
