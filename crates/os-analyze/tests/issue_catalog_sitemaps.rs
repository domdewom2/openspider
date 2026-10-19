//! UI-064: sechs Sitemap-Befunde, Fachspec-Namen gemäß K2-15.
use os_analyze::{issues, tabs, FilterKind};
use os_model::{IssuePriority, IssueType};

// spec: UI-064 [P1]
#[test]
fn sitemap_category_matches_benchmark_classification_and_has_documented_texts() {
    let expected = [
        (
            "URLs Not In Sitemap",
            IssueType::Issue,
            IssuePriority::Medium,
            true,
        ),
        ("Orphan URLs", IssueType::Issue, IssuePriority::Medium, true),
        (
            "Non-Indexable URLs In Sitemap",
            IssueType::Issue,
            IssuePriority::Medium,
            true,
        ),
        (
            "URLs In Multiple Sitemaps",
            IssueType::Warning,
            IssuePriority::Low,
            true,
        ),
        (
            "XML Sitemap With Over 50k URLs",
            IssueType::Issue,
            IssuePriority::High,
            false,
        ),
        (
            "XML Sitemap With Over 50MB",
            IssueType::Issue,
            IssuePriority::High,
            false,
        ),
    ];
    let tab = tabs::sitemaps::tab();
    assert_eq!(
        issues::catalog()
            .iter()
            .filter(|e| e.tab == "Sitemaps")
            .count(),
        expected.len()
    );
    for (filter, kind, priority, post) in expected {
        let entry = issues::entry_for("Sitemaps", filter).unwrap();
        assert_eq!(
            (entry.issue_type, entry.priority, entry.needs_crawl_analysis),
            (kind, priority, post)
        );
        assert!(!entry.type_rule.is_empty() && !entry.priority_rule.is_empty());
        let def = tab.filters.iter().find(|f| f.id == filter).unwrap();
        assert_eq!(matches!(def.kind, FilterKind::PostCrawl(_)), post);
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
}
