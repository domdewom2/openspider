//! N14 differential fixture and opt-in real HTTP crawl/statement benchmark.
#[path = "n14_support/before.rs"]
mod before;
#[path = "framework_common.rs"]
mod common;

use os_analyze::analysis::{steps::links, CancelToken};
use os_model::{CrawlConfig, StepState};
use os_store::{CrawlFile, OpenMode};
use rusqlite::{types::Value, Connection};
use std::time::Instant;

fn snapshot(conn: &Connection) -> Vec<Vec<Value>> {
    let mut result = Vec::new();
    for sql in [
        "SELECT * FROM url_ext ORDER BY url_id,key",
        "SELECT id,change_seq FROM urls ORDER BY id",
        "SELECT value FROM crawl_meta WHERE key='change_seq'",
    ] {
        let mut stmt = conn.prepare(sql).unwrap();
        let cols = stmt.column_count();
        result.extend(
            stmt.query_map([], |r| (0..cols).map(|i| r.get(i)).collect())
                .unwrap()
                .map(Result::unwrap),
        );
    }
    result
}

fn seed(file: &CrawlFile) {
    file.conn().execute_batch("BEGIN;
        WITH RECURSIVE ids(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM ids WHERE n<101)
        INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,folder_depth,url_length,indexability)
        SELECT n,'https://n14.test/'||n,'https://n14.test/'||n,'https://n14.test',n%11<>0,'done',
            CASE WHEN n%7=0 THEN 'start' ELSE 'link' END,0,30,
            CASE n%3 WHEN 0 THEN 'Non-Indexable' WHEN 1 THEN 'Indexable' ELSE NULL END FROM ids;
        WITH RECURSIVE ids(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM ids WHERE n<4000)
        INSERT INTO links(source_id,target_id,target_raw,link_type,position,follow,crawlable)
        SELECT 1+n%101,CASE WHEN n%23=0 THEN NULL ELSE 1+(n*17)%101 END,'https://n14.test/',
            CASE WHEN n%13=0 THEN 'Image' ELSE 'Hyperlink' END,n,
            CASE n%3 WHEN 0 THEN 0 WHEN 1 THEN 1 ELSE -1 END,n%17<>0 FROM ids;
        INSERT INTO url_ext(url_id,key,v_int,v_text,change_seq)
        SELECT id,'link.mixed_inlinks',CASE WHEN id%5=0 THEN NULL ELSE id%3 END,'stale',1 FROM urls;
        INSERT INTO url_ext(url_id,key,v_int,change_seq) SELECT id,'link.nofollow_only',1,1 FROM urls WHERE id%2=0;
        INSERT INTO url_ext(url_id,key,v_int,change_seq) SELECT id,'link.non_indexable_only',1,1 FROM urls WHERE id%3=0;
        INSERT INTO url_ext(url_id,key,v_text) SELECT id,'unrelated.text','untouched' FROM urls;
        COMMIT;") .unwrap();
}

// spec: TECH-139
// spec: TECH-140
// spec: TECH-142
// spec: TECH-145
#[test]
fn n14_streaming_matches_before_including_stale_facts_and_change_feed() {
    let dir = tempfile::tempdir().unwrap();
    let (_, old) = common::create(dir.path(), "before.ospider", 14);
    let (_, new) = common::create(dir.path(), "after.ospider", 14);
    seed(&old);
    seed(&new);
    let cfg = CrawlConfig::default();
    for _ in 0..2 {
        assert_eq!(
            before::run(&old, &cfg, &CancelToken::new()).unwrap(),
            StepState::Done
        );
        assert_eq!(
            links::run(&new, &cfg, &CancelToken::new()).unwrap(),
            StepState::Done
        );
        assert_eq!(snapshot(old.conn()), snapshot(new.conn()));
    }
    old.conn()
        .execute_batch("DELETE FROM links; UPDATE urls SET is_internal=0 WHERE id%2=0")
        .unwrap();
    new.conn()
        .execute_batch("DELETE FROM links; UPDATE urls SET is_internal=0 WHERE id%2=0")
        .unwrap();
    before::run(&old, &cfg, &CancelToken::new()).unwrap();
    links::run(&new, &cfg, &CancelToken::new()).unwrap();
    assert_eq!(snapshot(old.conn()), snapshot(new.conn()));
    let unchanged = snapshot(new.conn());
    let cancel = CancelToken::new();
    cancel.cancel();
    assert_eq!(links::run(&new, &cfg, &cancel).unwrap(), StepState::Pending);
    assert_eq!(unchanged, snapshot(new.conn()));
}

// SQLite's bundled profiler is enabled only here; it also captures statement execution
// while the Rust streaming loops consume rows. No product instrumentation or feature change.
unsafe extern "C" fn profile(
    _: u32,
    _: *mut std::ffi::c_void,
    stmt: *mut std::ffi::c_void,
    ns: *mut std::ffi::c_void,
) -> i32 {
    let sql = unsafe { std::ffi::CStr::from_ptr(rusqlite::ffi::sqlite3_sql(stmt.cast())) }
        .to_string_lossy();
    let nanos = unsafe { *ns.cast::<i64>() };
    eprintln!(
        "N14_SQL {}",
        serde_json::json!({"sql":sql,"seconds":nanos as f64/1e9})
    );
    0
}

fn profiling(conn: &Connection, enabled: bool) {
    unsafe {
        rusqlite::ffi::sqlite3_trace_v2(
            conn.handle(),
            if enabled {
                rusqlite::ffi::SQLITE_TRACE_PROFILE
            } else {
                0
            },
            if enabled { Some(profile) } else { None },
            std::ptr::null_mut(),
        );
    }
}

// spec: TECH-139
// spec: TECH-140
// spec: TECH-142
// spec: TECH-145
// spec: PERF-003
#[tokio::test(flavor = "multi_thread", worker_threads = 5)]
#[ignore = "Benchmark: N14_PAGES=200000/1000000, real synthetic HTTP crawl, 16 GiB scope"]
async fn n14_crawl_and_profile() {
    use os_engine::testing::{site_fetcher, test_deps};
    let n: usize = std::env::var("N14_PAGES")
        .unwrap_or_else(|_| "200000".into())
        .parse()
        .unwrap();
    let site =
        os_testsite::TestSite::start_spec(&format!("synthetic:n={n},links=30"), Default::default())
            .await
            .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("crawl.ospider");
    let cfg = CrawlConfig::default();
    let deps = test_deps();
    let start = Instant::now();
    let handle = os_engine::start_crawl(
        site_fetcher(&site, &cfg, &deps),
        os_engine::CrawlRequest {
            config: cfg.clone(),
            source: os_engine::CrawlSource::Spider {
                start_url: site.url("/"),
            },
            file: path.clone(),
            project: "N14".into(),
            name: "N14".into(),
        },
        deps,
    )
    .await
    .unwrap();
    let summary = handle.wait().await.unwrap();
    assert_eq!(summary.crawled, n as u64);
    eprintln!(
        "N14_CRAWL pages={n} seconds={} summary={summary:?}",
        start.elapsed().as_secs_f64()
    );
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    file.conn()
        .pragma_update(None, "cache_size", -65_536)
        .unwrap();
    file.conn()
        .pragma_update(Some("temp"), "cache_size", -32_768)
        .unwrap();
    // The standard synthetic site has no positive link issue facts. Before and
    // after therefore see identical input for these keys (checked below).
    let facts_before: i64 = file
        .conn()
        .query_row("SELECT COUNT(*) FROM analysis_link_facts", [], |r| r.get(0))
        .unwrap();
    assert_eq!(facts_before, 0);
    file.conn().execute_batch("CREATE TEMP TABLE n14_internal(id INTEGER PRIMARY KEY,indexable INTEGER);
        INSERT INTO n14_internal SELECT id,COALESCE(indexability,'')<>'Non-Indexable' FROM urls WHERE is_internal=1;").unwrap();
    let graph_plan = file.conn().prepare("EXPLAIN QUERY PLAN SELECT l.target_id AS id,
        MAX(l.follow<>0),MAX(l.follow=0),MAX(s.indexable)
        FROM links l JOIN n14_internal s ON s.id=l.source_id JOIN n14_internal t ON t.id=l.target_id
        WHERE l.link_type='Hyperlink' AND l.crawlable=1 AND l.source_id<>l.target_id GROUP BY l.target_id").unwrap()
        .query_map([], |r| r.get::<_, String>(3)).unwrap().map(Result::unwrap).collect::<Vec<_>>();
    eprintln!("N14_PLAN before_graph {}", serde_json::json!(graph_plan));
    file.conn()
        .execute_batch("DROP TABLE n14_internal")
        .unwrap();
    for (label, run) in [
        (
            "before",
            before::run as os_analyze::analysis::runner::StepFn,
        ),
        ("after", links::run),
    ] {
        eprintln!("N14_PHASE {label} pages={n}");
        profiling(file.conn(), true);
        let start = Instant::now();
        assert_eq!(
            run(&file, &cfg, &CancelToken::new()).unwrap(),
            StepState::Done
        );
        eprintln!(
            "N14_LINKS {label} pages={n} seconds={}",
            start.elapsed().as_secs_f64()
        );
        profiling(file.conn(), false);
        let facts: i64 = file
            .conn()
            .query_row("SELECT COUNT(*) FROM analysis_link_facts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(facts, facts_before);
    }
    // Also exercise a resumed crawl whose dictionary retains these keys even
    // when no current URL has an issue. This exposes the EAV scan independently.
    for key in [
        "link.mixed_inlinks",
        "link.nofollow_only",
        "link.non_indexable_only",
    ] {
        file.conn()
            .execute("INSERT OR IGNORE INTO ext_keys(key) VALUES(?1)", [key])
            .unwrap();
    }
    for (label, sql) in [
        ("before_eav", "SELECT url_id,key,v_int FROM url_ext WHERE key IN ('link.mixed_inlinks','link.nofollow_only','link.non_indexable_only')"),
        ("after_eav", "SELECT * FROM analysis_link_facts"),
        ("after_graph", "SELECT source_id,target_id,follow <> 0 FROM links NOT INDEXED WHERE link_type='Hyperlink' AND crawlable=1 AND source_id<>target_id"),
        ("after_delete", "DELETE FROM url_ext WHERE url_id=1 AND key IN ('link.mixed_inlinks','link.nofollow_only','link.non_indexable_only')"),
    ] {
        let plan = file.conn().prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap()
            .query_map([], |r| r.get::<_, String>(3)).unwrap().map(Result::unwrap).collect::<Vec<_>>();
        eprintln!("N14_PLAN {label} {}", serde_json::json!(plan));
        if label.ends_with("eav") {
            profiling(file.conn(), true);
            file.conn().prepare(sql).unwrap().query([]).unwrap().next().unwrap();
            profiling(file.conn(), false);
        }
    }
    // Existing progress callbacks/time diagnostics record every standard step.
    os_analyze::analysis::runner::run(&file, &cfg, &CancelToken::new(), &|step, fraction| {
        eprintln!("N14_PROGRESS {step:?} {fraction}");
    })
    .unwrap();
    drop(file);
    drop(site);
}
