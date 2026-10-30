#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "runtime_common.rs"]
mod common;
use os_app::{
    dto::CrawlConfigDto,
    pool::CrawlKey,
    runtime::{lifecycle_bus::CrawlEvent, sessions::SessionInfo},
    App, AppConfig, CallCtx, ErrorCode, WindowId,
};
use os_model::{AnalysisStep, CrawlConfig, CrawlState};
use os_store::{CrawlFile, OpenMode};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

fn dto(cfg: &CrawlConfig) -> Value {
    serde_json::to_value(CrawlConfigDto {
        schema_version: cfg.schema_version,
        generator: cfg.generator.clone(),
        config: serde_json::to_value(cfg).unwrap(),
    })
    .unwrap()
}
fn fixture(dir: &std::path::Path, state: CrawlState) -> (App, CrawlKey, std::path::PathBuf) {
    let app = common::new_app(dir);
    let path = common::make_crawl(dir, "config.ospider");
    let key = CrawlKey("fixture".into());
    app.register_crawl(key.clone(), path.clone());
    let session = SessionInfo {
        key: key.clone(),
        path: path.clone(),
        config: Arc::new(CrawlConfig::default()),
        state,
        writable: true,
        engine: None,
    };
    app.sessions()
        .set_current(WindowId("main".into()), session.clone());
    app.sessions()
        .set_current(WindowId("other".into()), session);
    (app, key, path)
}
async fn call(app: &App, name: &str, args: Value) -> Value {
    app.dispatch(common::ctx(), name, args).await.unwrap()
}

// spec: ONPAGE-002
// spec: CRAWL-126 [M2]
#[tokio::test]
async fn config_changes_are_persisted_invalidated_and_announced_after_commit() {
    let dir = tempfile::tempdir().unwrap();
    let (app, key, path) = fixture(dir.path(), CrawlState::Completed);
    let observed = Arc::new(Mutex::new(Vec::new()));
    let out = observed.clone();
    app.crawl_events().subscribe(Box::new(move |app, event| {
        if let CrawlEvent::ConfigChanged { key, old, new } = event {
            assert_eq!(
                app.sessions().all()[0].config.title.max_chars,
                new.title.max_chars
            );
            let session = app
                .sessions()
                .all()
                .into_iter()
                .find(|s| &s.key == key)
                .unwrap();
            assert_eq!(
                CrawlFile::open(&session.path, OpenMode::ReadOnly)
                    .unwrap()
                    .config()
                    .unwrap(),
                **new
            );
            out.lock()
                .unwrap()
                .push((old.title.max_chars, new.title.max_chars));
        }
    }));
    let mut cfg = CrawlConfig::default();
    cfg.title.max_chars = 50;
    cfg.links.high_crawl_depth = 8;
    let result = call(&app, "config.set", dto(&cfg)).await;
    assert!(result["invalidatedSteps"]
        .as_array()
        .unwrap()
        .contains(&json!("links")));
    assert!(result["deferred"].as_array().unwrap().is_empty());
    assert_eq!(*observed.lock().unwrap(), vec![(60, 50)]);
    assert_eq!(
        app.sessions()
            .current(&WindowId("other".into()))
            .unwrap()
            .config
            .as_ref(),
        &cfg
    );
    assert_eq!(call(&app, "config.get", Value::Null).await, dto(&cfg));
    app.write_queue()
        .submit_file(&key, |file| {
            let stale = os_analyze::analysis::persist::read_stale(file).unwrap();
            assert!(stale.contains(AnalysisStep::Links));
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(
        CrawlFile::open(&path, OpenMode::ReadOnly)
            .unwrap()
            .config()
            .unwrap(),
        cfg
    );
}

// spec: CRAWL-126 [M2]
#[tokio::test]
async fn running_accepts_only_f13_and_keeps_engine_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let (app, _, path) = fixture(dir.path(), CrawlState::Running);
    let engine_snapshot = app
        .sessions()
        .current(&WindowId("main".into()))
        .unwrap()
        .config;
    let mut cfg = CrawlConfig::default();
    cfg.title.max_chars = 50;
    cfg.speed.threads = 3;
    cfg.analysis.links = false;
    let result = call(&app, "config.set", dto(&cfg)).await;
    assert_eq!(
        result["deferred"],
        json!(["analysis.links", "title.max_chars"])
    );
    assert_eq!(engine_snapshot.title.max_chars, 60);
    assert_ne!(engine_snapshot.speed.threads, 3);
    cfg.scope.include.push(".*".into());
    let err = app
        .dispatch(common::ctx(), "config.set", dto(&cfg))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::CrawlRunning);
    assert_eq!(err.field.as_deref(), Some("scope.include"));
    cfg.scope.include.clear();
    assert_eq!(
        CrawlFile::open(&path, OpenMode::ReadOnly)
            .unwrap()
            .config()
            .unwrap(),
        cfg
    );
}

// spec: CRAWL-126 [M2]
#[tokio::test]
async fn paused_allows_fetch_settings_but_rejects_structural_changes() {
    let dir = tempfile::tempdir().unwrap();
    let (app, _, _) = fixture(dir.path(), CrawlState::Paused);
    let mut cfg = CrawlConfig::default();
    cfg.scope.exclude.push(".*/private/.*".into());
    cfg.limits.max_urls += 10;
    call(&app, "config.set", dto(&cfg)).await;
    // Alle Erfassungs- und Speichern/Crawlen-Schalter bleiben in Pause editierbar.
    let mut input = dto(&cfg);
    let fields = input["config"]["spider"].as_object_mut().unwrap();
    for value in fields.values_mut() {
        if let Some(flag) = value.as_bool() {
            *value = json!(!flag);
        } else {
            value["store"] = json!(false);
            value["crawl"] = json!(false);
        }
    }
    input["config"]["scope"]["include"] = json!([".*"]);
    input["config"]["scope"]["regex_partial_match"] = json!(true);
    call(&app, "config.set", input.clone()).await;
    assert_eq!(call(&app, "config.get", Value::Null).await, input);
    for (pointer, path, value) in [
        ("/config/mode", "mode", json!("list")),
        (
            "/config/scope/all_subdomains",
            "scope.all_subdomains",
            json!(true),
        ),
        (
            "/config/scope/subfolder_only",
            "scope.subfolder_only",
            json!(true),
        ),
        (
            "/config/scope/crawl_outside_start_folder",
            "scope.crawl_outside_start_folder",
            json!(false),
        ),
        (
            "/config/rendering/mode",
            "rendering.mode",
            json!("javascript"),
        ),
    ] {
        let mut changed = input.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        let err = app
            .dispatch(common::ctx(), "config.set", changed)
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput);
        assert_eq!(err.field.as_deref(), Some(path));
        assert_eq!(call(&app, "config.get", Value::Null).await, input);
    }
}

// spec: ONPAGE-002
// spec: INTEG-081 [M2]
// spec: INTEG-082 [M2]
// spec: CRAWL-102 [M2]
#[tokio::test]
async fn validation_and_rejection_leave_config_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let (app, _, path) = fixture(dir.path(), CrawlState::Completed);
    let baseline = dto(&CrawlConfig::default());
    assert_eq!(
        baseline["config"]["link_positions"]["rules"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["position"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec![
            "Navigation",
            "Header",
            "Aside",
            "Sidebar",
            "Footer",
            "Content"
        ]
    );
    for (pointer, value) in [
        ("/config/title/min_chars", json!(500)),
        ("/config/title/max_chars", json!(-1)),
        ("/config/link_positions/rules", json!([])),
        (
            "/config/robots/custom_files",
            json!({"https://bad.test":"x"}),
        ),
        ("/config/speed/threads", json!(0)),
    ] {
        let mut input = baseline.clone();
        *input.pointer_mut(pointer).unwrap() = value;
        let issues = call(&app, "config.validate", input.clone()).await;
        assert!(
            issues
                .as_array()
                .unwrap()
                .iter()
                .any(|i| i["warning"] == false),
            "{pointer}: {issues}"
        );
        assert_eq!(
            app.dispatch(common::ctx(), "config.set", input)
                .await
                .unwrap_err()
                .code,
            ErrorCode::InvalidInput
        );
        assert_eq!(
            CrawlFile::open(&path, OpenMode::ReadOnly)
                .unwrap()
                .config()
                .unwrap(),
            CrawlConfig::default()
        );
    }
    let schema = call(&app, "config.schema", Value::Null).await;
    assert_eq!(
        schema["properties"]["speed"]["$ref"],
        json!("#/$defs/SpeedConfig")
    );
}

// spec: INTEG-082 [M2]
#[tokio::test]
async fn ordered_rules_are_saved_and_recompute_is_triggered() {
    let dir = tempfile::tempdir().unwrap();
    let (app, _, _) = fixture(dir.path(), CrawlState::Completed);
    let mut cfg = CrawlConfig::default();
    cfg.link_positions.rules.swap(0, 1);
    let result = call(&app, "config.set", dto(&cfg)).await;
    assert!(result["invalidatedSteps"]
        .as_array()
        .unwrap()
        .contains(&json!("links")));
    assert_eq!(call(&app, "config.get", Value::Null).await, dto(&cfg));
}

// spec: ONPAGE-002
#[tokio::test]
async fn write_failure_rolls_back_config_and_emits_no_event() {
    let dir = tempfile::tempdir().unwrap();
    let (app, key, _) = fixture(dir.path(), CrawlState::Completed);
    app.write_queue().submit(key,|conn| {conn.execute_batch("CREATE TRIGGER fail_config BEFORE UPDATE ON crawl_config BEGIN SELECT RAISE(ABORT, 'fixture'); END;")?; Ok(())}).await.unwrap();
    let events = Arc::new(Mutex::new(0));
    let counter = events.clone();
    app.crawl_events()
        .subscribe(Box::new(move |_, _| *counter.lock().unwrap() += 1));
    let mut cfg = CrawlConfig::default();
    cfg.title.max_chars = 50;
    assert!(app
        .dispatch(common::ctx(), "config.set", dto(&cfg))
        .await
        .is_err());
    assert_eq!(
        call(&app, "config.get", Value::Null).await,
        dto(&CrawlConfig::default())
    );
    assert_eq!(*events.lock().unwrap(), 0);
}

// spec: OUT-220 [M2]
#[tokio::test]
async fn default_drafts_are_isolated_and_diff_uses_stable_paths() {
    let dir = tempfile::tempdir().unwrap();
    let app = App::new(AppConfig::new(dir.path())).unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.title.max_chars = 50;
    call(&app, "config.set", dto(&cfg)).await;
    assert_eq!(call(&app, "config.get", Value::Null).await, dto(&cfg));
    assert_eq!(
        app.dispatch(CallCtx::for_window("other"), "config.get", Value::Null)
            .await
            .unwrap(),
        dto(&CrawlConfig::default())
    );
    let diff = call(&app, "config.diff", json!({"base":{"kind":"defaults"}})).await;
    assert_eq!(diff[0]["path"], "title.max_chars");
    assert_eq!(diff[0]["current"], 50);
    assert_eq!(diff[0]["base"], 60);
}

// spec: CRAWL-126 [M2]
#[tokio::test]
async fn threads_change_reaches_the_running_engine_without_pause() {
    use os_app::runtime::sessions::EngineControl;
    use os_engine::{
        start_crawl,
        testing::{test_deps, SimFetcher, SimSite},
        CrawlRequest, CrawlSource,
    };
    use std::time::Duration;
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = 1;
    cfg.speed.max_per_host = Some(8);
    let path = dir.path().join("live.ospider");
    let fetcher = SimFetcher::new(SimSite::tree("site.test", 100).delay(Duration::from_millis(15)));
    let handle = start_crawl(
        fetcher.clone(),
        CrawlRequest {
            config: cfg.clone(),
            source: CrawlSource::Spider {
                start_url: "https://site.test/".into(),
            },
            file: path.clone(),
            project: "fixture".into(),
            name: "live threads".into(),
        },
        test_deps(),
    )
    .await
    .unwrap();
    let engine = Arc::new(EngineControl {
        handle: Mutex::new(Some(handle)),
    });
    let key = CrawlKey("live".into());
    app.register_crawl(key.clone(), path.clone());
    app.sessions().set_current(
        WindowId("main".into()),
        SessionInfo {
            key,
            path,
            config: Arc::new(cfg.clone()),
            state: CrawlState::Running,
            writable: true,
            engine: Some(engine.clone()),
        },
    );
    assert!(common::eventually(Duration::from_secs(3), || fetcher.requests().len() >= 5).await);
    assert_eq!(fetcher.max_concurrency(), 1);
    cfg.speed.threads = 4;
    let result = call(&app, "config.set", dto(&cfg)).await;
    assert_eq!(result["deferred"], json!([]));
    assert!(common::eventually(Duration::from_secs(2), || fetcher.max_concurrency() > 1).await);
    let handle = engine.handle.lock().unwrap().take().unwrap();
    handle.pause();
    assert_eq!(handle.wait().await.unwrap().state, CrawlState::Paused);
}

// spec: ONPAGE-002
#[tokio::test]
async fn threshold_change_requeries_stored_fixture_without_http() {
    use os_analyze::views::{filter_counts, query_view, ViewQuery};
    let fx = os_engine::testing::crawl_fixture("onpage-titles", CrawlConfig::default()).await;
    let app = common::new_app(fx.dir.path());
    let key = CrawlKey("titles".into());
    let path = fx.file.path().to_path_buf();
    app.register_crawl(key.clone(), path.clone());
    app.sessions().set_current(
        WindowId("main".into()),
        SessionInfo {
            key,
            path,
            config: Arc::new(CrawlConfig::default()),
            state: CrawlState::Completed,
            writable: true,
            engine: None,
        },
    );
    let before_requests = fx.site.requests().len();
    let before = query_view(
        &fx.file,
        &CrawlConfig::default(),
        &ViewQuery::new("Page Titles", "Over 60 Characters"),
    )
    .unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.title.max_chars = 50;
    call(&app, "config.set", dto(&cfg)).await;
    let stored = fx.file.config().unwrap();
    let after = query_view(
        &fx.file,
        &stored,
        &ViewQuery::new("Page Titles", "Over 50 Characters"),
    )
    .unwrap();
    assert!(after.total > before.total);
    let counts = filter_counts(&fx.file, &stored).unwrap();
    assert!(counts
        .iter()
        .any(|c| c.tab == "Page Titles" && c.label == "Over 50 Characters"));
    assert_eq!(fx.site.requests().len(), before_requests);
}

// spec: CRAWL-126 [M2]
#[tokio::test]
async fn paused_exclude_purges_queue_immediately_with_rollback_on_failure() {
    let dir = tempfile::tempdir().unwrap();
    let (app, key, _) = fixture(dir.path(), CrawlState::Paused);
    app.write_queue().submit(key.clone(),|conn| {
        for (id,url) in [(1,"https://site.test/"),(2,"https://site.test/private/one"),(3,"https://site.test/public/one")] {
            conn.execute("INSERT INTO urls (id,url,address,origin,is_internal,state,discovered_via,folder_depth,url_length) VALUES (?1,?2,?2,'https://site.test',1,'queued','hyperlink',0,30)",rusqlite::params![id,url])?;
            conn.execute("INSERT INTO queue (url_id,seq) VALUES (?1,?1)",[id])?;
        }
        conn.execute_batch("CREATE TRIGGER fail_config BEFORE UPDATE ON crawl_config BEGIN SELECT RAISE(ABORT, 'fixture'); END;")?;
        Ok(())
    }).await.unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.scope.exclude = vec![".*private.*".into(), "https://site.test/".into()];
    assert!(app
        .dispatch(common::ctx(), "config.set", dto(&cfg))
        .await
        .is_err());
    app.write_queue()
        .submit(key.clone(), |conn| {
            assert_eq!(
                conn.query_row("SELECT count(*) FROM queue", [], |r| r.get::<_, i64>(0))?,
                3
            );
            assert_eq!(
                conn.query_row("SELECT in_tables FROM urls WHERE id = 2", [], |r| r
                    .get::<_, i64>(0))?,
                1
            );
            conn.execute_batch("DROP TRIGGER fail_config")?;
            Ok(())
        })
        .await
        .unwrap();
    call(&app, "config.set", dto(&cfg)).await;
    app.write_queue().submit(key,|conn| {
        assert_eq!(conn.query_row("SELECT group_concat(url_id, ',') FROM (SELECT url_id FROM queue ORDER BY url_id)",[],|r|r.get::<_,String>(0))?,"1,3");
        assert_eq!(conn.query_row("SELECT not_crawled_reason FROM urls WHERE id = 2",[],|r|r.get::<_,String>(0))?,"excluded");
        assert_eq!(conn.query_row("SELECT value FROM crawl_counters WHERE key = 'excluded_skipped'",[],|r|r.get::<_,i64>(0))?,1);
        assert_eq!(conn.query_row("SELECT change_seq FROM urls WHERE id = 2",[],|r|r.get::<_,i64>(0))?,1);
        Ok(())
    }).await.unwrap();
}

// spec: ONPAGE-002
#[tokio::test]
async fn deferred_commit_failure_rolls_back_and_leaves_the_queue_connection_usable() {
    let dir = tempfile::tempdir().unwrap();
    let (app, key, _) = fixture(dir.path(), CrawlState::Completed);
    app.write_queue().submit_file(&key,|file| {
        file.conn().execute_batch("PRAGMA foreign_keys = ON;
            CREATE TABLE config_parent (id INTEGER PRIMARY KEY);
            CREATE TABLE config_child (id INTEGER REFERENCES config_parent(id) DEFERRABLE INITIALLY DEFERRED);
            CREATE TRIGGER fail_commit AFTER UPDATE ON crawl_config BEGIN INSERT INTO config_child VALUES (99); END;").unwrap();
        Ok(())
    }).await.unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.title.max_chars = 50;
    assert!(app
        .dispatch(common::ctx(), "config.set", dto(&cfg))
        .await
        .is_err());
    assert_eq!(
        call(&app, "config.get", Value::Null).await,
        dto(&CrawlConfig::default())
    );
    app.write_queue()
        .submit_file(&key, |file| {
            assert!(file.conn().is_autocommit());
            assert_eq!(file.config().unwrap(), CrawlConfig::default());
            assert_eq!(
                file.conn()
                    .query_row("SELECT count(*) FROM config_child", [], |r| r
                        .get::<_, i64>(0))
                    .unwrap(),
                0
            );
            file.conn()
                .execute_batch("DROP TRIGGER fail_commit")
                .unwrap();
            Ok(())
        })
        .await
        .unwrap();
    call(&app, "config.set", dto(&cfg)).await;
    assert_eq!(call(&app, "config.get", Value::Null).await, dto(&cfg));
}
