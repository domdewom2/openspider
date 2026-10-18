//! M2-D07 store-contract fixtures: hreflang/, out-hreflang and outside-head (K2-24).
use os_analyze::{
    analysis,
    details::DetailReq,
    tabs::hreflang,
    views::{query_view, state_of, ViewQuery},
    SortKey,
};
use os_model::{
    time::{IdSource, SeededIds},
    AnalysisStep, CancelToken, CrawlConfig, CrawlMode, FilterState, HreflangOverLimit, StepState,
    Timestamp, UrlId, Value,
};
use os_store::{CrawlFile, CrawlInit, StorageProfile};

fn fixture() -> (tempfile::TempDir, CrawlFile, CrawlConfig) {
    let dir = tempfile::tempdir().unwrap();
    let cfg = CrawlConfig::default();
    let at = Timestamp::from_db("2026-10-18T12:00:00.000Z").unwrap();
    let init = CrawlInit {
        crawl_id: SeededIds::new(7).new_crawl_id(at),
        mode: CrawlMode::Spider,
        start_url: Some("https://site.test/1".into()),
        list_source: None,
        project: "test".into(),
        name: "hreflang".into(),
        config: cfg.clone(),
        created_at: at,
    };
    let file = CrawlFile::create(
        &dir.path().join("test.ospider"),
        &init,
        StorageProfile::Balanced,
    )
    .unwrap();
    file.conn()
        .execute_batch(include_str!(
            "../../../tests/fixtures/tech-m2-hreflang/relations.sql"
        ))
        .unwrap();
    (dir, file, cfg)
}
fn ids(file: &CrawlFile, cfg: &CrawlConfig, filter: &str) -> Vec<i64> {
    query_view(file, cfg, &ViewQuery::new("Hreflang", filter))
        .unwrap()
        .rows
        .iter()
        .map(|r| match &r[0] {
            Value::Text(s) => s.rsplit('/').next().unwrap().parse().unwrap(),
            _ => panic!("address"),
        })
        .collect()
}
fn has(file: &CrawlFile, cfg: &CrawlConfig, filter: &str, id: i64) -> bool {
    ids(file, cfg, filter).contains(&id)
}
fn detail(file: &CrawlFile, cfg: &CrawlConfig, ids: &[UrlId]) -> os_analyze::details::DetailRows {
    hreflang::detail_rows(
        file,
        cfg,
        &DetailReq {
            url_ids: ids,
            params: &serde_json::Value::Null,
            sort: &[],
            offset: 0,
            limit: 500,
        },
    )
    .unwrap()
}
fn text(v: &Value) -> &str {
    if let Value::Text(s) = v {
        s
    } else {
        ""
    }
}

// spec: TECH-075
#[test]
fn unlinked_is_postcrawl_repeatable_and_stamps_sources() {
    let (_dir, file, cfg) = fixture();
    assert_eq!(
        state_of(&file, &cfg, "Hreflang", "Unlinked Hreflang URLs").unwrap(),
        FilterState::NeedsCrawlAnalysis
    );
    assert!(ids(&file, &cfg, "Unlinked Hreflang URLs").is_empty());
    let cancel = CancelToken::new();
    assert_eq!(
        analysis::steps::hreflang::run(&file, &cfg, &cancel).unwrap(),
        StepState::Done
    );
    analysis::persist::write_step(&file, AnalysisStep::Hreflang, StepState::Done, None, None)
        .unwrap();
    assert!(has(&file, &cfg, "Unlinked Hreflang URLs", 1));
    let page = hreflang::report_rows(&file, &cfg, "Unlinked Hreflang URLs", 0).unwrap();
    let targets: Vec<_> = page.rows.iter().map(|r| text(&r[2])).collect();
    assert_eq!(
        targets,
        [
            "https://site.test/3",
            "https://site.test/4",
            "https://site.test/5",
            "https://site.test/8",
            "https://site.test/16"
        ]
    );
    let seq = file.get_meta("change_seq").unwrap();
    analysis::steps::hreflang::run(&file, &cfg, &cancel).unwrap();
    assert_eq!(file.get_meta("change_seq").unwrap(), seq);
    file.conn()
        .execute("UPDATE url_counters SET inlinks=1 WHERE url_id=3", [])
        .unwrap();
    analysis::steps::hreflang::run(&file, &cfg, &cancel).unwrap();
    assert!(file
        .conn()
        .query_row("SELECT change_seq > 0 FROM urls WHERE id=1", [], |r| r
            .get::<_, bool>(0))
        .unwrap());
    assert!(
        !hreflang::report_rows(&file, &cfg, "Unlinked Hreflang URLs", 0)
            .unwrap()
            .rows
            .iter()
            .any(|r| text(&r[2]) == "https://site.test/3")
    );
    cancel.cancel();
    assert_eq!(
        analysis::steps::hreflang::run(&file, &cfg, &cancel).unwrap(),
        StepState::Pending
    );
}

// spec: TECH-077
#[test]
fn inconsistency_uses_source_selfcode_and_backlink_case_insensitively() {
    let (_dir, file, cfg) = fixture();
    assert!(has(
        &file,
        &cfg,
        "Inconsistent Language & Region Return Links",
        1
    )); // target 7 returns en, self en-gb
    assert!(!has(
        &file,
        &cfg,
        "Inconsistent Language & Region Return Links",
        11
    )); // no selfcode
    file.conn()
        .execute(
            "UPDATE hreflang SET code_norm='en-gb',code_raw='EN-gb' WHERE url_id=7",
            [],
        )
        .unwrap();
    assert!(!has(
        &file,
        &cfg,
        "Inconsistent Language & Region Return Links",
        1
    ));
    assert!(has(&file, &cfg, "Missing Return Links", 1)); // independent live predicate
}
// spec: TECH-078
#[test]
fn canonical_alternates_exclude_selfcanon_and_uncrawled() {
    let (_dir, file, cfg) = fixture();
    assert!(has(&file, &cfg, "Non-Canonical Return Links", 1));
    assert!(!has(&file, &cfg, "Non-Canonical Return Links", 2));
    file.conn()
        .execute("UPDATE urls SET state='queued' WHERE id IN (6,7)", [])
        .unwrap();
    assert!(!has(&file, &cfg, "Non-Canonical Return Links", 1));
}
// spec: TECH-079
#[test]
fn effective_noindex_does_not_hide_other_problems() {
    let (_dir, file, cfg) = fixture();
    assert!(has(&file, &cfg, "Noindex Return Links", 1));
    assert!(has(&file, &cfg, "Missing Return Links", 1));
    file.conn()
        .execute("UPDATE urls SET effective_index=1 WHERE id=6", [])
        .unwrap();
    assert!(!has(&file, &cfg, "Noindex Return Links", 1));
}
// spec: TECH-081
#[test]
fn duplicates_count_identical_annotations_across_sources() {
    let (_dir, file, cfg) = fixture();
    assert_eq!(ids(&file, &cfg, "Multiple Entries"), [12]);
    file.conn()
        .execute(
            "UPDATE hreflang SET code_norm='de-de' WHERE url_id=12 AND source='sitemap'",
            [],
        )
        .unwrap();
    assert!(ids(&file, &cfg, "Multiple Entries").is_empty());
    file.conn()
        .execute(
            "UPDATE hreflang SET code_norm='x-default' WHERE url_id=12",
            [],
        )
        .unwrap();
    assert_eq!(ids(&file, &cfg, "Multiple Entries"), [12]);
}
// spec: TECH-082
#[test]
fn self_reference_includes_xdefault_and_all_sources() {
    let (_dir, file, cfg) = fixture();
    assert!(has(&file, &cfg, "Missing Self Reference", 11));
    assert!(!has(&file, &cfg, "Missing Self Reference", 13));
    assert!(!has(&file, &cfg, "Missing Self Reference", 2));
    assert!(!has(&file, &cfg, "Missing Self Reference", 14));
}
// spec: TECH-083
#[test]
fn own_canonical_must_be_used_in_annotations() {
    let (_dir, file, cfg) = fixture();
    assert!(has(&file, &cfg, "Not Using Canonical", 10));
    assert!(!has(&file, &cfg, "Not Using Canonical", 1));
    file.conn()
        .execute("UPDATE hreflang SET target_id=2 WHERE url_id=10", [])
        .unwrap();
    assert!(!has(&file, &cfg, "Not Using Canonical", 10));
}
// spec: TECH-084
#[test]
fn missing_xdefault_requires_annotations_and_ignores_case() {
    let (_dir, file, cfg) = fixture();
    assert!(has(&file, &cfg, "Missing X-Default", 1));
    assert!(!has(&file, &cfg, "Missing X-Default", 13));
    assert!(!has(&file, &cfg, "Missing X-Default", 14));
}
// spec: TECH-086
// spec: TECH-008 [M2]
#[test]
fn outside_head_requires_validation_coverage_and_preserves_other_columns() {
    let (_dir, file, mut cfg) = fixture();
    assert!(has(&file, &cfg, "Outside <head>", 13));
    assert!(matches!(
        state_of(&file, &cfg, "Hreflang", "Outside <head>").unwrap(),
        FilterState::Incomplete { .. }
    ));
    cfg.advanced.html_validation = false;
    assert!(ids(&file, &cfg, "Outside <head>").is_empty());
    assert!(matches!(
        state_of(&file, &cfg, "Hreflang", "Outside <head>").unwrap(),
        FilterState::NeedsConfig(_)
    ));
    assert_eq!(detail(&file, &cfg, &[UrlId(13)]).rows.len(), 1);
    cfg.advanced.html_validation = true;
    file.conn()
        .execute("DELETE FROM url_ext WHERE key='validation.checked'", [])
        .unwrap();
    assert_eq!(
        state_of(&file, &cfg, "Hreflang", "Outside <head>").unwrap(),
        FilterState::NeedsRecrawl
    );
}
// spec: TECH-089
#[test]
fn details_cover_precedence_multiselection_sort_and_truncation() {
    let (_dir, file, cfg) = fixture();
    let data = detail(&file, &cfg, &[UrlId(1)]);
    assert_eq!(data.total, 10);
    assert_eq!(
        data.rows.iter().map(|r| text(&r[5])).collect::<Vec<_>>(),
        [
            "Self",
            "Confirmed",
            "Missing",
            "Non-200",
            "Not Crawled",
            "Noindex",
            "Non-Canonical",
            "Not Applicable",
            "Not Applicable",
            "Missing"
        ]
    );
    assert_eq!(
        data.notes,
        [(
            "Hreflang Truncated".into(),
            "5 weitere Annotationen nicht ausgewertet".into()
        )]
    );
    let data = detail(&file, &cfg, &[UrlId(1), UrlId(2)]);
    assert_eq!(data.columns[0].0, "Address");
    assert_eq!(data.total, 12);
    let req = DetailReq {
        url_ids: &[UrlId(1)],
        params: &serde_json::Value::Null,
        sort: &[SortKey {
            column: "Language Code".into(),
            descending: true,
        }],
        offset: 1,
        limit: 2,
    };
    let data = hreflang::detail_rows(&file, &cfg, &req).unwrap();
    assert_eq!(data.rows.len(), 2);
    assert_eq!(text(&data.rows[0][1]), "pt");
    assert_eq!(text(&data.rows[1][1]), "pl");
    file.conn()
        .execute(
            "UPDATE urls SET canonical_count=0,canonical_self=NULL WHERE id=7",
            [],
        )
        .unwrap();
    assert_eq!(
        text(&detail(&file, &cfg, &[UrlId(1)]).rows[6][5]),
        "Inconsistent"
    );
}
// spec: TECH-071 [M2]
#[test]
fn indexability_columns_and_skip_mode_apply_to_every_filter() {
    let (_dir, file, mut cfg) = fixture();
    let data = query_view(&file, &cfg, &ViewQuery::new("Hreflang", "All")).unwrap();
    assert_eq!(
        &data.columns[data.columns.len() - 2..],
        ["Indexability", "Indexability Status"]
    );
    cfg.hreflang.over_limit = HreflangOverLimit::Skip;
    assert!(!has(&file, &cfg, "All", 1));
    cfg.spider.hreflang.store = false;
    assert!(ids(&file, &cfg, "All").is_empty());
    assert!(detail(&file, &cfg, &[UrlId(1)]).rows.is_empty());
}
