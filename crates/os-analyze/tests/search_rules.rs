//! Lokale Store-Fixture fx-ui-basic, ohne Netz oder Engine-Erweiterungspunkte.
#[path = "framework_common.rs"]
mod fixture;
use fixture::{create, insert, row};
use os_analyze::search::{self, CompiledSearch, RuleNode, SearchCtx, SearchError};
use os_analyze::views::{self, ColumnSelection, SearchSpec, ViewQuery};
use os_analyze::{ColumnDef, ColumnKind, ColumnSql, FilterDef, FilterKind, TabDef};
use os_model::{CancelToken, CrawlConfig, Value};
use os_store::CrawlFile;
use std::time::{Duration, Instant};

fn tab() -> TabDef {
    let mut tab = os_analyze::defs::placeholder_tab(
        "Fixture",
        vec![FilterDef::placeholder("All", FilterKind::Live, &[])],
    );
    tab.columns = vec![
        ColumnDef::new("Address", "u.address", ColumnKind::Url, true),
        ColumnDef::new("Word Count", "u.word_count", ColumnKind::Int, true),
        ColumnDef::new("Status Code", "u.status_code", ColumnKind::Int, true),
        ColumnDef::new("Title", "u.title_1", ColumnKind::Text, false),
        ColumnDef::new(
            "Response Time",
            "u.response_time_ms",
            ColumnKind::Seconds3,
            true,
        ),
        ColumnDef {
            key: "Meta Robots",
            sql: ColumnSql::Multi {
                base: "Meta Robots",
                list_sql: "SELECT raw FROM directives WHERE url_id = ?1 ORDER BY position",
            },
            ..ColumnDef::new("Meta Robots", "", ColumnKind::Text, false)
        },
    ];
    tab
}
fn setup(file: &CrawlFile) {
    insert(
        file,
        &[
            row(1, "/Blog/Post-1")
                .int("word_count", 40)
                .int("status_code", 200)
                .text("title_1", "Straße")
                .int("response_time_ms", 512),
            row(2, "/blog/post-2")
                .int("word_count", 100)
                .int("status_code", 200),
            row(3, "/news/blog-news")
                .int("word_count", 200)
                .int("status_code", 404)
                .text("title_1", "ÄPFEL"),
            row(4, "/shop/shop/schuhe")
                .int("word_count", 300)
                .int("status_code", 200)
                .text("title_1", "κόσμος"),
            row(5, "/shop/schuhe/shop")
                .null("word_count")
                .int("status_code", 200),
        ],
    );
    file.conn().execute_batch("INSERT INTO directives(url_id,source,position,raw,tokens,applies,in_head) VALUES (1,'meta_robots',0,'index','index',1,1),(1,'meta_robots',1,'follow','follow',1,1),(2,'meta_robots',0,'noindex','noindex',1,1)").unwrap();
}
fn compiled(
    spec: &SearchSpec,
    tab: &TabDef,
    visible: &[&str],
) -> Result<CompiledSearch, SearchError> {
    search::compile_ctx(
        spec,
        &SearchCtx {
            tab,
            columns: &tab.columns,
            visible,
        },
    )
}
fn rules(text: &str, sensitive: bool) -> SearchSpec {
    SearchSpec::Rules {
        tree: search::parse(text).unwrap(),
        case_sensitive: sensitive,
    }
}
fn ids(file: &CrawlFile, spec: &SearchSpec) -> Vec<u32> {
    compiled(spec, &tab(), &["Address", "Word Count", "Status Code"])
        .unwrap()
        .matches_bitmap(
            file,
            Instant::now() + Duration::from_secs(2),
            &CancelToken::new(),
        )
        .unwrap()
        .iter()
        .collect()
}

// spec: UI-011 [P1]
#[test]
fn grammar_precedence_quotes_escapes_and_roundtrip() {
    for text in [
        r#"Address ~ "/blog/" AND "Word Count" < 200"#,
        r#"NOT (Address ^= '/shop' OR ("Title" = 'a "quote"' AND "Word Count" >= 1,5))"#,
        r#"Address REGEX "/([^/]+)/\1/""#,
        r#"Title IS EMPTY OR Title IS NOT EMPTY"#,
    ] {
        let tree = search::parse(text).unwrap();
        assert_eq!(search::parse(&search::format(&tree)).unwrap(), tree);
    }
    assert!(
        matches!(search::parse("Address = a OR Address = b AND Address = c").unwrap(), RuleNode::Or(nodes) if matches!(&nodes[1], RuleNode::And(_)))
    );
    assert_eq!(search::parse("Ä = x AND").unwrap_err().position, 9);
    assert_eq!(search::parse("Title = \"abc").unwrap_err().position, 8);
    assert!(search::parse("Address = x trailing").is_err());
    assert!(search::parse(&format!("{}Address = a{}", "(".repeat(65), ")".repeat(65))).is_err());
    assert!(search::parse("").is_err());
}

// spec: UI-011 [M2]
#[test]
fn address_substring_and_unicode_case_folding() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = create(dir.path(), "fx-ui-basic.ospider", 42);
    setup(&file);
    assert_eq!(ids(&file, &SearchSpec::Simple("blog".into())), [1, 2, 3]);
    assert_eq!(
        ids(&file, &SearchSpec::Simple(String::new())),
        [1, 2, 3, 4, 5]
    );
    assert_eq!(ids(&file, &rules(r#"Address ~ "Blog""#, true)), [1]);
    assert_eq!(ids(&file, &rules(r#"Title = "STRASSE""#, false)), [1]);
    assert_eq!(ids(&file, &rules(r#"Title ~ "äpf""#, false)), [3]);
    assert_eq!(ids(&file, &rules(r#"Title = "ΚΌΣΜΟΣ""#, false)), [4]);
    assert!(ids(&file, &rules(r#"Title = "STRASSE""#, true)).is_empty());
}

// spec: UI-011 [P1]
#[test]
fn operators_types_empty_cells_and_visible_columns() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = create(dir.path(), "ops.ospider", 43);
    setup(&file);
    for (query, expected) in [
        ("Address !~ shop", vec![1, 2, 3]),
        ("Address ^= /blog", vec![1, 2]),
        ("Address $= shop", vec![5]),
        ("\"Word Count\" < 50 OR \"Status Code\" = 404", vec![1, 3]),
        ("\"Word Count\" != 100", vec![1, 3, 4, 5]),
        ("\"Word Count\" IS EMPTY", vec![5]),
        ("\"Word Count\" IS NOT EMPTY", vec![1, 2, 3, 4]),
        ("\"Word Count\" >= 200", vec![3, 4]),
        ("\"Word Count\" <= 100", vec![1, 2]),
        ("NOT (\"Word Count\" > 100)", vec![1, 2, 5]),
        ("\"Response Time\" = 0,512", vec![1]),
        ("* ~ blog", vec![1, 2, 3]),
        ("* !~ blog", vec![4, 5]),
        ("Title IS EMPTY", vec![2, 5]),
    ] {
        assert_eq!(ids(&file, &rules(query, false)), expected, "{query}");
    }
    for query in [
        "Address > 3",
        "\"Word Count\" ~ 3",
        "\"Word Count\" = nope",
        "\"Word Count\" = NaN",
        "Unknown = a",
    ] {
        assert!(
            compiled(&rules(query, false), &tab(), &["Address"]).is_err(),
            "{query}"
        );
    }
    let spec = rules("* ~ STRASSE", false);
    let tab = tab();
    let c = compiled(&spec, &tab, &["Title"]).unwrap();
    assert_eq!(
        c.matches_bitmap(
            &file,
            Instant::now() + Duration::from_secs(2),
            &CancelToken::new()
        )
        .unwrap()
        .iter()
        .collect::<Vec<_>>(),
        [1]
    );
}

// spec: UI-011 [P1]
#[test]
fn regex_backreferences_lookarounds_and_negation() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = create(dir.path(), "regex.ospider", 44);
    setup(&file);
    assert_eq!(
        ids(&file, &rules(r#"Address REGEX "/([^/]+)/\1/""#, false)),
        [4]
    );
    assert_eq!(
        ids(&file, &rules(r#"Address REGEX "(?<=/)blog(?=/)""#, true)),
        [2]
    );
    assert_eq!(
        ids(&file, &rules(r#"Address NOTREGEX "^.*/blog/""#, false)),
        [3, 4, 5]
    );
    assert!(search::validate_regex("(").is_err());
    assert!(compiled(&rules(r#"Address REGEX "[""#, false), &tab(), &["Address"]).is_err());
}

// spec: UI-011 [P1]
#[test]
fn regex_step_limit_and_deadline_abort_the_whole_search() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = create(dir.path(), "bounded.ospider", 45);
    insert(
        &file,
        &[row(1, "ok"), row(2, &format!("{}!", "a".repeat(100)))],
    );
    let c = compiled(
        &rules(r#"Address REGEX "^(a|aa)+\1$""#, true),
        &tab(),
        &["Address"],
    )
    .unwrap();
    let bounded = c.matches_bitmap(
        &file,
        Instant::now() + Duration::from_secs(2),
        &CancelToken::new(),
    );
    assert!(
        matches!(bounded, Err(SearchError::StepLimit)),
        "{bounded:?}"
    );
    assert!(matches!(
        c.matches_bitmap(&file, Instant::now(), &CancelToken::new()),
        Err(SearchError::Deadline)
    ));
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(matches!(
        c.matches_bitmap(&file, Instant::now() + Duration::from_secs(2), &cancel),
        Err(SearchError::Cancelled)
    ));
    assert_eq!(search::search_budget(10, true), Duration::from_secs(2));
    assert_eq!(
        search::search_budget(1_000_000, true),
        Duration::from_secs(3)
    );
    assert_eq!(
        search::search_budget(5_000_000, true),
        Duration::from_secs(10)
    );
    assert_eq!(
        search::search_budget(5_000_000, false),
        Duration::from_secs(3)
    );
}

// spec: UI-011 [P1]
#[test]
fn multi_columns_use_the_same_numbered_values_as_views() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = create(dir.path(), "multi.ospider", 46);
    setup(&file);
    assert_eq!(ids(&file, &rules(r#""Meta Robots" = follow"#, false)), [1]);
    assert_eq!(
        ids(&file, &rules(r#""Meta Robots 2" = follow"#, false)),
        [1]
    );
    assert_eq!(
        ids(&file, &rules(r#""Meta Robots" != follow"#, false)),
        [2, 3, 4, 5]
    );
    assert_eq!(
        ids(&file, &rules(r#""Meta Robots 2" IS EMPTY"#, false)),
        [2, 3, 4, 5]
    );
}

// spec: OUT-020 [M2]
#[test]
fn view_and_stream_share_rules_filter_sort_and_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = create(dir.path(), "export.ospider", 47);
    setup(&file);
    let mut tab = tab();
    tab.filters.push(FilterDef {
        predicate: "u.status_code = 200",
        ..FilterDef::placeholder("Success", FilterKind::Live, &[])
    });
    let tabs = [tab];
    let cfg = CrawlConfig::default();
    let mut q = ViewQuery::new("Fixture", "Success");
    q.columns = ColumnSelection::Named(vec!["Address".into(), "Word Count".into()]);
    q.search = Some(rules(
        r#"Address REGEX "(?i)blog" AND "Word Count" < 200"#,
        false,
    ));
    q.sort = vec![os_analyze::SortKey {
        column: "Word Count".into(),
        descending: true,
    }];
    let page = views::query_view_in(&tabs, &file, &cfg, &q).unwrap();
    assert_eq!(
        page.row_keys.iter().map(|id| id.0).collect::<Vec<_>>(),
        [2, 1]
    );
    let mut exported = Vec::new();
    let columns = views::stream_view_in(&tabs, &file, &cfg, &q, &mut |row| {
        exported.push(row.to_vec());
        Ok(())
    })
    .unwrap();
    assert_eq!(columns, page.columns);
    assert_eq!(exported, page.rows);
    assert_eq!(exported[0][1], Value::Int(100));
    let temp_count: i64 = file
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM sqlite_temp_master WHERE name LIKE 'os_search_%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(temp_count, 0);
    q.search = Some(rules(r#"Address REGEX "[""#, false));
    let mut emitted = 0;
    assert!(views::stream_view_in(&tabs, &file, &cfg, &q, &mut |_| {
        emitted += 1;
        Ok(())
    })
    .is_err());
    assert_eq!(emitted, 0);
}

// spec: UI-011 [P1]
#[test]
fn cursors_reject_different_rules_and_case_flags() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = create(dir.path(), "cursor.ospider", 48);
    setup(&file);
    let tabs = [tab()];
    let cfg = CrawlConfig::default();
    let mut q = ViewQuery::new("Fixture", "All");
    q.limit = 1;
    q.search = Some(rules("Address ~ blog", false));
    let first = views::query_view_in(&tabs, &file, &cfg, &q).unwrap();
    q.start = views::PageStart::Cursor(first.next_cursor.unwrap());
    assert_eq!(
        views::query_view_in(&tabs, &file, &cfg, &q)
            .unwrap()
            .row_keys[0]
            .0,
        2
    );
    q.search = Some(rules("Address ~ blog", true));
    assert!(matches!(
        views::query_view_in(&tabs, &file, &cfg, &q),
        Err(os_analyze::error::ViewError::ForeignCursor)
    ));
    q.search = Some(rules("Address ~ shop", false));
    assert!(matches!(
        views::query_view_in(&tabs, &file, &cfg, &q),
        Err(os_analyze::error::ViewError::ForeignCursor)
    ));
}

// spec: UI-011 [P1]
#[test]
fn deadline_and_cancellation_interrupt_a_running_sqlite_step() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = create(dir.path(), "interrupt.ospider", 49);
    insert(&file, &[row(1, "/a")]);
    let mut tab = tab();
    tab.columns.push(ColumnDef::new("Slow", "(WITH RECURSIVE numbers(n) AS (VALUES(0) UNION ALL SELECT n+1 FROM numbers WHERE n<100000000) SELECT MAX(n) FROM numbers)", ColumnKind::Int, false));
    let c = compiled(&rules("Slow > 0", false), &tab, &["Address"]).unwrap();
    assert!(matches!(
        c.matches_bitmap(
            &file,
            Instant::now() + Duration::from_millis(20),
            &CancelToken::new()
        ),
        Err(SearchError::Deadline)
    ));
    let token = CancelToken::new();
    let cancel = token.clone();
    let worker = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(20));
        cancel.cancel();
    });
    let result = c.matches_bitmap(&file, Instant::now() + Duration::from_secs(2), &token);
    worker.join().unwrap();
    assert!(matches!(result, Err(SearchError::Cancelled)));
    assert_eq!(
        file.conn()
            .query_row("SELECT COUNT(*) FROM urls", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
}

// spec: UI-011 [M2]
#[test]
fn parallel_address_search_spans_batches_and_uses_unicode_folding() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = create(dir.path(), "parallel.ospider", 50);
    file.conn().execute_batch("WITH RECURSIVE numbers(n) AS (VALUES(1) UNION ALL SELECT n+1 FROM numbers WHERE n<20000) INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,folder_depth,url_length) SELECT n,'https://site.test/'||n,'/Äpfel/'||n,'https://site.test',1,'Done','Link',0,10 FROM numbers").unwrap();
    let c = compiled(&SearchSpec::Simple("äPFEL".into()), &tab(), &["Address"]).unwrap();
    let found = c
        .matches_bitmap(
            &file,
            Instant::now() + Duration::from_secs(2),
            &CancelToken::new(),
        )
        .unwrap();
    assert_eq!(found.len(), 20000);
    assert!(found.contains(1));
    assert!(found.contains(20000));
    // Auch der SQL-View-Pfad promoviert die einfache Suche zur Unicode-Bitmap.
    let mut q = ViewQuery::new("Fixture", "All");
    q.search = Some(SearchSpec::Simple("äPFEL".into()));
    assert_eq!(
        views::query_view_in(&[tab()], &file, &CrawlConfig::default(), &q)
            .unwrap()
            .total,
        20000
    );
}

// spec: OUT-020 [M2]
#[test]
fn all_export_columns_do_not_change_the_visible_search_columns() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = create(dir.path(), "visible.ospider", 51);
    setup(&file);
    let tabs = [tab()];
    let cfg = CrawlConfig::default();
    let mut q = ViewQuery::new("Fixture", "All");
    q.search = Some(rules("* ~ STRASSE", false));
    q.search_visible = Some(vec!["Title".into()]);
    q.columns = ColumnSelection::All;
    assert_eq!(
        views::query_view_in(&tabs, &file, &cfg, &q).unwrap().total,
        1
    );
    let mut exported = 0;
    views::stream_view_in(&tabs, &file, &cfg, &q, &mut |_| {
        exported += 1;
        Ok(())
    })
    .unwrap();
    assert_eq!(exported, 1);
    q.search_visible = Some(vec!["Address".into()]);
    assert_eq!(
        views::query_view_in(&tabs, &file, &cfg, &q).unwrap().total,
        0
    );
}

// spec: UI-011 [P1]
#[test]
fn standalone_compiler_and_sql_guard_work_read_only_and_inside_transactions() {
    let dir = tempfile::tempdir().unwrap();
    let (path, file) = create(dir.path(), "guard.ospider", 52);
    setup(&file);
    let core = search::compile(&search::parse("Address ~ blog").unwrap(), false).unwrap();
    assert_eq!(
        core.matches_bitmap(
            &file,
            Instant::now() + Duration::from_secs(2),
            &CancelToken::new()
        )
        .unwrap()
        .len(),
        3
    );
    let tx = file.conn().unchecked_transaction().unwrap();
    file.conn()
        .execute("UPDATE urls SET word_count=41 WHERE id=1", [])
        .unwrap();
    {
        let guard = core
            .prepare(
                &file,
                Instant::now() + Duration::from_secs(2),
                &CancelToken::new(),
            )
            .unwrap();
        let count: i64 = file
            .conn()
            .query_row(
                &format!("SELECT COUNT(*) FROM urls u WHERE {}", guard.sql()),
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 3);
    }
    assert!(!file.conn().is_autocommit());
    assert_eq!(
        file.conn()
            .query_row("SELECT word_count FROM urls WHERE id=1", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        41
    );
    tx.rollback().unwrap();
    drop(file);
    let file = CrawlFile::open(&path, os_store::OpenMode::ReadOnly).unwrap();
    let mut q = ViewQuery::new("Fixture", "All");
    q.search = Some(SearchSpec::Simple("blog".into()));
    assert_eq!(
        views::query_view_in(&[tab()], &file, &CrawlConfig::default(), &q)
            .unwrap()
            .total,
        3
    );
    assert_eq!(
        file.conn()
            .query_row("SELECT word_count FROM urls WHERE id=1", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        40
    );
}
