#[path = "framework_common.rs"]
mod fixture;
use os_analyze::{
    search::FilterSource,
    segments::{self, SegmentFilter},
};
use os_model::{
    CancelToken, CrawlConfig, SegmentCondition, SegmentConfig, SegmentGroup, SegmentOp,
};

fn cond(field: &str, op: SegmentOp, value: &str) -> SegmentCondition {
    SegmentCondition {
        field: field.into(),
        op,
        value: value.into(),
    }
}
fn segment(name: &str, conditions: Vec<SegmentCondition>) -> SegmentConfig {
    SegmentConfig {
        name: name.into(),
        color: "#336699".into(),
        groups: vec![SegmentGroup { conditions }],
    }
}
fn setup(file: &os_store::CrawlFile) {
    fixture::insert(
        file,
        &[
            fixture::row(1, "https://site.test/blog/a")
                .int("status_code", 200)
                .int("word_count", 100)
                .int("crawl_depth", 0)
                .text("indexability", "Indexable"),
            fixture::row(2, "https://site.test/blog/b")
                .int("status_code", 404)
                .int("word_count", 200)
                .int("crawl_depth", 12),
            fixture::row(3, "https://site.test/shop/a")
                .int("status_code", 500)
                .int("word_count", 300),
            fixture::row(4, "https://site.test/info/a")
                .null("status_code")
                .null("word_count"),
            fixture::row(5, "https://outside.test/blog/a")
                .external()
                .int("status_code", 200),
            fixture::row(6, "https://site.test/Blog/a").int("status_code", 301),
        ],
    );
}
fn cfg() -> CrawlConfig {
    let mut cfg = CrawlConfig::default();
    cfg.segments.list = vec![
        segment("Blog", vec![cond("Address", SegmentOp::Contains, "/blog/")]),
        segment(
            "Error",
            vec![cond("Status Code", SegmentOp::GreaterThan, "399")],
        ),
    ];
    cfg
}
// spec: CRAWL-134
// spec: UI-068 [P1]
#[test]
fn fixture_priority_groups_numeric_nulls_and_case_sensitive_text() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = fixture::create(dir.path(), "rules.ospider", 14);
    setup(&file);
    let mut cfg = cfg();
    let a = segments::preview(&file, &cfg, &CancelToken::new()).unwrap();
    assert_eq!(a.segments[0].iter().collect::<Vec<_>>(), [1, 2]);
    assert_eq!(a.segments[1].iter().collect::<Vec<_>>(), [3]);
    assert_eq!(a.unassigned.iter().collect::<Vec<_>>(), [4, 6]);
    cfg.segments.list.swap(0, 1);
    let a = segments::preview(&file, &cfg, &Default::default()).unwrap();
    assert_eq!(a.segments[0].iter().collect::<Vec<_>>(), [2, 3]);
    assert_eq!(a.segments[1].iter().collect::<Vec<_>>(), [1]);
    cfg.segments.list = vec![segment(
        "Mixed",
        vec![
            cond("Address", SegmentOp::Contains, "/shop/"),
            cond("Status Code", SegmentOp::GreaterOrEqual, "500"),
        ],
    )];
    cfg.segments.list[0].groups.push(SegmentGroup {
        conditions: vec![cond("Address", SegmentOp::RegexMatches, r"(?i)/blog/")],
    });
    let a = segments::preview(&file, &cfg, &Default::default()).unwrap();
    assert_eq!(a.segments[0].iter().collect::<Vec<_>>(), [1, 2, 3, 6]);
}
// spec: CRAWL-134
#[test]
fn references_are_validated_and_rest_is_the_remaining_population() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = fixture::create(dir.path(), "refs.ospider", 14);
    setup(&file);
    let mut cfg = cfg();
    cfg.segments.list.push(segment(
        "Rest",
        vec![cond("Segment", SegmentOp::InNoSegment, "")],
    ));
    assert!(segments::validate(&cfg).is_empty());
    let a = segments::preview(&file, &cfg, &Default::default()).unwrap();
    assert_eq!(a.segments[2].iter().collect::<Vec<_>>(), [4, 6]);
    assert!(a.unassigned.is_empty());
    cfg.segments.list[1].groups[0]
        .conditions
        .push(cond("Segment", SegmentOp::InSegment, "Blog"));
    assert!(segments::validate(&cfg).is_empty());
    cfg.segments.list.swap(0, 1);
    assert!(segments::validate(&cfg)
        .iter()
        .any(|v| v.message == "segment-reference-must-be-above"));
}
// spec: CRAWL-134
// spec: UI-020 [P1]
#[test]
fn persisted_filters_column_reassignment_and_json_roundtrip_share_one_assignment() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = fixture::create(dir.path(), "saved.ospider", 14);
    setup(&file);
    let cfg = cfg();
    let assignment = segments::reassign(&file, &cfg, &Default::default()).unwrap();
    let json = serde_json::to_string(&cfg.segments).unwrap();
    let mut roundtrip = cfg.clone();
    roundtrip.segments = serde_json::from_str(&json).unwrap();
    assert_eq!(
        segments::preview(&file, &roundtrip, &Default::default()).unwrap(),
        assignment
    );
    assert_eq!(
        SegmentFilter::new("Blog").bitmap(&file).unwrap(),
        assignment.segments[0]
    );
    assert_eq!(
        SegmentFilter::unassigned().bitmap(&file).unwrap(),
        assignment.unassigned
    );
    let tabs = os_analyze::tabs::configured_for_file(&cfg, &file);
    let mut q = os_analyze::views::ViewQuery::new("Internal", "All");
    q.columns = os_analyze::views::ColumnSelection::Named(vec!["Segment".into()]);
    let page = os_analyze::views::query_view_in(&tabs, &file, &cfg, &q).unwrap();
    assert!(page
        .rows
        .iter()
        .any(|r| r.contains(&os_model::Value::Text("Blog".into()))));
    let mut reversed = cfg.clone();
    reversed.segments.list.reverse();
    segments::reassign(&file, &reversed, &Default::default()).unwrap();
    assert_eq!(
        SegmentFilter::new("Error")
            .bitmap(&file)
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        [2, 3]
    );
    assert_eq!(
        file.conn()
            .query_row("SELECT COUNT(*) FROM urls", [], |r| r.get::<_, u64>(0))
            .unwrap(),
        6
    );
    assert_eq!(
        segments::saved_config(&file).unwrap().segments,
        reversed.segments
    );
    assert_eq!(file.config().unwrap().segments, reversed.segments);
}
// spec: CRAWL-134
// spec: UI-068 [P1]
#[test]
fn invalid_names_regex_types_and_cancellation_never_destroy_assignments() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = fixture::create(dir.path(), "invalid.ospider", 14);
    setup(&file);
    let mut cfg = cfg();
    segments::reassign(&file, &cfg, &Default::default()).unwrap();
    cfg.segments.list[1].name = "blog".into();
    assert!(segments::reassign(&file, &cfg, &Default::default()).is_err());
    cfg.segments.list[1].name = "Error".into();
    cfg.segments.list[0].groups[0].conditions[0] = cond("Address", SegmentOp::RegexMatches, "a(");
    let errors = segments::validate(&cfg);
    assert!(errors
        .iter()
        .any(|e| e.path.ends_with("conditions.0") && e.line == 1));
    cfg.segments.list[0].groups[0].conditions[0] = cond("Address", SegmentOp::GreaterThan, "2");
    assert!(!segments::validate(&cfg).is_empty());
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(matches!(
        segments::reassign(&file, &crate::cfg(), &cancel),
        Err(segments::SegmentError::Cancelled)
    ));
    assert_eq!(SegmentFilter::new("Blog").bitmap(&file).unwrap().len(), 2);
}
// spec: UI-067 [P1]
#[test]
fn statistics_partition_internal_urls_and_include_depth_zero_and_ten_plus() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = fixture::create(dir.path(), "stats.ospider", 14);
    setup(&file);
    let cfg = cfg();
    segments::reassign(&file, &cfg, &Default::default()).unwrap();
    let rows = segments::statistics(&file, &cfg, &Default::default()).unwrap();
    assert_eq!(rows.iter().map(|r| r.urls).sum::<u64>(), 5);
    assert_eq!(rows[0].urls, 2);
    assert_eq!(rows[0].percent, 40.0);
    assert_eq!(rows[0].depths["0"], 1);
    assert_eq!(rows[0].depths["10+"], 1);
    assert!(rows.last().unwrap().name.is_none());
}
// spec: CRAWL-134
#[test]
fn multivalue_conditions_match_any_value_including_negative_operators() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = fixture::create(dir.path(), "multi.ospider", 14);
    setup(&file);
    file.conn().execute_batch("INSERT INTO directives(url_id,source,position,raw,tokens,applies,in_head) VALUES(1,'meta_robots',0,'index','index',1,1),(1,'meta_robots',1,'follow','follow',1,1)").unwrap();
    let mut cfg = cfg();
    cfg.segments.list = vec![segment(
        "Multi",
        vec![cond("Meta Robots", SegmentOp::NotEquals, "index")],
    )];
    let a = segments::preview(&file, &cfg, &Default::default()).unwrap();
    assert!(a.segments[0].contains(1));
}
// spec: CRAWL-134
#[test]
fn reassign_one_hundred_thousand_urls_within_spec_budget() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = fixture::create(dir.path(), "budget.ospider", 14);
    file.conn().execute_batch("WITH RECURSIVE n(i) AS (VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i<100000) INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,folder_depth,url_length) SELECT i,'https://site.test/blog/'||i,'https://site.test/blog/'||i,'https://site.test',1,'done','hyperlink',0,30 FROM n").unwrap();
    let start = std::time::Instant::now();
    let a = segments::reassign(&file, &cfg(), &Default::default()).unwrap();
    assert_eq!(a.segments[0].len(), 100000);
    assert!(start.elapsed().as_secs_f64() < 5.0, "{:?}", start.elapsed());
}

// spec: CRAWL-134
#[test]
fn incremental_updates_and_savepoint_rollback_preserve_other_assignments() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = fixture::create(dir.path(), "incremental.ospider", 14);
    setup(&file);
    let cfg = cfg();
    segments::reassign(&file, &cfg, &Default::default()).unwrap();
    file.conn()
        .execute(
            "UPDATE urls SET address='https://site.test/blog/new' WHERE id=4",
            [],
        )
        .unwrap();
    segments::reassign_urls(&file, &cfg, &[os_model::UrlId(4)], &Default::default()).unwrap();
    assert_eq!(
        SegmentFilter::new("Blog")
            .bitmap(&file)
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        [1, 2, 4]
    );
    assert_eq!(SegmentFilter::new("Error").bitmap(&file).unwrap().len(), 1);
    let tx = file.conn().unchecked_transaction().unwrap();
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(segments::reassign(&file, &cfg, &cancel).is_err());
    tx.commit().unwrap();
    assert_eq!(SegmentFilter::new("Blog").bitmap(&file).unwrap().len(), 3);
    file.conn()
        .execute("DELETE FROM urls WHERE id=4", [])
        .unwrap();
    segments::reassign_urls(&file, &cfg, &[os_model::UrlId(4)], &Default::default()).unwrap();
    assert_eq!(SegmentFilter::new("Blog").bitmap(&file).unwrap().len(), 2);
}
// spec: CRAWL-134
#[test]
fn issue_operators_and_pending_analysis_use_the_common_issue_catalogue() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = fixture::create(dir.path(), "issues.ospider", 14);
    setup(&file);
    let mut cfg = cfg();
    cfg.segments.list = vec![segment(
        "Errors",
        vec![cond(
            "Issues",
            SegmentOp::HasIssue,
            "response-codes-internal-client-error-4xx",
        )],
    )];
    let a = segments::preview(&file, &cfg, &Default::default()).unwrap();
    assert_eq!(a.segments[0].iter().collect::<Vec<_>>(), [2]);
    cfg.segments.list[0].groups[0].conditions = vec![cond("Link Score", SegmentOp::IsEmpty, "")];
    let a = segments::preview(&file, &cfg, &Default::default()).unwrap();
    assert!(a.segments[0].is_empty());
    assert_eq!(a.pending.len(), 1);
}

// spec: UI-020 [P1]
#[test]
fn view_sessions_intersect_segment_bitmaps_and_rebuild_after_reassignment() {
    use os_analyze::views::{
        session::{Budget, RowsReq, SessionSpec, ViewSession},
        ColumnSelection,
    };
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = fixture::create(dir.path(), "view.ospider", 14);
    setup(&file);
    let cfg = cfg();
    segments::reassign(&file, &cfg, &Default::default()).unwrap();
    let mut session = ViewSession::open(
        &file,
        &cfg,
        SessionSpec {
            tab: "Internal".into(),
            filter: "All".into(),
            search: None,
            sort: Vec::new(),
            columns: ColumnSelection::Named(vec!["Address".into(), "Segment".into()]),
            restrict: vec![std::sync::Arc::new(SegmentFilter::new("Blog"))],
        },
        Budget::default(),
    )
    .unwrap();
    let rows = session
        .rows(
            &file,
            RowsReq {
                version: session.version(),
                offset: 0,
                limit: 50,
            },
        )
        .unwrap();
    assert_eq!(rows.total, 2);
    assert_eq!(rows.row_keys, vec![os_model::UrlId(1), os_model::UrlId(2)]);
    let mut reversed = cfg.clone();
    reversed.segments.list.reverse();
    segments::reassign(&file, &reversed, &Default::default()).unwrap();
    // Session consumers receive reset_seq through the change feed and rebuild their generation.
    session
        .apply_changes(&file, &[os_model::UrlId(1), os_model::UrlId(2)])
        .unwrap();
    let rows = session
        .rows(
            &file,
            RowsReq {
                version: session.version(),
                offset: 0,
                limit: 50,
            },
        )
        .unwrap();
    assert_eq!(rows.total, 1);
    assert_eq!(rows.row_keys, vec![os_model::UrlId(1)]);
}

// spec: UI-067 [P1]
#[test]
fn issue_metrics_count_distinct_urls_when_multiple_issues_match() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = fixture::create(dir.path(), "distinct.ospider", 14);
    fixture::insert(
        &file,
        &[fixture::row(1, "https://site.test/blog/a")
            .int("status_code", 200)
            .text("content_class", "HTML")
            .text("indexability", "Indexable")],
    );
    let cfg = cfg();
    segments::reassign(&file, &cfg, &Default::default()).unwrap();
    let rows = segments::statistics(&file, &cfg, &Default::default()).unwrap();
    assert_eq!(rows[0].issues["Issue"], 1);
    assert_eq!(rows[0].issues["High"], 1);
}
// spec: CRAWL-134
#[test]
fn headless_observer_assigns_new_urls_and_finishes_without_polling_feedback() {
    let dir = tempfile::tempdir().unwrap();
    let (path, file) = fixture::create(dir.path(), "observer.ospider", 14);
    setup(&file);
    file.set_config(&cfg()).unwrap();
    let observer = segments::observe(path, Default::default()).unwrap();
    let start = std::time::Instant::now();
    loop {
        if SegmentFilter::new("Blog").bitmap(&file).unwrap().len() == 2 {
            break;
        }
        assert!(start.elapsed().as_secs_f64() < 1.0);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let first = segments::live_snapshot(&file, 0).unwrap().unwrap().sequence;
    std::thread::sleep(std::time::Duration::from_millis(550));
    let next = segments::live_snapshot(&file, 0).unwrap().unwrap().sequence;
    assert_eq!(first, next);
    drop(observer);
    assert_eq!(
        segments::live_snapshot(&file, 0).unwrap().unwrap().sequence,
        next
    );
}
