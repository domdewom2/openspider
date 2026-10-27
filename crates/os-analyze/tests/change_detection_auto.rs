#[path = "change_detection_fixture.rs"]
mod fixture;
use os_analyze::compare::change::{
    self,
    auto::{self, AutoResult},
};
use os_model::{CancelToken, CrawlConfig};

// spec: OUT-152
// spec: TECH-308
// spec: CRAWL-036
#[test]
fn first_crawl_is_success_then_exactly_one_saved_comparison_and_matching_mail_counts() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("comparisons");
    let old = fixture::fixture(dir.path(), 1, 1, CrawlConfig::default());
    assert_eq!(
        auto::run(old.path(), dir.path(), &out, None, &CancelToken::new()).unwrap(),
        AutoResult::NoPrevious
    );
    let new = fixture::fixture(dir.path(), 2, 2, CrawlConfig::default());
    let AutoResult::Saved(path) =
        auto::run(new.path(), dir.path(), &out, None, &CancelToken::new()).unwrap()
    else {
        panic!("saved comparison")
    };
    let (c, cfg) = auto::load(&path, &CancelToken::new()).unwrap();
    let expected = change::counts(&c, &cfg.change_detection, false).unwrap();
    assert_eq!(auto::saved_counts(&path, false).unwrap(), expected);
    assert_eq!(
        auto::saved_counts(&path, true)
            .unwrap()
            .iter()
            .find(|r| r.element == "Page Titles")
            .unwrap()
            .change,
        1
    );
    assert_eq!(
        auto::run(new.path(), dir.path(), &out, None, &CancelToken::new()).unwrap(),
        AutoResult::Saved(path)
    );
    assert_eq!(
        std::fs::read_dir(out)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|e| e == "oscompare"))
            .count(),
        1
    );
    assert_eq!(
        new.get_meta("analysis_state").unwrap().as_deref(),
        Some("completed")
    );
}
// spec: OUT-152
#[test]
fn failed_stopped_limited_and_other_project_crawls_are_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("comparisons");
    let old = fixture::fixture(dir.path(), 1, 1, CrawlConfig::default());
    for (seed, state, reason, project) in [
        (3, "Paused", "queue_empty", "project"),
        (4, "Completed", "stopped", "project"),
        (5, "Completed", "limit_reached", "project"),
        (6, "Completed", "queue_empty", "other"),
    ] {
        let f = fixture::fixture(dir.path(), seed, 2, CrawlConfig::default());
        f.set_meta("state", state).unwrap();
        f.set_meta("end_reason", reason).unwrap();
        f.set_meta("project", project).unwrap();
        f.set_meta("started_at", "2026-01-20T00:00:00.000Z")
            .unwrap();
    }
    let new = fixture::fixture(dir.path(), 2, 2, CrawlConfig::default());
    let AutoResult::Saved(path) =
        auto::run(new.path(), dir.path(), &out, None, &CancelToken::new()).unwrap()
    else {
        panic!()
    };
    let (c, _) = auto::load(&path, &CancelToken::new()).unwrap();
    assert_eq!(c.previous().meta.crawl_id, old.meta().unwrap().crawl_id);
}
// spec: TECH-300
#[test]
fn saved_configuration_is_immutable_and_reclassification_creates_a_new_artifact() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("comparisons");
    let _old = fixture::fixture(dir.path(), 1, 1, CrawlConfig::default());
    let new = fixture::fixture(dir.path(), 2, 2, CrawlConfig::default());
    let AutoResult::Saved(first) =
        auto::run(new.path(), dir.path(), &out, None, &CancelToken::new()).unwrap()
    else {
        panic!()
    };
    let before = std::fs::read(&first).unwrap();
    let mut cfg = CrawlConfig::default().compare;
    cfg.change_detection.elements.clear();
    let AutoResult::Saved(second) = auto::run(
        new.path(),
        dir.path(),
        &out,
        Some(&cfg),
        &CancelToken::new(),
    )
    .unwrap() else {
        panic!()
    };
    assert_ne!(first, second);
    assert_eq!(std::fs::read(&first).unwrap(), before);
    assert!(auto::saved_counts(&second, false).unwrap().is_empty());
}

// spec: OUT-152
#[test]
fn configuration_mapping_is_used_for_auto_pairing() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("comparisons");
    let old = fixture::fixture(dir.path(), 1, 1, CrawlConfig::default());
    old.conn().execute_batch("UPDATE urls SET url=replace(url,'site.test','staging.test'),address=replace(address,'site.test','staging.test')").unwrap();
    let new = fixture::fixture(dir.path(), 2, 2, CrawlConfig::default());
    let mut cfg = CrawlConfig::default().compare;
    cfg.mapping.push(os_model::MappingRule {
        pattern: "staging\\.test".into(),
        replacement: "site.test".into(),
    });
    let AutoResult::Saved(path) = auto::run(
        new.path(),
        dir.path(),
        &out,
        Some(&cfg),
        &CancelToken::new(),
    )
    .unwrap() else {
        panic!()
    };
    let (c, stored) = auto::load(&path, &CancelToken::new()).unwrap();
    assert_eq!(stored, cfg);
    assert_eq!(change::ids(&c, "All", true).unwrap().len(), 11);
}
