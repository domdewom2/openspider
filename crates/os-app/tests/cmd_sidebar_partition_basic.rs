//! UI-102: the partition the core announces with `sidebar.overview` is disjoint on real crawl data.
#[path = "cmd_catalog_fixture.rs"]
mod fixture;
use fixture::*;
use serde_json::{json, Value};
use std::collections::BTreeSet;

/// URL ids of one tab/filter view (the store fixture has far fewer than 500 rows).
async fn ids(fx: &Fx, tab: &str, filter: &str) -> BTreeSet<u64> {
    let spec = json!({"kind":"tab","tab":tab,"filter":filter,"search":null,"sort":[],"segment":null,"compare":null});
    let h = fx.open(spec, json!(["Address"])).await;
    let total = h["total"].as_u64().unwrap();
    assert!(total <= 500, "{tab}/{filter}: {total} rows");
    let rows = fx.rows(&h, 0, 500, json!(["Address"])).await;
    let keys: BTreeSet<u64> = rows["rowKeys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k.as_u64().unwrap())
        .collect();
    assert_eq!(keys.len() as u64, total, "{tab}/{filter}");
    keys
}

fn count(tab: &Value, filter: &str) -> u64 {
    tab["filters"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["id"] == filter)
        .unwrap_or_else(|| panic!("{} has no filter {filter}", tab["tab"]))["count"]
        .as_u64()
        .unwrap()
}

/// Checks every announced partition: categories pairwise disjoint and inside the base (by URL ids), category
/// counts of the overview equal to the view totals, sum of the categories ≤ base. Returns (tab, sum, base).
async fn check_partitions(fx: &Fx) -> Vec<(String, u64, u64)> {
    let overview = fx.call("sidebar.overview", json!({})).await;
    let mut out = Vec::new();
    for tab in overview["tabs"].as_array().unwrap() {
        let Some(partition) = tab.get("partition").filter(|p| !p.is_null()) else {
            continue;
        };
        let name = tab["tab"].as_str().unwrap();
        let base_filter = partition["base"].as_str().unwrap();
        let base = ids(fx, name, base_filter).await;
        assert_eq!(count(tab, base_filter), base.len() as u64, "{name}");
        let mut seen = BTreeSet::new();
        let mut sum = 0;
        for filter in partition["filters"].as_array().unwrap() {
            let filter = filter.as_str().unwrap();
            assert_ne!(filter, base_filter);
            let members = ids(fx, name, filter).await;
            assert_eq!(count(tab, filter), members.len() as u64, "{name}/{filter}");
            assert!(
                members.is_subset(&base),
                "{name}/{filter} outside {base_filter}"
            );
            let overlap: Vec<_> = members.intersection(&seen).collect();
            assert!(overlap.is_empty(), "{name}/{filter} overlaps: {overlap:?}");
            seen.extend(members.iter().copied());
            sum += members.len() as u64;
        }
        assert!(sum <= base.len() as u64, "{name}: {sum} > {}", base.len());
        out.push((name.to_string(), sum, base.len() as u64));
    }
    out
}

// spec: UI-102
#[tokio::test]
async fn partitions_of_fx_ui_basic_are_disjoint_and_cover_all() {
    let fx = Fx::new();
    let checked = check_partitions(&fx).await;
    let tabs: Vec<&str> = checked.iter().map(|(t, ..)| t.as_str()).collect();
    assert_eq!(tabs, ["Internal", "External", "Response Codes"]);
    // fx-ui-basic: every URL of the base belongs to exactly one category.
    for (tab, sum, base) in &checked {
        assert_eq!(sum, base, "{tab}");
        assert!(*base > 0, "{tab}");
    }
    let overview = fx.call("sidebar.overview", json!({})).await;
    let titles = overview["tabs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["tab"] == "Page Titles")
        .unwrap();
    // Overlapping filters: no partition, so the chart shows bars without a sum.
    assert!(titles.get("partition").is_none_or(Value::is_null));
}

// spec: UI-102
#[tokio::test]
async fn partitions_stay_disjoint_on_unusual_urls() {
    let fx = Fx::new();
    // Every status class and content class once more, plus odd values: no status, status 0, 999, blocked by
    // robots.txt but shown in the tables, and redirects (subsets of 3XX).
    fx.write(
        "INSERT INTO urls(id,url,address,origin,is_internal,in_tables,state,discovered_via,folder_depth,url_length,content_class,status_code,status,indexability) VALUES
        (20,'https://site.test/a.js','https://site.test/a.js','https://site.test',1,1,'done','link',0,20,'JavaScript',200,'OK','Indexable'),
        (21,'https://site.test/a.css','https://site.test/a.css','https://site.test',1,1,'done','link',0,20,'CSS',500,'Server Error','Non-Indexable'),
        (22,'https://site.test/a.pdf','https://site.test/a.pdf','https://site.test',1,1,'done','link',0,20,'PDF',999,'Odd','Non-Indexable'),
        (23,'https://site.test/a.xml','https://site.test/a.xml','https://site.test',1,1,'done','link',0,20,'XML',0,NULL,NULL),
        (24,'https://site.test/a.woff','https://site.test/a.woff','https://site.test',1,1,'done','link',0,20,'Fonts',NULL,NULL,NULL),
        (25,'https://site.test/a.mp4','https://site.test/a.mp4','https://site.test',1,1,'done','link',0,20,'Media',302,'Found','Non-Indexable'),
        (26,'https://site.test/a.swf','https://site.test/a.swf','https://site.test',1,1,'done','link',0,20,'Plugins',410,'Gone','Non-Indexable'),
        (27,'https://site.test/blocked','https://site.test/blocked','https://site.test',1,1,'discovered','link',0,20,'Unknown',NULL,NULL,NULL),
        (28,'https://ext.test/x.css','https://ext.test/x.css','https://ext.test',0,1,'done','link',0,20,'CSS',503,'Unavailable',NULL),
        (29,'https://ext.test/x','https://ext.test/x','https://ext.test',0,1,'done','link',0,20,'Other',204,'No Content',NULL);
        UPDATE urls SET robots_blocked=1, not_crawled_reason='robots' WHERE id=27;
        UPDATE urls SET redirect_url='https://site.test/', redirect_target_id=1, redirect_type='http' WHERE id=25;",
    );
    let checked = check_partitions(&fx).await;
    assert_eq!(checked.len(), 3);
    for (tab, sum, base) in &checked {
        assert!(sum <= base, "{tab}");
    }
}

// spec: UI-102
#[tokio::test]
async fn ignore_but_report_keeps_response_codes_disjoint() {
    // `ignore_but_report` fetches URLs that robots.txt blocks and marks them (`robots_blocked = 1` with a real
    // status): such a URL is in `Blocked by Robots.txt` and in `Success (2XX)`. The partition then leaves the
    // blocked filter out; blocked URLs without a status fall into the residual category.
    let mut cfg = os_model::CrawlConfig::default();
    cfg.robots.mode = os_model::RobotsMode::IgnoreButReport;
    let fx = Fx::config(cfg);
    fx.write(
        "INSERT INTO urls(id,url,address,origin,is_internal,in_tables,state,discovered_via,folder_depth,url_length,content_class,status_code,status,indexability) VALUES
        (30,'https://site.test/robots-ok','https://site.test/robots-ok','https://site.test',1,1,'done','link',0,20,'HTML',200,'OK','Indexable'),
        (31,'https://site.test/robots-pending','https://site.test/robots-pending','https://site.test',1,1,'discovered','link',0,20,'Unknown',NULL,NULL,NULL);
        UPDATE urls SET robots_blocked=1 WHERE id IN (30,31);",
    );
    let overview = fx.call("sidebar.overview", json!({})).await;
    let codes = overview["tabs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["tab"] == "Response Codes")
        .unwrap();
    let filters: Vec<&str> = codes["partition"]["filters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f.as_str().unwrap())
        .collect();
    assert!(!filters.contains(&"Blocked by Robots.txt"));
    assert!(filters.contains(&"Success (2XX)"));
    assert_eq!(filters.len(), 6);
    // The blocked URL with status 200 is counted once, in its status class.
    assert!(ids(&fx, "Response Codes", "Blocked by Robots.txt").await.contains(&30));
    assert!(ids(&fx, "Response Codes", "Success (2XX)").await.contains(&30));
    let checked = check_partitions(&fx).await;
    let (_, sum, base) = checked.iter().find(|(t, ..)| t == "Response Codes").unwrap();
    assert!(sum <= base, "{sum} > {base}");
}
