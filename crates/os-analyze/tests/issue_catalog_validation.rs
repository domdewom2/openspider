use os_analyze::issues::{self, catalog, doc_page, Lang};
use os_model::{IssuePriority, IssueType, Value};

// spec: UI-064 [P1]
#[test]
fn validation_category_matches_spec_benchmark_and_has_bilingual_documentation() {
    let entries: Vec<_> = catalog().iter().filter(|e| e.tab == "Validation").collect();
    assert_eq!(entries.len(), 9);
    for entry in entries {
        let warning = [
            "Invalid HTML Elements In <head>",
            "<head> Not First In <html> Element",
            "<body> Element Preceding <html>",
        ]
        .contains(&entry.filter.as_str());
        assert_eq!(
            entry.issue_type,
            if warning {
                IssueType::Warning
            } else {
                IssueType::Issue
            },
            "{}",
            entry.filter
        );
        assert_eq!(entry.priority, IssuePriority::High);
        assert!(!entry.needs_crawl_analysis);
        for lang in Lang::ALL {
            let t = issues::texts(&entry.doc_key, *lang, &[]).unwrap();
            assert!(!t.what.is_empty() && !t.why.is_empty() && !t.fix.is_empty());
        }
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/issues")
            .join(format!("{}.md", entry.doc_key));
        let expected = doc_page(entry);
        if std::env::var_os("UPDATE_GOLDEN").is_some() {
            std::fs::write(&path, &expected).unwrap();
        }
        assert_eq!(std::fs::read_to_string(path).unwrap(), expected);
    }
}

// spec: CRAWL-140 [P1]
#[test]
fn size_issue_names_use_mib_and_current_thresholds() {
    for (id, path) in [
        (
            "validation-html-document-size",
            "validation.html_doc_max_bytes",
        ),
        ("validation-resource-size", "validation.resource_max_bytes"),
    ] {
        let entry = catalog().iter().find(|e| e.id == id).unwrap();
        let params = [(path, Value::Int(3 * 1_048_576))];
        assert!(issues::filter_label(entry, &params).contains("3MB"));
        for lang in Lang::ALL {
            let t = issues::texts(id, *lang, &params).unwrap();
            assert!(!t.what.contains("{max}"));
            assert!(!t.fix.contains("{max}"));
        }
    }
}
