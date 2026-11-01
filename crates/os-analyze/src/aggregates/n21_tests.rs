use super::{live, *};
use os_model::{CrawlConfig, CrawlMode, Timestamp};
use os_store::{CrawlInit, StorageProfile};

// spec: UI-145
#[test]
fn cold_sidebar_batches_shared_buckets_and_then_repairs_raw_contributions() {
    let dir = tempfile::tempdir().unwrap();
    let file = CrawlFile::create(
        &dir.path().join("n21-sidebar.ospider"),
        &CrawlInit {
            crawl_id: "00000000-0000-0000-0000-000000000021".parse().unwrap(),
            mode: CrawlMode::Spider,
            start_url: Some("https://site.test/".into()),
            list_source: None,
            project: "n21".into(),
            name: "n21".into(),
            config: CrawlConfig::default(),
            created_at: Timestamp::UNIX_EPOCH,
        },
        StorageProfile::Balanced,
    )
    .unwrap();
    file.conn().execute_batch("WITH RECURSIVE ids(id) AS (VALUES(1) UNION ALL SELECT id+1 FROM ids WHERE id<8192)
        INSERT INTO urls(id,url,address,origin,is_internal,in_tables,state,discovered_via,crawl_depth,folder_depth,url_length,content_class,indexability,status_code,response_time_ms,change_seq)
        SELECT id,'https://site.test/a/'||id,'https://site.test/a/'||id,'https://site.test',1,1,'done','link',2,1,40,'HTML','Indexable',200,123,1 FROM ids;").unwrap();
    file.set_meta("change_seq", "1").unwrap();
    let before = file.conn().total_changes();
    live::refresh(&file, &CrawlConfig::default()).unwrap();
    // One raw snapshot write per URL; shared counts are written once per batch.
    assert!(file.conn().total_changes() - before < 2 * 8192);
    assert_eq!(paths(&file, "", 0, 10).unwrap()[0].counts.count, 8192);
    assert_eq!(response_times(&file, true).unwrap().0[0], 8192);
    file.conn().execute_batch("UPDATE urls SET status_code=404,response_time_ms=2500,change_seq=2 WHERE id=1; DELETE FROM urls WHERE id=2").unwrap();
    file.set_meta("change_seq", "2").unwrap();
    file.set_meta("reset_seq", "2").unwrap();
    live::refresh(&file, &CrawlConfig::default()).unwrap();
    let root = paths(&file, "", 0, 10).unwrap();
    assert_eq!(root[0].counts.count, 8191);
    assert_eq!(root[0].counts.response_codes["4xx"], 1);
    assert_eq!(response_times(&file, true).unwrap().0[2], 1);
    let before = file.conn().total_changes();
    live::refresh(&file, &CrawlConfig::default()).unwrap();
    assert_eq!(
        file.conn().total_changes(),
        before,
        "unchanged read must not rebuild"
    );
}

fn sidebar_fixture(dir: &std::path::Path) -> CrawlFile {
    let file = CrawlFile::create(
        &dir.join("n21-sidebar-cache.ospider"),
        &CrawlInit {
            crawl_id: "00000000-0000-0000-0000-000000000022".parse().unwrap(),
            mode: CrawlMode::Spider,
            start_url: Some("https://site.test/".into()),
            list_source: None,
            project: "n21".into(),
            name: "n21".into(),
            config: CrawlConfig::default(),
            created_at: Timestamp::UNIX_EPOCH,
        },
        StorageProfile::Balanced,
    )
    .unwrap();
    file.conn().execute_batch("WITH RECURSIVE ids(id) AS (VALUES(1) UNION ALL SELECT id+1 FROM ids WHERE id<300)
        INSERT INTO urls(id,url,address,origin,is_internal,in_tables,state,discovered_via,crawl_depth,folder_depth,url_length,content_class,indexability,status_code,response_time_ms,change_seq)
        SELECT id,'https://site.test/d'||(id%7)||'/e'||(id%3)||'/'||id,'https://site.test/'||id,'https://site.test',id%11<>0,id%13<>0,'done','link',id%12,2,40,
          CASE WHEN id%5=0 THEN 'Images' ELSE 'HTML' END,CASE WHEN id%4=0 THEN 'Non-Indexable' ELSE 'Indexable' END,
          CASE WHEN id%17=0 THEN NULL WHEN id%9=0 THEN 404 ELSE 200 END,(id*37)%12000,1 FROM ids;").unwrap();
    file.set_meta("change_seq", "1").unwrap();
    file.set_meta("state", "Completed").unwrap();
    file
}

type CountRow = (String, String, String, i64, i64, i64, i64, i64);
fn all_counts(file: &CrawlFile) -> Vec<CountRow> {
    let mut stmt = file.conn().prepare("SELECT kind,key,parent,n,idx,nonidx,s2,s4 FROM temp.k03_counts WHERE n<>0 ORDER BY kind,key").unwrap();
    stmt.query_map([], |r| {
        Ok((
            r.get(0)?,
            r.get(1)?,
            r.get(2)?,
            r.get(3)?,
            r.get(4)?,
            r.get(5)?,
            r.get(6)?,
            r.get(7)?,
        ))
    })
    .unwrap()
    .collect::<Result<_, _>>()
    .unwrap()
}

// spec: UI-145
#[test]
fn split_cold_build_equals_the_serial_aggregates() {
    let dir = tempfile::tempdir().unwrap();
    let file = sidebar_fixture(dir.path());
    live::refresh(&file, &CrawlConfig::default()).unwrap();
    let serial = all_counts(&file);
    assert!(!serial.is_empty());
    let other = CrawlFile::open(file.path(), os_store::OpenMode::ReadOnly).unwrap();
    crate::views::FORCE_PARALLEL.with(|f| f.set(true));
    let result = live::refresh(&other, &CrawlConfig::default());
    crate::views::FORCE_PARALLEL.with(|f| f.set(false));
    result.unwrap();
    assert_eq!(crate::views::PARALLEL_PASSES.with(|n| n.get()), 1);
    assert_eq!(all_counts(&other), serial);
}

// spec: UI-145
// spec: UI-147
#[test]
fn persisted_sidebar_aggregates_restore_and_then_follow_changes() {
    let dir = tempfile::tempdir().unwrap();
    let file = sidebar_fixture(dir.path());
    let reader = CrawlFile::open(file.path(), os_store::OpenMode::ReadOnly).unwrap();
    live::refresh(&reader, &CrawlConfig::default()).unwrap();
    let cold = all_counts(&reader);
    let export = live::take_export(&reader)
        .unwrap()
        .expect("cold build export");
    assert!(live::take_export(&reader).unwrap().is_none(), "taken once");
    assert!(
        !live::persist(&reader, &export).unwrap(),
        "read-only stores nothing"
    );
    assert!(live::persist(&file, &export).unwrap());

    // A fresh connection restores without scanning and offers nothing to store again.
    let restored = CrawlFile::open(file.path(), os_store::OpenMode::ReadOnly).unwrap();
    live::refresh(&restored, &CrawlConfig::default()).unwrap();
    assert_eq!(all_counts(&restored), cold);
    assert!(live::take_export(&restored).unwrap().is_none());

    // Changes after a restore rebuild once with raw facts, then stay incremental.
    file.conn()
        .execute_batch(
            "UPDATE urls SET status_code=500, change_seq=2 WHERE id=1; DELETE FROM urls WHERE id=2",
        )
        .unwrap();
    file.set_meta("change_seq", "2").unwrap();
    file.set_meta("reset_seq", "2").unwrap();
    live::refresh(&restored, &CrawlConfig::default()).unwrap();
    let truth = CrawlFile::open(file.path(), os_store::OpenMode::ReadOnly).unwrap();
    live::refresh(&truth, &CrawlConfig::default()).unwrap();
    assert_eq!(all_counts(&restored), all_counts(&truth));
    assert_ne!(all_counts(&restored), cold);
    // F05 contract on a restored connection: named changes are applied.
    let applied = CrawlFile::open(file.path(), os_store::OpenMode::ReadOnly).unwrap();
    live::refresh(&applied, &CrawlConfig::default()).unwrap();
    file.conn()
        .execute_batch("UPDATE urls SET response_time_ms=9500, change_seq=3 WHERE id=3")
        .unwrap();
    file.set_meta("change_seq", "3").unwrap();
    live::apply_changes(&applied, &CrawlConfig::default(), &[os_model::UrlId(3)]).unwrap();
    live::refresh(&applied, &CrawlConfig::default()).unwrap();
    live::refresh(&truth, &CrawlConfig::default()).unwrap();
    assert_eq!(all_counts(&applied), all_counts(&truth));
    // A stale stored version is ignored.
    let stale = CrawlFile::open(file.path(), os_store::OpenMode::ReadOnly).unwrap();
    live::refresh(&stale, &CrawlConfig::default()).unwrap();
    assert!(
        live::take_export(&stale).unwrap().is_some(),
        "cold build, not the old export"
    );
}
