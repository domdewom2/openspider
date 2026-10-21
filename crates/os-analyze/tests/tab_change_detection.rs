#[path = "change_detection_fixture.rs"]
mod fixture;
use os_analyze::{
    compare::change,
    defs::SortKey,
    views::{self, ColumnSelection, ViewQuery},
};
use os_model::{CancelToken, CrawlConfig, Value};

// spec: TECH-301
// spec: TECH-307
#[test]
fn percentage_zero_denominator_is_empty_sorted_as_infinity_and_supports_absolute_change() {
    let dir = tempfile::tempdir().unwrap();
    let old = fixture::fixture(dir.path(), 1, 1, CrawlConfig::default());
    let new = fixture::fixture(dir.path(), 2, 2, CrawlConfig::default());
    old.conn().execute_batch("UPDATE urls SET word_count=0 WHERE id=1;UPDATE urls SET word_count=100 WHERE id IN(2,3)").unwrap();
    new.conn().execute_batch("UPDATE urls SET word_count=10 WHERE id=1;UPDATE urls SET word_count=200 WHERE id=2;UPDATE urls SET word_count=50 WHERE id=3").unwrap();
    let c = fixture::comparison(new, old);
    let cfg = fixture::all();
    change::calculate(&c, &cfg, &CancelToken::new()).unwrap();
    let mut q = ViewQuery::new("Change Detection", "Word Count");
    q.columns = ColumnSelection::Named(vec!["Word Count Change %".into()]);
    q.sort = vec![SortKey {
        column: "Word Count Change %".into(),
        descending: true,
    }];
    let tab = change::tab_for_query(&cfg, true, &mut q);
    let page = views::query_view_in(&[tab], &c.current().file, &c.current().config, &q).unwrap();
    assert_eq!(page.row_keys[0].0, 1);
    assert_eq!(page.rows[0][0], Value::Null);
    assert_eq!(page.rows[1][0], Value::Float(100.0));
    q.sort = vec![SortKey {
        column: "Word Count Change".into(),
        descending: true,
    }];
    let tab = change::tab_for_query(&cfg, true, &mut q);
    let page = views::query_view_in(&[tab], &c.current().file, &c.current().config, &q).unwrap();
    assert_eq!(page.row_keys[0].0, 2);
}
// spec: TECH-303
#[test]
fn all_outlink_counts_and_structured_type_reordering_are_compared_independently() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.extraction.structured_data.json_ld = true;
    let old = fixture::fixture(dir.path(), 1, 1, cfg.clone());
    let new = fixture::fixture(dir.path(), 2, 2, cfg);
    old.conn()
        .execute_batch(
            "UPDATE sd_items SET json='{\"@type\":[\"Product\",\"Article\"]}' WHERE url_id=1",
        )
        .unwrap();
    new.conn().execute_batch("UPDATE sd_items SET json='{\"@type\":[\"Article\",\"Product\",\"Product\"]}' WHERE url_id=1;UPDATE url_counters SET outlinks=2,unique_outlinks=1,external_outlinks=3,unique_external_outlinks=1 WHERE url_id=1").unwrap();
    let c = fixture::comparison(new, old);
    change::calculate(&c, &fixture::all(), &CancelToken::new()).unwrap();
    assert!(!change::ids(&c, "Structured Data Unique Types", true)
        .unwrap()
        .contains(1));
    for filter in [
        "Internal Outlinks",
        "Unique Internal Outlinks",
        "External Outlinks",
        "Unique External Outlinks",
    ] {
        assert_eq!(
            change::ids(&c, filter, true)
                .unwrap()
                .iter()
                .collect::<Vec<_>>(),
            vec![1]
        );
    }
}
// spec: TECH-304
#[test]
fn non_indexable_reason_change_is_detected_even_without_status_change() {
    let dir = tempfile::tempdir().unwrap();
    let old = fixture::fixture(dir.path(), 1, 1, CrawlConfig::default());
    let new = fixture::fixture(dir.path(), 2, 2, CrawlConfig::default());
    old.conn()
        .execute_batch(
            "UPDATE urls SET indexability='Non-Indexable',indexability_status='noindex' WHERE id=1",
        )
        .unwrap();
    new.conn().execute_batch("UPDATE urls SET indexability='Non-Indexable',indexability_status='Canonicalised' WHERE id=1").unwrap();
    let c = fixture::comparison(new, old);
    change::calculate(&c, &fixture::all(), &CancelToken::new()).unwrap();
    assert!(change::ids(&c, "Indexability", true).unwrap().contains(1));
}

// spec: TECH-300
#[test]
fn ordinary_crawl_overview_has_no_change_detection_section() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture::fixture(dir.path(), 1, 1, CrawlConfig::default());
    let counts = views::filter_counts(&file, &file.config().unwrap()).unwrap();
    assert!(!counts.iter().any(|count| count.tab == "Change Detection"));
}
