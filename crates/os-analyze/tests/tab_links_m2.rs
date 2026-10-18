#[path = "tab_links_support.rs"]
mod support;
use os_analyze::analysis::{steps::links, CancelToken};
use os_model::{CrawlConfig, StepState, UrlId, Value};
use os_store::queries::anchors::{self, AnchorOptions};
use support::*;

// spec: TECH-136
#[test]
fn phrases_are_unicode_normalized_exact_matches_and_configurable() {
    let (_dir, file) = file();
    fixture(&file);
    let mut cfg = CrawlConfig::default();
    assert_eq!(
        ids(
            &file,
            &cfg,
            "Non-Descriptive Anchor Text In Internal Outlinks"
        ),
        [1, 2, 3]
    );
    cfg.links.non_descriptive_anchors = vec!["hier klicken für preise".into()];
    assert_eq!(
        ids(
            &file,
            &cfg,
            "Non-Descriptive Anchor Text In Internal Outlinks"
        ),
        [1]
    );
    cfg.links.non_descriptive_anchors = vec!["\u{2003}ÄNDERUNG!?\u{2003}".into()];
    sql(
        &file,
        "UPDATE links SET anchor = 'Änderung!' WHERE source_id = 7",
    );
    assert_eq!(anchors::phrase("\u{2003}Änderung!?\u{2003}"), "änderung");
    assert_eq!(
        ids(
            &file,
            &cfg,
            "Non-Descriptive Anchor Text In Internal Outlinks"
        ),
        [7]
    );
}
// spec: TECH-137
// spec: TECH-138
#[test]
fn thresholds_count_follow_instances_without_resources_or_self_links() {
    let (_dir, file) = file();
    fixture(&file);
    let mut cfg = CrawlConfig::default();
    assert!(ids(&file, &cfg, "Pages With High External Outlinks").is_empty());
    cfg.links.high_external_outlinks = 1;
    assert_eq!(ids(&file, &cfg, "Pages With High External Outlinks"), [1]);
    cfg.links.high_external_outlinks = 2;
    assert!(ids(&file, &cfg, "Pages With High External Outlinks").is_empty());
    cfg.links.high_internal_outlinks = 4;
    assert_eq!(
        ids(&file, &cfg, "Pages With High Internal Outlinks"),
        [1, 3]
    );
}
// spec: TECH-139
// spec: TECH-140
// spec: TECH-142
#[test]
fn incoming_analysis_is_atomic_repeatable_and_excludes_self_and_resources() {
    let (_dir, file) = file();
    fixture(&file);
    let cfg = CrawlConfig::default();
    assert_eq!(
        links::run(&file, &cfg, &CancelToken::new()).unwrap(),
        StepState::Done
    );
    // Query predicates independently of runner state, then verify runner publication below.
    for (key, expected) in [
        ("link.mixed_inlinks", vec![4]),
        ("link.nofollow_only", vec![5]),
        ("link.non_indexable_only", vec![5, 6]),
    ] {
        let actual: Vec<i64> = file
            .conn()
            .prepare("SELECT url_id FROM url_ext WHERE key = ?1 ORDER BY url_id")
            .unwrap()
            .query_map([key], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(actual, expected);
    }
    let cancel = CancelToken::new();
    cancel.cancel();
    assert_eq!(
        links::run(&file, &cfg, &cancel).unwrap(),
        StepState::Pending
    );
    sql(&file, "DELETE FROM links WHERE target_id = 5");
    links::run(&file, &cfg, &CancelToken::new()).unwrap();
    assert_eq!(
        file.conn()
            .query_row(
                "SELECT COUNT(*) FROM url_ext WHERE key='link.nofollow_only'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    os_analyze::analysis::runner::run_only(
        &file,
        &cfg,
        &CancelToken::new(),
        &|_, _| {},
        &[os_model::AnalysisStep::Links],
    )
    .unwrap();
    assert_eq!(ids(&file, &cfg, "Non-Indexable Page Inlinks Only"), [6]);
}
// spec: TECH-141
#[test]
fn localhost_covers_resources_and_respects_only_exact_start_authority() {
    let (_dir, file) = file();
    fixture(&file);
    let cfg = CrawlConfig::default();
    assert_eq!(ids(&file, &cfg, "Outlinks To Localhost"), [1]);
    for url in [
        "http://localhost:9/x",
        "http://sub.localhost/",
        "http://127.255.1.2/",
        "http://[::1]:80/",
        "http://0.0.0.0/",
    ] {
        assert!(anchors::is_loopback(url), "{url}");
    }
    for url in [
        "http://192.168.1.1/",
        "http://localhost.example/",
        "http://10.0.0.1/",
        "http://127.example/",
    ] {
        assert!(!anchors::is_loopback(url), "{url}");
    }
    sql(
        &file,
        "UPDATE crawl_meta SET value='http://127.2.3.4/' WHERE key='start_url'",
    );
    assert!(ids(&file, &cfg, "Outlinks To Localhost").is_empty());
    sql(
        &file,
        "UPDATE crawl_meta SET value='https://127.2.3.4/' WHERE key='start_url'",
    );
    assert_eq!(ids(&file, &cfg, "Outlinks To Localhost"), [1]);
}
// spec: TECH-143
#[test]
fn uncrawlable_filter_requires_option_and_an_internal_extracted_target() {
    let (_dir, file) = file();
    fixture(&file);
    let mut cfg = CrawlConfig::default();
    assert!(ids(&file, &cfg, "Pages With Uncrawlable Internal Outlinks").is_empty());
    cfg.spider.store_uncrawlable_links = true;
    assert_eq!(
        ids(&file, &cfg, "Pages With Uncrawlable Internal Outlinks"),
        [8]
    );
    sql(&file, "UPDATE links SET target_id=NULL WHERE source_id=8");
    assert!(ids(&file, &cfg, "Pages With Uncrawlable Internal Outlinks").is_empty());
}
// spec: TECH-146
#[test]
fn anchors_aggregate_page_drilldown_and_apply_both_directions_and_exclusions() {
    let (_dir, file) = file();
    fixture(&file);
    let data = anchors::detail_rows(&file, &[UrlId(4)], 0, 50).unwrap();
    let products = data
        .rows
        .iter()
        .find(|r| r[0] == Value::Text("Produkte".into()))
        .unwrap();
    assert_eq!(
        &products[1..6],
        &[
            Value::Int(4),
            Value::Int(4),
            Value::Int(1),
            Value::Int(3),
            Value::Int(1)
        ]
    );
    assert!(data
        .rows
        .iter()
        .any(|r| r[0] == Value::Text("produkte".into())));
    assert!(data
        .rows
        .iter()
        .any(|r| r[0] == Value::Text("(leer)".into())));
    let percent: f64 = data
        .rows
        .iter()
        .map(|r| {
            if let Value::Float(n) = r[6] {
                n
            } else {
                panic!()
            }
        })
        .sum();
    assert!((percent - 100.0).abs() < 1e-9);
    assert_eq!(
        anchors::detail_rows(&file, &[UrlId(4)], 1, 1)
            .unwrap()
            .total,
        data.total
    );
    assert_eq!(
        anchors::matching_link_ids(&file, &[UrlId(4)], "Produkte", AnchorOptions::default())
            .unwrap()
            .len(),
        4
    );
    let opts = AnchorOptions {
        exclude_nofollow: true,
        ..AnchorOptions::default()
    };
    assert_eq!(
        anchors::matching_link_ids(&file, &[UrlId(4)], "Produkte", opts)
            .unwrap()
            .len(),
        3
    );
    let opts = AnchorOptions {
        outlinks: true,
        exclude_self: false,
        ..opts
    };
    assert_eq!(
        anchors::matching_link_ids(&file, &[UrlId(4)], "Produkte", opts)
            .unwrap()
            .len(),
        1
    );
    assert!(anchors::detail_rows(&file, &[], 0, 50)
        .unwrap()
        .rows
        .is_empty());
}

// spec: TECH-146
#[test]
fn anchors_registry_preserves_numeric_types_and_selection_options() {
    use os_analyze::details::{detail_source, DetailKind, DetailReq};
    let (_dir, file) = file();
    fixture(&file);
    let (_, source) = detail_source(DetailKind::Anchors);
    let params = serde_json::json!({"excludeNofollow":true,"excludeSelf":false});
    let req = DetailReq {
        url_ids: &[UrlId(4)],
        params: &params,
        sort: &[],
        offset: 0,
        limit: 50,
    };
    let data = source(&file, &CrawlConfig::default(), &req).unwrap();
    assert_eq!(data.columns[1].1, os_analyze::ColumnKind::Int);
    assert_eq!(
        data.columns[6].1,
        os_analyze::ColumnKind::Float { decimals: 2 }
    );
    let products = data
        .rows
        .iter()
        .find(|r| r[0] == Value::Text("Produkte".into()))
        .unwrap();
    assert_eq!(products[1], Value::Int(4)); // Three followed sources and the self-link.
    let params = serde_json::json!({"direction":"outlinks","excludeSelf":false});
    let req = DetailReq {
        params: &params,
        ..req
    };
    let data = source(&file, &CrawlConfig::default(), &req).unwrap();
    assert_eq!(data.rows.len(), 1);
    assert_eq!(data.rows[0][1], Value::Int(1));
}
