//! Klassifizierungs-Benchmark der sechs Mobile-Befunde aus der Spec.
use os_analyze::{issues, tabs, FilterKind};
use os_model::{IssuePriority, IssueType};

// spec: UI-064 [P1]
#[test]
fn mobile_category_matches_spec_benchmark_and_has_own_texts() {
    let expected = [
        ("Viewport Not Set", IssueType::Issue, IssuePriority::High),
        ("Target Size", IssueType::Issue, IssuePriority::Medium),
        (
            "Content Not Sized Correctly",
            IssueType::Issue,
            IssuePriority::High,
        ),
        ("Illegible Font Size", IssueType::Issue, IssuePriority::High),
        ("Unsupported Plugins", IssueType::Issue, IssuePriority::High),
        (
            "Mobile Alternate Link",
            IssueType::Warning,
            IssuePriority::High,
        ),
    ];
    let tab = tabs::mobile::tab();
    assert_eq!(
        issues::catalog()
            .iter()
            .filter(|e| e.tab == "Mobile")
            .count(),
        6
    );
    for (name, typ, priority) in expected {
        let entry = issues::entry_for("Mobile", name).unwrap();
        assert_eq!((entry.issue_type, entry.priority), (typ, priority));
        assert!(!entry.needs_crawl_analysis);
        assert_eq!(
            tab.filters.iter().find(|f| f.id == name).unwrap().kind,
            FilterKind::Live
        );
        for lang in issues::Lang::ALL {
            let text = issues::raw_texts(&entry.doc_key, *lang).unwrap();
            assert!(!text.what.is_empty() && !text.why.is_empty() && !text.fix.is_empty());
        }
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/issues")
            .join(format!("{}.md", entry.doc_key));
        if std::env::var("UPDATE_MOBILE_DOCS").as_deref() == Ok("1") {
            std::fs::write(&path, issues::doc_page(entry)).unwrap();
        }
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            issues::doc_page(entry)
        );
    }
}
