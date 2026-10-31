#[path = "framework_common.rs"]
mod common;
use common::*;
use os_analyze::views::{query_view, ViewQuery};
use os_model::CrawlConfig;
// spec: ONPAGE-023
#[test]
fn class_filters_partition_both_modes_and_plugins_is_public_name() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = create(dir.path(), "types.ospider", 3);
    for (n, class) in ["Plugins", "Other", "Media", "Fonts", "XML", "Unknown"]
        .iter()
        .enumerate()
    {
        insert(
            &file,
            &[row(n as i64 + 1, &format!("{n}")).text("content_class", class)],
        );
    }
    let mut cfg = CrawlConfig::default();
    for parity in [true, false] {
        cfg.ui.parity_filters = parity;
        let tabs = os_analyze::tabs::configured(&cfg);
        let tab = tabs.iter().find(|t| t.id == "Internal").unwrap();
        assert!(tab.filters.iter().any(|f| f.id == "Media"));
        let count = |filter| {
            query_view(&file, &cfg, &ViewQuery::new("Internal", filter))
                .unwrap()
                .total
        };
        assert_eq!(count("Plugins"), 1);
        assert_eq!(count("Other"), 1);
        let sum: u64 = tab
            .filters
            .iter()
            .filter(|f| f.id != "All")
            .map(|f| count(f.id))
            .sum();
        assert_eq!(sum, count("All"));
    }
}

// spec: ONPAGE-011
// spec: TECH-022
#[test]
fn internal_canonical_display_strips_fragment_without_changing_stored_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = create(dir.path(), "fragment.ospider", 96);
    insert(&file, &[row(1, "https://site.test/source")]);
    file.conn().execute_batch("INSERT INTO canonicals (url_id,source,position,raw,resolved,in_head) VALUES (1,'html',0,'/target#part','https://site.test/target#part',1)").unwrap();
    let mut query = ViewQuery::new("Internal", "All");
    query.columns = os_analyze::views::ColumnSelection::All;
    let data = query_view(&file, &CrawlConfig::default(), &query).unwrap();
    let column = data
        .columns
        .iter()
        .position(|name| name == "Canonical Link Element 1")
        .unwrap();
    assert_eq!(
        data.rows[0][column],
        os_model::Value::Text("https://site.test/target".into())
    );
    let stored: String = file
        .conn()
        .query_row("SELECT resolved FROM canonicals", [], |row| row.get(0))
        .unwrap();
    assert_eq!(stored, "https://site.test/target#part");
}
