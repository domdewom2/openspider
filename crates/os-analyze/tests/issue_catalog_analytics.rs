//! UI-064 [P1]: Benchmark der Kategorie „Analytics“ (vier Befunde) mit Herleitung, Texten und Doku-Seiten.
use os_analyze::{issues, tabs, FilterKind};
use os_model::{IssuePriority, IssueType};

// spec: UI-064 [P1]
#[test]
fn analytics_category_matches_benchmark_classification_and_has_documented_texts() {
    let expected = [
        (
            "Orphan URLs",
            IssueType::Issue,
            IssuePriority::Medium,
            "T-I3",
            "P-M2",
        ),
        (
            "Bounce Rate Above 70%",
            IssueType::Warning,
            IssuePriority::Low,
            "T-W3",
            "P-L2",
        ),
        (
            "No GA Data",
            IssueType::Warning,
            IssuePriority::Low,
            "T-W3",
            "P-L2",
        ),
        (
            "Non-Indexable with GA Data",
            IssueType::Warning,
            IssuePriority::Low,
            "T-W3",
            "P-L2",
        ),
    ];
    let tab = tabs::analytics::tab();
    assert_eq!(
        issues::catalog()
            .iter()
            .filter(|e| e.tab == "Analytics")
            .count(),
        expected.len()
    );
    for (filter, kind, priority, type_rule, priority_rule) in expected {
        let entry = issues::entry_for("Analytics", filter).unwrap();
        assert_eq!(
            (entry.issue_type, entry.priority),
            (kind, priority),
            "{filter}"
        );
        assert_eq!(
            (entry.type_rule.as_str(), entry.priority_rule.as_str()),
            (type_rule, priority_rule)
        );
        assert!(entry.override_reason.is_none());
        let def = tab.filters.iter().find(|f| f.id == filter).unwrap();
        assert_eq!(def.kind, FilterKind::Live);
        assert!(!entry.needs_crawl_analysis);
        for lang in issues::Lang::ALL {
            let texts = issues::raw_texts(&entry.doc_key, *lang).unwrap();
            assert!(!texts.what.is_empty() && !texts.why.is_empty() && !texts.fix.is_empty());
        }
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/issues")
            .join(format!("{}.md", entry.doc_key));
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            issues::doc_page(entry)
        );
    }
    // „Sessions Above 0“ ist kein Befund.
    assert!(issues::entry_for("Analytics", "Sessions Above 0").is_none());
}
