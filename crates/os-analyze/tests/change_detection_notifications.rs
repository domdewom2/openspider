mod change_detection_fixture;
use change_detection_fixture as fixture;
use os_analyze::compare::change::{self, auto, notification};
use os_model::{CancelToken, ChangeElement, CompareConfig, CrawlConfig};

// spec: OUT-152
// spec: TECH-308
// spec: OUT-282
#[test]
fn notification_uses_saved_configuration_and_filters_empty_rows() {
    let dir = tempfile::tempdir().unwrap();
    let old = fixture::fixture(dir.path(), 1, 1, CrawlConfig::default());
    let new = fixture::fixture(dir.path(), 2, 2, CrawlConfig::default());
    let mut cfg = CompareConfig::default();
    cfg.change_detection.elements = vec![ChangeElement::PageTitles];
    let auto::AutoResult::Saved(path) = auto::run(
        new.path(),
        dir.path(),
        &dir.path().join("comparisons"),
        Some(&cfg),
        &CancelToken::new(),
    )
    .unwrap() else {
        panic!("comparison expected")
    };
    let (mut saved, stored) = auto::load(&path, &CancelToken::new()).unwrap();
    change::calculate(&saved, &stored.change_detection, &CancelToken::new()).unwrap();
    let all = notification::rows(&mut saved, &stored.change_detection, false).unwrap();
    assert!(all.iter().any(|r| r.change == 0));
    assert!(all
        .windows(2)
        .all(|w| w[0].change.unsigned_abs() >= w[1].change.unsigned_abs()));
    let filtered = notification::rows(&mut saved, &stored.change_detection, true).unwrap();
    assert!(filtered.iter().all(|r| r.change != 0));
    assert_eq!(filtered.len(), all.iter().filter(|r| r.change != 0).count());
    assert!(all
        .iter()
        .filter(|r| r.previous == 0)
        .all(|r| r.percent.is_none()));
    let mail = notification::content(&new, "<Task & C02>", true, false).unwrap();
    assert!(mail.html.contains("&lt;Task &amp; C02&gt;"));
    assert!(!mail.html.contains("<Task & C02>"));
    assert!(mail.text.contains("Page Titles\t0\t1\t1\t\n"));
    assert!(!mail.text.contains("Indexability\t0\t"));
    assert!(mail.text.contains("Missing\t1\t0\t-1\t-100.00\n"));
    assert!(mail.text.contains("Issue / Filter\tType\tPriority\tURLs"));
    assert!(mail.text.contains("Non-Indexable: 1"));
    // Kein neuer Vergleich und kein Crawl nötig für die Mail.
    drop(old);
    std::fs::write(path, "corrupt comparison").unwrap();
    assert!(notification::content(&new, "Task", true, false).is_err());
}

// spec: OUT-152
#[test]
fn notification_requires_a_completed_auto_compare_result() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture::fixture(dir.path(), 1, 1, CrawlConfig::default());
    assert!(notification::content(&file, "Task", true, false).is_err());
    assert!(notification::content(&file, "Task", false, false).is_ok());
    assert_eq!(
        auto::run(
            file.path(),
            dir.path(),
            &dir.path().join("comparisons"),
            None,
            &CancelToken::new()
        )
        .unwrap(),
        auto::AutoResult::NoPrevious
    );
    let mail = notification::content(&file, "Task", true, false).unwrap();
    assert!(mail.text.contains("kein vorheriger Crawl"));
    assert!(mail.html.contains("kein vorheriger Crawl"));
    assert!(!mail.text.contains("Previous\tCurrent"));
}
