use os_analyze::{
    issues::{self, Lang},
    tabs::accessibility,
};
use os_model::{IssuePriority, IssueType};
// spec: UI-064 [P1]
// spec: TECH-252
// spec: TECH-253
// spec: TECH-254
// spec: TECH-255
// spec: TECH-256
// spec: TECH-257
#[test]
fn all_93_rules_have_spec_priorities_and_bilingual_texts() {
    let entries: Vec<_> = issues::catalog()
        .iter()
        .filter(|e| e.tab == "Accessibility")
        .collect();
    assert_eq!(entries.len(), 93);
    let snapshot: Vec<_> = include_str!("../../../tests/fixtures/ui/fx-ui-a11y/spec-rules.tsv")
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| line.split('\t').collect::<Vec<_>>())
        .collect();
    assert_eq!(snapshot.len(), 93);
    assert_eq!(accessibility::rules().len(), snapshot.len());
    let mut ids = std::collections::BTreeSet::new();
    for expected in snapshot {
        assert!(ids.insert(expected[0]));
        let r = accessibility::rule(expected[0]).unwrap();
        assert_eq!(r.group, expected[1], "{} group", r.id);
        assert_eq!(expected[2], "Warning");
        assert_eq!(r.priority, expected[3], "{} priority", r.id);
        let e = entries.iter().find(|e| e.filter == r.id).unwrap();
        assert_eq!(e.issue_type, IssueType::Warning);
        assert_eq!(
            e.priority,
            match expected[3] {
                "High" => IssuePriority::High,
                "Medium" => IssuePriority::Medium,
                "Low" => IssuePriority::Low,
                other => panic!("unknown priority {other}"),
            }
        );
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/issues")
            .join(format!("{}.md", e.doc_key));
        let expected = issues::doc_page(e);
        if std::env::var_os("UPDATE_GOLDEN").is_some() {
            std::fs::write(&path, &expected).unwrap();
        }
        assert_eq!(std::fs::read_to_string(path).unwrap(), expected);
        for lang in Lang::ALL {
            let t = issues::texts(&e.doc_key, *lang, &[]).unwrap();
            assert!(!t.what.is_empty() && !t.why.is_empty() && !t.fix.is_empty());
        }
    }
    for (group, expected) in accessibility::GROUPS.into_iter().zip([
        [8, 13, 7],
        [52, 2, 2],
        [3, 0, 0],
        [1, 0, 2],
        [2, 0, 0],
        [1, 0, 0],
    ]) {
        let counts = ["High", "Medium", "Low"].map(|p| {
            accessibility::rules()
                .iter()
                .filter(|r| r.group == group && r.priority == p)
                .count()
        });
        assert_eq!(counts, expected);
    }
}
