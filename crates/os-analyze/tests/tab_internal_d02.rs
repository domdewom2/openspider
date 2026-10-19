#[path = "framework_common.rs"]
mod common;
use common::*;
use os_analyze::views::{query_view, ViewQuery};
use os_model::CrawlConfig;
// spec: ONPAGE-023
#[test]
fn class_filters_partition_both_modes_and_flash_is_public_name() {
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
        assert_eq!(tab.filters.iter().any(|f| f.id == "Media"), !parity);
        let count = |filter| {
            query_view(&file, &cfg, &ViewQuery::new("Internal", filter))
                .unwrap()
                .total
        };
        assert_eq!(count("Flash"), 1);
        assert_eq!(count("Other"), if parity { 4 } else { 1 });
        let sum: u64 = tab
            .filters
            .iter()
            .filter(|f| f.id != "All")
            .map(|f| count(f.id))
            .sum();
        assert_eq!(sum, count("All"));
    }
}
