#[path = "framework_common.rs"]
mod common;
use os_analyze::views::{query_view, ColumnSelection, ViewQuery};
use os_model::{CrawlConfig, FilterState, UrlId, Value};
use os_store::CrawlFile;
fn fixture() -> (tempfile::TempDir, CrawlFile, CrawlConfig) {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = common::create(dir.path(), "sd.ospider", 9);
    file.conn()
        .execute_batch(include_str!("../../../tests/fixtures/tech-m2-sd/store.sql"))
        .unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.extraction.structured_data.json_ld = true;
    (dir, file, cfg)
}
fn page(file: &CrawlFile, cfg: &CrawlConfig, filter: &str) -> os_analyze::views::ViewPage {
    let mut q = ViewQuery::new("Structured Data", filter);
    q.columns = ColumnSelection::All;
    query_view(file, cfg, &q).unwrap()
}
fn ids(file: &CrawlFile, cfg: &CrawlConfig, filter: &str) -> Vec<i64> {
    let mut ids = page(file, cfg, filter)
        .row_keys
        .iter()
        .map(|id| id.0)
        .collect::<Vec<_>>();
    ids.sort();
    ids
}
// spec: TECH-183
#[test]
fn population_and_top_level_type_counts() {
    let (_dir, file, mut cfg) = fixture();
    assert_eq!(ids(&file, &cfg, "All"), [1, 2, 3, 4, 8, 9]);
    let p = page(&file, &cfg, "Contains Structured Data");
    let row = &p.rows[p.row_keys.iter().position(|id| id.0 == 1).unwrap()];
    for (column, wanted) in [
        ("Total Types", Value::Int(3)),
        ("Unique Types", Value::Int(2)),
        ("Type 1", Value::Text("Product".into())),
        ("Type 2", Value::Text("Thing".into())),
    ] {
        assert_eq!(
            row[p.columns.iter().position(|c| c == column).unwrap()],
            wanted,
            "{column}"
        );
    }
    file.conn()
        .execute(
            "UPDATE sd_items SET json=?1 WHERE id=1",
            [r#"{"@type":["https://vocab.test/Thing","https://schema.org/Product"],"_os_types":["Thing","Product"]}"#],
        )
        .unwrap();
    let reordered = page(&file, &cfg, "Contains Structured Data");
    let row = &reordered.rows[reordered.row_keys.iter().position(|id| id.0 == 1).unwrap()];
    assert_eq!(
        row[reordered
            .columns
            .iter()
            .position(|c| c == "Type 1")
            .unwrap()],
        Value::Text("Thing".into())
    );
    cfg.extraction.structured_data.microdata = true;
    cfg.extraction.structured_data.rdfa = true;
    let p = page(&file, &cfg, "Contains Structured Data");
    let row = &p.rows[p.row_keys.iter().position(|id| id.0 == 1).unwrap()];
    assert_eq!(
        row[p.columns.iter().position(|c| c == "Total Types").unwrap()],
        Value::Int(5)
    );
    assert!(p.columns.iter().any(|c| c == "Type 4"));
    assert!(!p.columns.iter().any(|c| c == "Type 5"));
    cfg.extraction.structured_data.json_ld = false;
    cfg.extraction.structured_data.microdata = false;
    cfg.extraction.structured_data.rdfa = false;
    assert!(matches!(
        os_analyze::views::state_of(&file, &cfg, "Structured Data", "All").unwrap(),
        FilterState::NeedsConfig(_)
    ));
}
// spec: TECH-184
// spec: TECH-185
// spec: TECH-186
#[test]
fn active_formats_and_issue_indexability_share_view_counts() {
    let (_dir, file, mut cfg) = fixture();
    assert_eq!(ids(&file, &cfg, "Contains Structured Data"), [1, 2, 9]);
    cfg.issues.ignore_non_indexable = false;
    assert_eq!(ids(&file, &cfg, "Missing Structured Data"), [3, 4, 8]);
    assert_eq!(ids(&file, &cfg, "Parse Errors"), [2, 9]);
    cfg.issues.ignore_non_indexable = true;
    assert_eq!(ids(&file, &cfg, "Missing Structured Data"), [4, 8]);
    assert_eq!(ids(&file, &cfg, "Parse Errors"), [2]);
    assert_eq!(ids(&file, &cfg, "JSON-LD URLs"), [1]);
    assert!(ids(&file, &cfg, "Microdata URLs").is_empty());
    cfg.extraction.structured_data.microdata = true;
    assert_eq!(ids(&file, &cfg, "Microdata URLs"), [1, 4]);
    assert_eq!(ids(&file, &cfg, "Missing Structured Data"), [8]);
    let counts = os_analyze::views::filter_counts(&file, &cfg).unwrap();
    let count = counts
        .iter()
        .find(|c| c.tab == "Structured Data" && c.filter == "Parse Errors")
        .unwrap();
    assert_eq!(count.count, Some(1));
}
// spec: TECH-189
#[test]
fn details_preserve_format_order_and_parse_error_metadata_and_page_selection() {
    let (_dir, file, _cfg) = fixture();
    let rows = os_store::queries::structured_data::detail_rows(&file, &[UrlId(1), UrlId(2)], 0, 20)
        .unwrap();
    assert_eq!(rows.total, 5);
    assert_eq!(rows.rows.len(), 5);
    assert_eq!(rows.rows[0][2], Value::Text("json-ld".into()));
    assert_eq!(rows.rows[2][2], Value::Text("microdata".into()));
    let Value::Text(json) = &rows.rows[4][4] else {
        panic!("JSON");
    };
    let v: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(v["_os_parse_error"]["line"], 2);
    let paged = os_store::queries::structured_data::detail_rows(&file, &[UrlId(2)], 0, 1).unwrap();
    assert_eq!(paged.total, 1);
    let empty = os_store::queries::structured_data::detail_rows(&file, &[], 0, 20).unwrap();
    assert!(empty.rows.is_empty());
}

// spec: TECH-180
// spec: TECH-181
// spec: TECH-182
// spec: TECH-183
// spec: TECH-185
#[tokio::test]
async fn fixture_crawl_persists_all_formats_through_the_writer() {
    let mut cfg = CrawlConfig::default();
    cfg.extraction.structured_data.json_ld = true;
    cfg.extraction.structured_data.microdata = true;
    cfg.extraction.structured_data.rdfa = true;
    let crawl =
        os_engine::testing::crawl_fixture_from("tech-m2-sd", "/mixed.html", cfg.clone()).await;
    let p = page(&crawl.file, &cfg, "Contains Structured Data");
    assert_eq!(p.total, 1);
    let row = &p.rows[0];
    assert_eq!(
        row[p.columns.iter().position(|c| c == "Total Types").unwrap()],
        Value::Int(3)
    );
    let data =
        os_store::queries::structured_data::detail_rows(&crawl.file, &p.row_keys, 0, 20).unwrap();
    assert_eq!(data.total, 3);
    for filter in ["JSON-LD URLs", "Microdata URLs", "RDFa URLs"] {
        assert_eq!(page(&crawl.file, &cfg, filter).total, 1);
    }
}

// spec: TECH-186
// spec: TECH-189
#[tokio::test]
async fn fixture_crawl_persists_parse_errors_without_losing_valid_blocks() {
    let mut cfg = CrawlConfig::default();
    cfg.extraction.structured_data.json_ld = true;
    let crawl =
        os_engine::testing::crawl_fixture_from("tech-m2-sd", "/errors.html", cfg.clone()).await;
    let p = page(&crawl.file, &cfg, "Parse Errors");
    assert_eq!(p.total, 1);
    let data =
        os_store::queries::structured_data::detail_rows(&crawl.file, &p.row_keys, 0, 20).unwrap();
    assert_eq!(data.total, 5);
    assert_eq!(
        data.rows
            .iter()
            .filter(|r| r[5] == Value::Text("parse".into()))
            .count(),
        4
    );
    assert_eq!(page(&crawl.file, &cfg, "Contains Structured Data").total, 1);
    assert_eq!(page(&crawl.file, &cfg, "Missing Structured Data").total, 0);
}
