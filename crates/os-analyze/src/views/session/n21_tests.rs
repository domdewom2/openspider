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

// spec: UI-141
// spec: UI-142
#[test]
fn all_scan_prepares_class_bitmaps_with_the_same_visible_population() {
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
