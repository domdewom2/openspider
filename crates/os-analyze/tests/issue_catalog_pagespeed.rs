//! UI-064 [P1]: PageSpeed-Befunde (19 Opportunities) gegen den Benchmark, mit Texten und Doku-Seiten.
use os_analyze::{issues, tabs, views::FilterCount, FilterKind};
use os_model::{FilterState, IssuePriority, IssueType};

fn benchmark() -> Vec<(&'static str, IssuePriority)> {
    use IssuePriority::{Low, Medium};
    vec![
        ("Document Request Latency", Medium),
        ("LCP Request Discovery", Medium),
        ("Render Blocking Requests", Medium),
        ("Network Dependency Tree", Medium),
        ("Use Efficient Cache Lifetimes", Medium),
        ("Layout Shift Culprits", Medium),
        ("Improve Image Delivery", Medium),
        ("Forced Reflow", Medium),
        ("Legacy JavaScript", Medium),
        ("Duplicated JavaScript", Medium),
        ("Avoid Enormous Network Payloads", Medium),
        ("Minify CSS", Medium),
        ("Minify JavaScript", Medium),
        ("Reduce Unused CSS", Medium),
        ("Reduce Unused JavaScript", Medium),
        ("Reduce JavaScript Execution Time", Medium),
        ("Minimize Main-Thread Work", Medium),
        ("Optimize DOM Size", Low),
        ("Font Display", Low),
    ]
}

// spec: UI-064 [P1]
#[test]
fn pagespeed_category_matches_benchmark_classification_and_has_documented_texts() {
    let expected = benchmark();
    let tab = tabs::pagespeed::tab();
    assert_eq!(
        issues::catalog()
            .iter()
            .filter(|e| e.tab == "PageSpeed")
            .count(),
        expected.len()
    );
    for (filter, priority) in expected {
        let entry = issues::entry_for("PageSpeed", filter).unwrap();
        assert_eq!(
            (entry.issue_type, entry.priority, entry.needs_crawl_analysis),
            (IssueType::Opportunity, priority, false),
            "{filter}"
        );
        assert_eq!(entry.type_rule, "T-O2");
        assert!(entry.override_reason.is_none(), "{filter}");
        let def = tab.filters.iter().find(|f| f.id == filter).unwrap();
        assert_eq!(def.kind, FilterKind::Live);
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

fn count(filter: &str, n: Option<u64>, state: FilterState) -> FilterCount {
    FilterCount {
        tab: "PageSpeed".into(),
        filter: filter.into(),
        label: filter.into(),
        count: n,
        pct: None,
        state,
        version: 1,
    }
}

// spec: UI-064 [P1]
#[test]
fn without_connected_source_pagespeed_creates_no_issues() {
    let off = vec![
        count(
            "Minify CSS",
            None,
            FilterState::NeedsConfig("api.psi.enabled".into()),
        ),
        count(
            "Font Display",
            None,
            FilterState::NeedsConfig("api.psi.enabled".into()),
        ),
    ];
    assert!(issues::issues_from_counts(&off).is_empty());
    let on = vec![
        count("Minify CSS", Some(3), FilterState::Ready),
        count("Font Display", Some(1), FilterState::Ready),
    ];
    let rows = issues::issues_from_counts(&on);
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|r| r.issue_type == IssueType::Opportunity));
    assert_eq!(rows[0].id, "pagespeed-minify-css");
    assert_eq!(rows[0].priority, IssuePriority::Medium);
}
