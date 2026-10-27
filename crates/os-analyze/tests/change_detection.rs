//! Synthetische SQLite-Fixture: zwei Site-Versionen mit denselben URLs und unterschiedlichen Messwerten.
use os_analyze::compare::{change, Side};
use os_analyze::views::{self, ViewQuery};
use os_model::{CancelToken, ChangeDetectionConfig, ChangeElement, CrawlConfig};

#[path = "change_detection_fixture.rs"]
mod fixture;
use fixture::{all, comparison, fixture};

// spec: TECH-300
// spec: OUT-144
#[test]
fn registry_configuration_and_disabled_elements() {
    let defaults = ChangeDetectionConfig::default();
    assert_eq!(change::ELEMENTS.len(), 14);
    assert_eq!(defaults.elements.len(), 13);
    assert_eq!(defaults.content_threshold_pct, 10);
    assert_eq!(
        change::ELEMENTS
            .iter()
            .map(|(e, _, _)| *e)
            .collect::<Vec<_>>(),
        ChangeElement::ALL
    );
    for (e, _, _) in change::ELEMENTS {
        let mut cfg = all();
        cfg.elements.retain(|x| *x != e);
        let tab = change::tab(&cfg, true);
        assert_eq!(tab.filters.len(), 14);
        assert!(!tab.filters.iter().any(|f| change::ELEMENTS
            .iter()
            .any(|(known, label, _)| *known == e && *label == f.id)));
    }
    let cfg = CrawlConfig::default();
    assert_eq!(
        CrawlConfig::from_toml_str(&cfg.to_toml_string().unwrap())
            .unwrap()
            .compare,
        cfg.compare
    );
}
// spec: TECH-301
#[test]
fn population_requires_both_answered_internal_html_pages() {
    let dir = tempfile::tempdir().unwrap();
    let c = comparison(
        fixture(dir.path(), 1, 1, CrawlConfig::default()),
        fixture(dir.path(), 2, 2, CrawlConfig::default()),
    );
    change::calculate(&c, &all(), &CancelToken::new()).unwrap();
    assert_eq!(change::ids(&c, "All", true).unwrap().len(), 11);
    let tab = change::tab(&all(), true);
    let page = views::query_view_in(
        &[tab],
        &c.current().file,
        &c.current().config,
        &ViewQuery::new("Change Detection", "All"),
    )
    .unwrap();
    assert_eq!(page.total, 11);
    assert!(page.columns.contains(&"Title 1 (Previous)".into()));
    assert!(page.columns.contains(&"Word Count Change %".into()));
}
// spec: TECH-302
#[test]
fn first_text_normalizes_whitespace_but_preserves_case_and_missing_values() {
    let dir = tempfile::tempdir().unwrap();
    let c = comparison(
        fixture(dir.path(), 1, 1, CrawlConfig::default()),
        fixture(dir.path(), 2, 2, CrawlConfig::default()),
    );
    change::calculate(&c, &all(), &CancelToken::new()).unwrap();
    for (filter, expected) in [
        ("Page Titles", vec![1]),
        ("H1", vec![3]),
        ("Meta Description", vec![4]),
    ] {
        assert_eq!(
            change::ids(&c, filter, true)
                .unwrap()
                .iter()
                .collect::<Vec<_>>(),
            expected
        );
    }
}
// spec: TECH-303
#[test]
fn numbers_use_writer_counters_and_compare_type_sets() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.extraction.structured_data.json_ld = true;
    let c = comparison(
        fixture(dir.path(), 1, 1, cfg.clone()),
        fixture(dir.path(), 2, 2, cfg),
    );
    change::calculate(&c, &all(), &CancelToken::new()).unwrap();
    for (filter, expected) in [
        ("Word Count", vec![5]),
        ("Crawl Depth", vec![6]),
        ("Inlinks", vec![7]),
        ("Unique Inlinks", vec![]),
        ("Structured Data Unique Types", vec![8]),
    ] {
        assert_eq!(
            change::ids(&c, filter, true)
                .unwrap()
                .iter()
                .collect::<Vec<_>>(),
            expected
        );
    }
    let mut q = ViewQuery::new("Change Detection", "Structured Data Unique Types");
    q.columns = views::ColumnSelection::Named(vec![
        "Unique Type Names (Current)".into(),
        "Unique Type Names (Previous)".into(),
        "Unique Types Change".into(),
    ]);
    let p = views::query_view_in(
        &[change::tab(&all(), true)],
        &c.current().file,
        &c.current().config,
        &q,
    )
    .unwrap();
    assert_eq!(
        p.rows[0],
        vec!["Article".into(), "Product".into(), 0i64.into()]
    );
}
// spec: TECH-304
#[test]
fn indexability_compares_status_and_reason_sets() {
    let dir = tempfile::tempdir().unwrap();
    let c = comparison(
        fixture(dir.path(), 1, 1, CrawlConfig::default()),
        fixture(dir.path(), 2, 2, CrawlConfig::default()),
    );
    change::calculate(&c, &all(), &CancelToken::new()).unwrap();
    assert_eq!(
        change::ids(&c, "Indexability", true)
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        vec![9]
    );
}
// spec: TECH-305
// spec: OUT-145
#[test]
fn minhash_rounded_boundary_and_golden_are_deterministic() {
    assert_eq!(change::content_change_pct(0.90), 10.0);
    assert_eq!(change::content_change_pct(0.8999), 10.01);
    assert!(change::content_change_pct(0.90) <= 10.0);
    assert!(change::content_change_pct(0.8999) > 10.0);
    let dir = tempfile::tempdir().unwrap();
    let c = comparison(
        fixture(dir.path(), 1, 1, CrawlConfig::default()),
        fixture(dir.path(), 2, 2, CrawlConfig::default()),
    );
    change::calculate(&c, &all(), &CancelToken::new()).unwrap();
    assert_eq!(
        change::ids(&c, "Content", true)
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        vec![11]
    );
    let mut output = "case,change_pct\n".to_string();
    for (name, a, b) in [
        ("identical", "one two three four", "ONE  two three four"),
        ("different", "one two three four", "five six seven eight"),
        ("empty", "", ""),
    ] {
        let sig = |s| {
            os_analyze::minhash::signature(&os_analyze::minhash::shingles(s))
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect::<Vec<_>>()
        };
        output.push_str(&format!(
            "{name},{:.2}\n",
            change::content_change_pct(change::signature_similarity(&sig(a), &sig(b)).unwrap())
        ));
    }
    assert_eq!(
        output,
        include_str!("../src/compare/change/content-change.golden.csv")
    );
    let mut cfg = all();
    cfg.content_threshold_pct = 100;
    change::calculate(&c, &cfg, &CancelToken::new()).unwrap();
    assert!(change::ids(&c, "Content", true).unwrap().is_empty());
}
// spec: TECH-307
#[test]
fn not_changed_is_exact_complement_and_swap_reverses_deltas() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = comparison(
        fixture(dir.path(), 1, 1, CrawlConfig::default()),
        fixture(dir.path(), 2, 2, CrawlConfig::default()),
    );
    change::calculate(&c, &all(), &CancelToken::new()).unwrap();
    let population = change::ids(&c, "All", true).unwrap();
    for (_, filter, _) in change::ELEMENTS {
        let yes = change::ids(&c, filter, true).unwrap();
        let no = change::ids(&c, filter, false).unwrap();
        assert_eq!(&yes | &no, population);
        assert!((&yes & &no).is_empty());
    }
    c.swap();
    change::calculate(&c, &all(), &CancelToken::new()).unwrap();
    let mut q = ViewQuery::new("Change Detection", "Word Count");
    q.columns = views::ColumnSelection::Named(vec!["Word Count Change".into()]);
    let p = views::query_view_in(
        &[change::tab(&all(), true)],
        &c.current().file,
        &c.current().config,
        &q,
    )
    .unwrap();
    assert_eq!(p.rows[0][0], (-30i64).into());
}
// spec: TECH-300
#[test]
fn missing_measurements_are_empty_and_cancel_is_atomic() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.extraction.structured_data.json_ld = false;
    cfg.extraction.structured_data.microdata = false;
    cfg.extraction.structured_data.rdfa = false;
    let a = fixture(dir.path(), 1, 1, cfg);
    a.conn()
        .execute_batch("DELETE FROM url_ext WHERE key='content.text'")
        .unwrap();
    let c = comparison(a, fixture(dir.path(), 2, 2, CrawlConfig::default()));
    assert!(!change::content_available(&c, Side::Previous).unwrap());
    change::calculate(&c, &all(), &CancelToken::new()).unwrap();
    assert!(change::ids(&c, "Content", true).unwrap().is_empty());
    assert!(change::ids(&c, "Structured Data Unique Types", true)
        .unwrap()
        .is_empty());
    let before = change::ids(&c, "All", true).unwrap();
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(change::calculate(&c, &all(), &cancel).is_err());
    assert_eq!(change::ids(&c, "All", true).unwrap(), before);
}
