// Tests share the process-wide LRU pool; isolate budget and cache assertions.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
#[path = "framework_common.rs"]
mod common;

use os_analyze::views::session::{Budget, BuildControl, RowsReq, SessionSpec, ViewSession};
use os_analyze::views::{self, ColumnSelection, PageStart, ViewQuery};
use os_analyze::{SortKey, ViewError};
use os_model::{CrawlConfig, UrlId};
use os_store::CrawlFile;
use roaring::RoaringBitmap;
use std::{sync::Arc, time::Duration};

fn fixture() -> (tempfile::TempDir, CrawlFile) {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = common::create(dir.path(), "session.ospider", 91);
    common::insert(
        &file,
        &[
            common::row(1, "https://s.test/z")
                .text("title_1", "Apple")
                .int("status_code", 200),
            common::row(2, "https://s.test/a")
                .text("title_1", "apple")
                .int("status_code", 200),
            common::row(3, "https://s.test/c")
                .text("title_1", "Ärger")
                .int("status_code", 201),
            common::row(4, "https://s.test/d")
                .text("title_1", "Zebra")
                .int("status_code", 404),
            common::row(5, "https://s.test/e")
                .null("title_1")
                .int("status_code", 200),
            common::row(6, "https://s.test/f")
                .text("title_1", "")
                .int("status_code", 200),
        ],
    );
    file.conn()
        .execute(
            "UPDATE urls SET content_class = 'HTML', title_count = 1",
            [],
        )
        .unwrap();
    common::set_change_seq(&file, 7);
    (dir, file)
}
fn spec(column: &str, descending: bool) -> SessionSpec {
    SessionSpec {
        tab: "Internal".into(),
        filter: "All".into(),
        search: None,
        sort: vec![SortKey {
            column: column.into(),
            descending,
        }],
        columns: ColumnSelection::Named(vec![
            "Address".into(),
            "Status Code".into(),
            "Title 1".into(),
        ]),
        restrict: vec![],
    }
}
fn window(s: &ViewSession, f: &CrawlFile) -> os_analyze::views::session::Rows {
    s.rows(
        f,
        RowsReq {
            version: s.version(),
            offset: 0,
            limit: 500,
        },
    )
    .unwrap()
}

// spec: UI-142
#[test]
fn lazy_open_windows_locate_and_generation_are_independent_of_writer_sequence() {
    let _serial = SERIAL.lock().unwrap();
    let (_dir, file) = fixture();
    let cfg = CrawlConfig::default();
    let mut s = ViewSession::open(&file, &cfg, spec("Address", false), Budget::default()).unwrap();
    assert_eq!(s.memory_bytes(), 0);
    assert_eq!(s.built_seq(), None);
    let w = window(&s, &file);
    assert_eq!(w.total, 6);
    assert_eq!(w.row_keys[0], UrlId(2));
    assert_eq!(s.built_seq(), Some(7));
    for (pos, id) in w.row_keys.iter().enumerate() {
        assert_eq!(s.locate(&file, *id).unwrap(), Some(pos as u64));
    }
    assert_eq!(s.locate(&file, UrlId(-1)).unwrap(), None);
    assert_eq!(s.locate(&file, UrlId(99)).unwrap(), None);
    let tail = s
        .rows(
            &file,
            RowsReq {
                version: s.version(),
                offset: 4,
                limit: 200,
            },
        )
        .unwrap();
    assert_eq!(tail.row_keys, w.row_keys[4..]);
    let empty = s
        .rows(
            &file,
            RowsReq {
                version: s.version(),
                offset: u64::MAX,
                limit: 200,
            },
        )
        .unwrap();
    assert!(empty.rows.is_empty());
    assert_eq!(empty.columns, w.columns);
    assert!(matches!(
        s.rows(
            &file,
            RowsReq {
                version: 1,
                offset: 0,
                limit: 501
            }
        ),
        Err(ViewError::InvalidLimit(501))
    ));
    s.update(&file, &cfg, spec("Address", true)).unwrap();
    assert_eq!(s.version(), 2);
    assert_eq!(s.built_seq(), Some(7));
    assert!(matches!(
        s.rows(
            &file,
            RowsReq {
                version: 1,
                offset: 0,
                limit: 20
            }
        ),
        Err(ViewError::ForeignCursor)
    ));
}

// spec: UI-141
#[test]
fn permutations_match_sql_for_text_numeric_nulls_and_multiple_keys() {
    let _serial = SERIAL.lock().unwrap();
    let (_dir, file) = fixture();
    let cfg = CrawlConfig::default();
    let mut s = ViewSession::open(&file, &cfg, spec("Title 1", false), Budget::default()).unwrap();
    for column in ["Title 1", "Status Code", "Address"] {
        for descending in [false, true] {
            let specification = spec(column, descending);
            let mut q = ViewQuery::new("Internal", "All");
            q.columns = specification.columns.clone();
            q.sort = specification.sort.clone();
            q.limit = 500;
            let expected = views::query_view(&file, &cfg, &q).unwrap();
            s.update(&file, &cfg, specification).unwrap();
            let actual = window(&s, &file);
            assert_eq!(actual.row_keys, expected.row_keys, "{column} {descending}");
            assert_eq!(actual.rows, expected.rows);
        }
    }
    let mut specification = spec("Status Code", true);
    specification.sort.push(SortKey {
        column: "Address".into(),
        descending: true,
    });
    let mut q = ViewQuery::new("Internal", "All");
    q.sort = specification.sort.clone();
    q.columns = specification.columns.clone();
    s.update(&file, &cfg, specification).unwrap();
    assert_eq!(
        window(&s, &file).row_keys,
        views::query_view(&file, &cfg, &q).unwrap().row_keys
    );
}
struct Only(Vec<u32>);
impl os_analyze::search::FilterSource for Only {
    fn key(&self) -> String {
        "fixture:only".into()
    }
    fn bitmap(&self, _: &CrawlFile) -> Result<RoaringBitmap, ViewError> {
        Ok(self.0.iter().copied().collect())
    }
}

// spec: UI-141
#[test]
fn bitmap_restrictions_intersect_and_cancelled_replacement_keeps_old_view() {
    let _serial = SERIAL.lock().unwrap();
    let (_dir, file) = fixture();
    let cfg = CrawlConfig::default();
    let mut s = ViewSession::open(&file, &cfg, spec("Address", false), Budget::default()).unwrap();
    let old = window(&s, &file);
    let mut next = spec("Address", true);
    next.restrict = vec![Arc::new(Only(vec![1, 2, 3])), Arc::new(Only(vec![2, 3, 4]))];
    let control = BuildControl::default();
    control.cancel.cancel();
    assert!(s.update_controlled(&file, &cfg, next, &control).is_err());
    assert_eq!(window(&s, &file), old);
    let mut next = spec("Address", false);
    next.restrict = vec![Arc::new(Only(vec![1, 2, 3])), Arc::new(Only(vec![2, 3, 4]))];
    let control = BuildControl::default();
    s.update_controlled(&file, &cfg, next, &control).unwrap();
    assert_eq!(control.cancel.progress(), 1.0);
    assert_eq!(window(&s, &file).row_keys, vec![UrlId(2), UrlId(3)]);
}

// spec: UI-142
#[test]
fn changes_repair_membership_sort_order_and_invalidate_deleted_ids() {
    let _serial = SERIAL.lock().unwrap();
    let (_dir, file) = fixture();
    let cfg = CrawlConfig::default();
    let mut next = spec("Address", false);
    next.search = Some(views::SearchSpec::Simple("/a".into()));
    let mut s = ViewSession::open(&file, &cfg, next, Budget::default()).unwrap();
    assert_eq!(window(&s, &file).row_keys, vec![UrlId(2)]);
    file.conn()
        .execute(
            "UPDATE urls SET address = 'https://s.test/another' WHERE id = 3",
            [],
        )
        .unwrap();
    file.conn()
        .execute("DELETE FROM urls WHERE id = 2", [])
        .unwrap();
    common::set_change_seq(&file, 8);
    s.apply_changes(&file, &[UrlId(2), UrlId(3)]).unwrap();
    assert_eq!(window(&s, &file).row_keys, vec![UrlId(3)]);
    assert_eq!(s.built_seq(), Some(8));
    assert_eq!(s.version(), 2);
    s.apply_changes(&file, &[]).unwrap();
    assert_eq!(s.version(), 2);
}

// spec: UI-142
#[test]
fn cursor_is_bound_to_session_generation_and_offset_adapter() {
    let _serial = SERIAL.lock().unwrap();
    let (_dir, file) = fixture();
    let cfg = CrawlConfig::default();
    let mut s = ViewSession::open(&file, &cfg, spec("Address", false), Budget::default()).unwrap();
    let (first, next) = s.page(&file, &PageStart::Offset(0), 2).unwrap();
    assert_eq!(first.rows.len(), 2);
    let cursor = PageStart::Cursor(next.unwrap());
    let (second, _) = s.page(&file, &cursor, 2).unwrap();
    assert_eq!(second.row_keys, window(&s, &file).row_keys[2..4]);
    let other = ViewSession::open(&file, &cfg, spec("Address", false), Budget::default()).unwrap();
    assert!(other.page(&file, &cursor, 2).is_err());
    s.update(&file, &cfg, spec("Address", true)).unwrap();
    assert!(s.page(&file, &cursor, 2).is_err());
}

// spec: UI-142
#[test]
fn budget_eviction_rebuilds_and_zero_idle_releases_without_changing_version() {
    let _serial = SERIAL.lock().unwrap();
    let (_dir, file) = fixture();
    let cfg = CrawlConfig::default();
    let s = ViewSession::open(
        &file,
        &cfg,
        spec("Address", false),
        Budget {
            max_bytes: 0,
            idle_release: Duration::from_secs(60),
        },
    )
    .unwrap();
    let first = window(&s, &file);
    assert_eq!(s.memory_bytes(), 0);
    assert_eq!(window(&s, &file), first);
    let idle = ViewSession::open(
        &file,
        &cfg,
        spec("Address", false),
        Budget {
            max_bytes: u64::MAX,
            idle_release: Duration::ZERO,
        },
    )
    .unwrap();
    assert_eq!(window(&idle, &file), first);
    assert_eq!(idle.memory_bytes(), 0);
}

// spec: UI-141
#[test]
fn persistent_cache_is_disposable_and_validates_sequence_config_reset_and_step_stamp() {
    let _serial = SERIAL.lock().unwrap();
    let (_dir, file) = fixture();
    let cfg = CrawlConfig::default();
    let run = |cfg: &CrawlConfig| {
        let s =
            ViewSession::open(&file, cfg, spec("Status Code", false), Budget::default()).unwrap();
        window(&s, &file)
    };
    let original = run(&cfg);
    let before: i64 = file
        .conn()
        .query_row("SELECT COUNT(*) FROM view_cache", [], |r| r.get(0))
        .unwrap();
    assert_eq!(before, 1);
    assert_eq!(run(&cfg), original);
    file.conn()
        .execute("UPDATE view_cache SET data = x'ff'", [])
        .unwrap();
    assert_eq!(run(&cfg), original, "corrupt blob is reconstructed");
    file.conn()
        .execute("DELETE FROM urls WHERE id = 1", [])
        .unwrap();
    common::set_change_seq(&file, 8);
    assert_eq!(run(&cfg).total, 5, "old writer sequence is rejected");
    file.conn()
        .execute("DELETE FROM urls WHERE id = 2", [])
        .unwrap();
    file.set_meta("reset_seq", "9").unwrap();
    assert_eq!(
        run(&cfg).total,
        4,
        "reset invalidates even a matching writer sequence"
    );
    common::set_change_seq(&file, 9);
    file.set_meta("reset_seq", "0").unwrap();
    run(&cfg);
    file.conn()
        .execute("DELETE FROM urls WHERE id = 3", [])
        .unwrap();
    file.conn().execute("INSERT INTO analysis_steps (step,state,finished_at,config_hash) VALUES ('Links','Completed','2026-10-17T00:00:00Z','new')", []).unwrap();
    assert_eq!(run(&cfg).total, 3, "analysis stamp invalidates the bitmap");
    let mut changed = cfg.clone();
    changed.issues.ignore_non_indexable = !cfg.issues.ignore_non_indexable;
    run(&changed);
    let after: i64 = file
        .conn()
        .query_row("SELECT COUNT(*) FROM view_cache", [], |r| r.get(0))
        .unwrap();
    assert!(after > before, "config dependency signature changed");
}

// spec: UI-142
#[test]
fn read_only_session_builds_only_in_memory_and_rejects_other_crawls() {
    let _serial = SERIAL.lock().unwrap();
    let (_dir, file) = fixture();
    let cfg = CrawlConfig::default();
    let path = file.path().to_path_buf();
    drop(file);
    let file = CrawlFile::open(&path, os_store::OpenMode::ReadOnly).unwrap();
    let s = ViewSession::open(&file, &cfg, spec("Address", false), Budget::default()).unwrap();
    assert_eq!(window(&s, &file).total, 6);
    let count: i64 = file
        .conn()
        .query_row("SELECT COUNT(*) FROM view_cache", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
    let (_other_dir, other) = fixture();
    assert!(s.locate(&other, UrlId(1)).is_err());
}

// spec: UI-142
#[test]
fn sparse_ids_and_multivalue_headers_remain_stable_across_windows() {
    let _serial = SERIAL.lock().unwrap();
    let (_dir, file) = fixture();
    let cfg = CrawlConfig::default();
    common::insert(&file, &[common::row(4_000_000_000, "sparse")]);
    for (id, pos, raw) in [(1, 0, "a"), (1, 1, "b"), (2, 0, "c")] {
        file.conn().execute("INSERT INTO canonicals (url_id,source,position,raw,in_head) VALUES (?1,'html',?2,?3,1)", rusqlite::params![id,pos,raw]).unwrap();
    }
    let mut specification = spec("Address", false);
    specification.columns =
        ColumnSelection::Named(vec!["Address".into(), "Canonical Link Element".into()]);
    let s = ViewSession::open(&file, &cfg, specification, Budget::default()).unwrap();
    let first = s
        .rows(
            &file,
            RowsReq {
                version: 1,
                offset: 0,
                limit: 1,
            },
        )
        .unwrap();
    let last = s
        .rows(
            &file,
            RowsReq {
                version: 1,
                offset: 6,
                limit: 1,
            },
        )
        .unwrap();
    assert_eq!(first.columns, last.columns);
    assert_eq!(first.columns.len(), 3);
    assert!(s.locate(&file, UrlId(4_000_000_000)).unwrap().is_some());
}

// spec: UI-141
#[test]
fn old_snapshot_remains_readable_during_cancelled_worker_build() {
    let _serial = SERIAL.lock().unwrap();
    use std::sync::{mpsc, Mutex};
    struct Gate {
        entered: mpsc::Sender<()>,
        resume: Mutex<mpsc::Receiver<()>>,
    }
    impl os_analyze::search::FilterSource for Gate {
        fn key(&self) -> String {
            "fixture:gate".into()
        }
        fn bitmap(&self, _: &CrawlFile) -> Result<RoaringBitmap, ViewError> {
            self.entered.send(()).unwrap();
            self.resume.lock().unwrap().recv().unwrap();
            Ok((1..=6).collect())
        }
    }
    let (_dir, file) = fixture();
    let cfg = CrawlConfig::default();
    let mut session =
        ViewSession::open(&file, &cfg, spec("Address", false), Budget::default()).unwrap();
    let before = window(&session, &file);
    let snapshot = session.snapshot(&file).unwrap();
    let (entered, wait) = mpsc::channel();
    let (resume, blocked) = mpsc::channel();
    let mut next = spec("Status Code", true);
    next.restrict.push(Arc::new(Gate {
        entered,
        resume: Mutex::new(blocked),
    }));
    let control = BuildControl::default();
    let worker_control = control.clone();
    let path = file.path().to_path_buf();
    let worker = std::thread::spawn(move || {
        let file = CrawlFile::open(&path, os_store::OpenMode::ReadOnly).unwrap();
        assert!(session
            .update_controlled(&file, &cfg, next, &worker_control)
            .is_err());
        session
    });
    wait.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(
        snapshot
            .rows(
                &file,
                RowsReq {
                    version: before.version,
                    offset: 0,
                    limit: 500
                }
            )
            .unwrap(),
        before
    );
    control.cancel.cancel();
    resume.send(()).unwrap();
    let session = worker.join().unwrap();
    assert_eq!(window(&session, &file), before);
}
