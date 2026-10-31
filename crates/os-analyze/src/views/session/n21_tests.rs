use super::*;
use os_model::{CrawlMode, Timestamp};
use os_store::{CrawlInit, StorageProfile};

fn spec(filter: &str) -> SessionSpec {
    SessionSpec {
        tab: "Internal".into(),
        filter: filter.into(),
        search: None,
        sort: vec![],
        columns: ColumnSelection::Named(vec!["Address".into()]),
        restrict: vec![],
    }
}

fn fixture() -> (tempfile::TempDir, CrawlConfig, CrawlFile) {
    let dir = tempfile::tempdir().unwrap();
    let cfg = CrawlConfig::default();
    let file = CrawlFile::create(
        &dir.path().join("n21.ospider"),
        &CrawlInit {
            crawl_id: "00000000-0000-0000-0000-000000000001".parse().unwrap(),
            mode: CrawlMode::Spider,
            start_url: Some("https://site.test/".into()),
            list_source: None,
            project: "n21".into(),
            name: "n21".into(),
            config: cfg.clone(),
            created_at: Timestamp::UNIX_EPOCH,
        },
        StorageProfile::Balanced,
    )
    .unwrap();
    file.conn()
        .execute_batch(include_str!(
            "../../../../../apps/desktop/src/ipc/__fixtures__/k06/fx-ui-basic/scenario.sql"
        ))
        .unwrap();
    file.conn()
        .execute_batch("UPDATE urls SET content_class='HTML' WHERE id IN (6,7)")
        .unwrap();
    (dir, cfg, file)
}

// spec: UI-141
// spec: UI-142
#[test]
fn all_scan_prepares_class_bitmaps_with_the_same_visible_population() {
    let (_dir, cfg, file) = fixture();
    let mut view = ViewSession::open(&file, &cfg, spec("All"), Budget::default()).unwrap();
    assert_eq!(view.prepare(&file, &BuildControl::default()).unwrap(), 6);
    let tabs = crate::tabs::configured_for_file(&cfg, &file);
    let html = build::build(&tabs, &ViewQuery::new("Internal", "HTML"), &cfg).unwrap();
    let key = filter_key(&html, true);
    let expected = vec![1, 2, 3, 4, 8];
    {
        let cache = view.cache.lock().unwrap();
        assert_eq!(cache.filters[&key].ids.iter().collect::<Vec<_>>(), expected);
        let images = build::build(&tabs, &ViewQuery::new("Internal", "Images"), &cfg).unwrap();
        assert_eq!(
            cache.filters[&filter_key(&images, true)]
                .ids
                .iter()
                .collect::<Vec<_>>(),
            vec![5]
        );
        let css = build::build(&tabs, &ViewQuery::new("Internal", "CSS"), &cfg).unwrap();
        assert!(cache.filters[&filter_key(&css, true)].ids.is_empty());
    }
    view.update(&file, &cfg, spec("HTML")).unwrap();
    let rows = view
        .rows(
            &file,
            RowsReq {
                version: view.version(),
                offset: 0,
                limit: 200,
            },
        )
        .unwrap();
    assert_eq!(
        rows.row_keys.iter().map(|id| id.0).collect::<Vec<_>>(),
        expected.into_iter().map(i64::from).collect::<Vec<_>>()
    );
}

// spec: UI-141
// spec: UI-142
// spec: UI-147
#[test]
fn shared_memberships_cover_external_and_denominator_and_expire_with_views() {
    let (_dir, cfg, file) = fixture();
    let internal = ViewSession::open(&file, &cfg, spec("All"), Budget::default()).unwrap();
    internal.prepare(&file, &BuildControl::default()).unwrap();
    let mut external_spec = spec("HTML");
    external_spec.tab = "External".into();
    let mut external = ViewSession::open(&file, &cfg, external_spec, Budget::default()).unwrap();
    external.prepare(&file, &BuildControl::default()).unwrap();
    let weak = {
        let a = internal.cache.lock().unwrap();
        let b = external.cache.lock().unwrap();
        let facts = a.membership.as_ref().unwrap();
        assert!(Arc::ptr_eq(facts, b.membership.as_ref().unwrap()));
        assert_eq!(
            facts.html_2xx,
            super::super::internal_html_2xx_count(file.conn()).unwrap()
        );
        Arc::downgrade(facts)
    };
    let query = ViewQuery::new("External", "HTML");
    let expected = super::super::query_view(&file, &cfg, &query).unwrap();
    let rows = external
        .rows(
            &file,
            RowsReq {
                version: external.version(),
                offset: 0,
                limit: 200,
            },
        )
        .unwrap();
    assert_eq!(rows.row_keys, expected.row_keys);
    // A reset without a sequence increment must not reuse the old memberships.
    file.conn()
        .execute_batch("UPDATE urls SET in_tables=0 WHERE is_internal=0")
        .unwrap();
    file.set_meta("reset_seq", "123").unwrap();
    external
        .update(&file, &cfg, {
            let mut s = spec("All");
            s.tab = "External".into();
            s
        })
        .unwrap();
    assert_eq!(
        external.prepare(&file, &BuildControl::default()).unwrap(),
        0
    );
    assert!(!Arc::ptr_eq(
        internal.cache.lock().unwrap().membership.as_ref().unwrap(),
        external.cache.lock().unwrap().membership.as_ref().unwrap()
    ));
    internal.cache.lock().unwrap().release();
    assert!(
        weak.upgrade().is_none(),
        "weak registry must not extend the lease"
    );
}

// spec: UI-141
// spec: UI-142
#[test]
fn live_changes_repair_shared_memberships_and_hidden_html_denominator() {
    let (_dir, cfg, file) = fixture();
    let mut view = ViewSession::open(&file, &cfg, spec("All"), Budget::default()).unwrap();
    view.prepare(&file, &BuildControl::default()).unwrap();
    file.conn().execute_batch("UPDATE urls SET is_internal=0, in_tables=0 WHERE id=1; DELETE FROM urls WHERE id=2; UPDATE urls SET content_class='Images' WHERE id=7").unwrap();
    file.set_meta("change_seq", "2").unwrap();
    view.apply_changes(&file, &[UrlId(1), UrlId(2), UrlId(7), UrlId(1)])
        .unwrap();
    let cache = view.cache.lock().unwrap();
    let facts = cache.membership.as_ref().unwrap();
    assert_eq!(
        facts.html_2xx,
        super::super::internal_html_2xx_count(file.conn()).unwrap()
    );
    let tabs = crate::tabs::configured_for_file(&cfg, &file);
    for (tab, filter) in [
        ("Internal", "All"),
        ("External", "HTML"),
        ("Internal", "Images"),
    ] {
        let q = ViewQuery::new(tab, filter);
        let built = build::build(&tabs, &q, &cfg).unwrap();
        let expected = super::super::query_view(&file, &cfg, &q).unwrap();
        assert_eq!(
            facts
                .bitmap(&built)
                .unwrap()
                .iter()
                .map(|id| UrlId(i64::from(id)))
                .collect::<Vec<_>>(),
            expected.row_keys
        );
    }
}

fn tab_spec(tab: &str, filter: &str) -> SessionSpec {
    let mut s = spec(filter);
    s.tab = tab.into();
    s
}

fn all_rows(view: &ViewSession, file: &CrawlFile) -> Vec<UrlId> {
    view.rows(
        file,
        RowsReq {
            version: view.version(),
            offset: 0,
            limit: 500,
        },
    )
    .unwrap()
    .row_keys
}

// spec: UI-141
// spec: UI-147
#[test]
fn response_codes_and_parallel_passes_match_the_sql_population() {
    let (_dir, cfg, file) = fixture();
    file.set_meta("state", "Completed").unwrap();
    // Hidden/external rows without a status must stay outside Response Codes.
    file.conn()
        .execute_batch("UPDATE urls SET status_code=NULL WHERE id=7")
        .unwrap();
    for parallel in [false, true] {
        crate::views::FORCE_PARALLEL.with(|f| f.set(parallel));
        // Each round scans once; later views of the round decode the stored pass.
        os_store::view_cache::clear(&file).unwrap();
        for (tab, filter) in [
            ("Response Codes", "All"),
            ("Internal", "All"),
            ("Internal", "HTML"),
            ("External", "All"),
            ("External", "HTML"),
        ] {
            let view =
                ViewSession::open(&file, &cfg, tab_spec(tab, filter), Budget::default()).unwrap();
            view.prepare(&file, &BuildControl::default()).unwrap();
            assert!(
                view.cache.lock().unwrap().membership.is_some(),
                "{tab}/{filter} must use the shared pass"
            );
            let expected = super::super::query_view(&file, &cfg, &ViewQuery::new(tab, filter))
                .unwrap()
                .row_keys;
            assert_eq!(all_rows(&view, &file), expected, "{tab}/{filter}");
            view.cache.lock().unwrap().release();
        }
    }
    crate::views::FORCE_PARALLEL.with(|f| f.set(false));
    assert_eq!(crate::views::PARALLEL_PASSES.with(|n| n.get()), 1);
}

// spec: UI-142
// spec: UI-147
#[test]
fn persisted_memberships_replace_the_scan_of_a_reopened_saved_crawl() {
    let (_dir, cfg, file) = fixture();
    file.set_meta("state", "Completed").unwrap();
    let path = file.path().to_path_buf();
    let expected = {
        let view = ViewSession::open(&file, &cfg, spec("All"), Budget::default()).unwrap();
        view.prepare(&file, &BuildControl::default()).unwrap();
        // The writable test connection stores the pass right after the build.
        assert!(!view.shared_pending());
        let rows = all_rows(&view, &file);
        view.cache.lock().unwrap().release();
        rows
    };
    drop(file);
    let stored = CrawlFile::open(&path, os_store::OpenMode::ReadOnly).unwrap();
    let view = ViewSession::open(&stored, &cfg, spec("All"), Budget::default()).unwrap();
    view.prepare(&stored, &BuildControl::default()).unwrap();
    assert!(!view.shared_pending(), "loaded from view_cache");
    assert_eq!(all_rows(&view, &stored), expected);
    view.cache.lock().unwrap().release();
    drop(view);

    // A new data version ignores the stored pass; a read-only host leaves it pending.
    let writer = CrawlFile::open(&path, os_store::OpenMode::ReadWrite).unwrap();
    writer
        .conn()
        .execute_batch("UPDATE urls SET is_internal=0 WHERE id=2")
        .unwrap();
    writer.set_meta("change_seq", "99").unwrap();
    drop(writer);
    let view = ViewSession::open(&stored, &cfg, spec("All"), Budget::default()).unwrap();
    view.prepare(&stored, &BuildControl::default()).unwrap();
    assert!(view.shared_pending());
    assert!(!all_rows(&view, &stored).contains(&UrlId(2)));
    // The writer persists exactly the pass the read-only view prepared.
    let writer = CrawlFile::open(&path, os_store::OpenMode::ReadWrite).unwrap();
    assert!(persist_shared(&writer).unwrap());
    assert!(!view.shared_pending());
    assert!(!persist_shared(&writer).unwrap());
}
