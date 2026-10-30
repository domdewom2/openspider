#[path = "framework_common.rs"]
mod common;

use os_analyze::{
    analysis::{self, CancelToken},
    views::{self, ViewQuery},
};
use os_model::{CrawlConfig, FilterState, StepState};

// spec: ONPAGE-179
#[test]
fn missing_sizes_are_available_in_text_mode_and_have_an_overview_count() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = common::create(dir.path(), "sizes.ospider", 7);
    let cfg = CrawlConfig::default();
    file.conn()
        .execute_batch(include_str!(
            "../../../tests/fixtures/onpage-m2-images/analysis/facts.sql"
        ))
        .unwrap();
    assert_eq!(
        analysis::steps::images::run(&file, &cfg, &CancelToken::new()).unwrap(),
        StepState::NotApplicable
    );
    assert_eq!(
        views::state_of(&file, &cfg, "Images", "Missing Size Attributes").unwrap(),
        FilterState::Ready
    );
    assert_eq!(
        views::query_view(
            &file,
            &cfg,
            &ViewQuery::new("Images", "Missing Size Attributes")
        )
        .unwrap()
        .total,
        3
    );
    let counts = views::filter_counts(&file, &cfg).unwrap();
    let count = counts
        .iter()
        .find(|c| c.tab == "Images" && c.filter == "Missing Size Attributes")
        .unwrap();
    assert_eq!(count.count, Some(3));
    assert_eq!(
        views::state_of(&file, &cfg, "Images", "Background Images").unwrap(),
        FilterState::NeedsRendering
    );
}

// spec: ONPAGE-077
#[test]
fn internal_search_defaults_exclude_s_but_allow_explicit_configuration() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = common::create(dir.path(), "search.ospider", 8);
    let cases: Vec<(String, bool)> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/n07-onpage/search.json"
    ))
    .unwrap();
    for (i, (url, _)) in cases.iter().enumerate() {
        common::insert(
            &file,
            &[common::row(i as i64 + 1, url).int("status_code", 200)],
        );
    }
    file.conn()
        .execute("UPDATE urls SET url=address", [])
        .unwrap();
    let mut cfg = CrawlConfig::default();
    let page = views::query_view(&file, &cfg, &ViewQuery::new("URL", "Internal Search")).unwrap();
    assert_eq!(
        page.total,
        cases.iter().filter(|(_, hit)| *hit).count() as u64
    );
    for row in page.rows {
        let os_model::Value::Text(url) = &row[0] else {
            panic!("address")
        };
        assert!(cases.iter().any(|(expected, hit)| expected == url && *hit));
    }
    cfg.url.internal_search_params.push("s".into());
    assert_eq!(
        views::query_view(&file, &cfg, &ViewQuery::new("URL", "Internal Search"))
            .unwrap()
            .total,
        page.total + 2
    );
}

// spec: ONPAGE-023
// spec: ONPAGE-026
// spec: ONPAGE-024
#[test]
fn content_type_filters_partition_internal_and_external_in_both_profiles() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = common::create(dir.path(), "types.ospider", 9);
    let classes = [
        "HTML",
        "JavaScript",
        "CSS",
        "Images",
        "PDF",
        "Plugins",
        "Media",
        "Fonts",
        "XML",
        "Other",
        "Unknown",
    ];
    for (i, class) in classes.iter().enumerate() {
        common::insert(
            &file,
            &[common::row(i as i64 + 1, &format!("https://site.test/{i}"))
                .text("content_class", class)],
        );
        common::insert(
            &file,
            &[
                common::row(i as i64 + 20, &format!("https://other.test/{i}"))
                    .text("content_class", class)
                    .external(),
            ],
        );
    }
    for parity in [false, true] {
        let mut cfg = CrawlConfig::default();
        cfg.ui.parity_filters = parity;
        for name in ["Internal", "External"] {
            let tabs = os_analyze::tabs::configured(&cfg);
            let tab = tabs.iter().find(|t| t.id == name).unwrap();
            assert!(!tab.filters.iter().any(|f| f.id == "Flash"));
            let count = |filter| {
                views::query_view(&file, &cfg, &ViewQuery::new(name, filter))
                    .unwrap()
                    .total
            };
            let mut sum = 0;
            for class in classes {
                assert_eq!(count(class), 1, "{name}:{class} parity={parity}");
                sum += count(class);
            }
            assert_eq!(sum, count("All"));
        }
    }
}

// spec: ONPAGE-011 [M2]
#[test]
fn absent_second_keyword_length_matches_the_element_tab() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = common::create(dir.path(), "keywords.ospider", 10);
    common::insert(
        &file,
        &[common::row(1, "https://site.test/keywords")
            .text("content_class", "HTML")
            .int("status_code", 200)],
    );
    let cfg = CrawlConfig::default();
    let value = |tab| {
        let mut query = ViewQuery::new(tab, "All");
        query.columns = views::ColumnSelection::Named(vec!["Meta Keywords 2 Length".into()]);
        views::query_view(&file, &cfg, &query).unwrap().rows[0][0].clone()
    };
    assert_eq!(value("Internal"), os_model::Value::Int(0));
    assert_eq!(value("Internal"), value("Meta Keywords"));
}

// spec: ONPAGE-013
#[test]
fn internal_text_ratio_formats_persisted_precision_to_three_places() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = common::create(dir.path(), "ratio.ospider", 11);
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/n07-onpage/text-ratio.json"
    ))
    .unwrap();
    let raw = fixture["visible_chars"].as_f64().unwrap()
        / fixture["source_chars"].as_f64().unwrap()
        * 100.0;
    common::insert(
        &file,
        &[common::row(1, "https://site.test/ratio")
            .text("content_class", "HTML")
            .int("status_code", 200)
            .real("text_ratio", raw)],
    );
    let mut query = ViewQuery::new("Internal", "All");
    query.columns = views::ColumnSelection::Named(vec!["Text Ratio".into()]);
    let page = views::query_view(&file, &CrawlConfig::default(), &query).unwrap();
    assert_eq!(
        page.rows[0][0],
        os_model::Value::Float(fixture["expected"].as_f64().unwrap())
    );
}
