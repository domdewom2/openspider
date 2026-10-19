#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "session_common.rs"]
mod common;
use common::*;
use os_app::{dto::*, runtime::lifecycle_bus::CrawlEvent, session};
use serde_json::json;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

// spec: CRAWL-126 [M2]
// spec: CRAWL-128
// spec: CRAWL-143
// spec: UI-018
#[tokio::test]
async fn fixture_pause_copy_reopen_resume_matches_uninterrupted_crawl() {
    let site = os_testsite::TestSite::start_dir(fixture(), 0)
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    cfg().save_default(&app.config().paths.settings).unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let copy = events.clone();
    app.crawl_events().subscribe(Box::new(move |_, ev| {
        copy.lock().unwrap().push(match ev {
            CrawlEvent::Started(_) => "started",
            CrawlEvent::Paused(_) => "paused",
            CrawlEvent::Resumed(_) => "resumed",
            CrawlEvent::Completed(_) => "completed",
            CrawlEvent::Cleared(_) => "cleared",
            CrawlEvent::Opened(_) => "opened",
            _ => "other",
        })
    }));
    let started = call(
        &app,
        "crawl.start",
        serde_json::to_value(request(site.url("/"))).unwrap(),
    )
    .await;
    tokio::time::timeout(Duration::from_secs(5), async {
        while site.requests().len() < 6 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    call(&app, "crawl.pause", json!(null)).await;
    assert_eq!(
        app.sessions().current(&window()).unwrap().state,
        os_model::CrawlState::Paused
    );
    let before = site.requests().len();
    tokio::time::sleep(Duration::from_millis(120)).await;
    assert_eq!(site.requests().len(), before);
    let queued = app
        .read_pool()
        .run(key(&app), |f, _| {
            Ok(f.conn()
                .query_row("SELECT COUNT(*) FROM queue", [], |r| r.get::<_, u64>(0))?)
        })
        .await
        .unwrap();
    assert!(queued > 0);
    let copy_path = dir.path().join("portable.ospider");
    let saved = operation(&app, "crawl.saveAs", json!({"dest":copy_path})).await;
    assert_eq!(saved.state, OpState::Done, "{:?}", saved.error);
    call(&app, "crawl.clear", json!(null)).await;
    assert!(app.sessions().current(&window()).is_none());
    let summary = open(&app, &copy_path).await;
    assert_eq!(summary.crawl_id, started["crawlId"].as_str().unwrap());
    call(&app, "crawl.resume", json!(null)).await;
    completed(&app).await;
    let actual = app
        .read_pool()
        .run(key(&app), |f, _| {
            let mut stmt = f.conn().prepare(
                "SELECT url,status_code,crawl_depth FROM urls WHERE in_tables = 1 ORDER BY url",
            )?;
            let rows = stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Option<u16>>(1)?,
                        r.get::<_, Option<u32>>(2)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await
        .unwrap();
    assert_eq!(actual.len(), 27, "{actual:?}");
    let full_dir = tempfile::tempdir().unwrap();
    let full = common::app(full_dir.path());
    cfg().save_default(&full.config().paths.settings).unwrap();
    call(
        &full,
        "crawl.start",
        serde_json::to_value(request(site.url("/"))).unwrap(),
    )
    .await;
    completed(&full).await;
    let expected = full
        .read_pool()
        .run(key(&full), |f, _| {
            let mut stmt = f.conn().prepare(
                "SELECT url,status_code,crawl_depth FROM urls WHERE in_tables = 1 ORDER BY url",
            )?;
            let rows = stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Option<u16>>(1)?,
                        r.get::<_, Option<u32>>(2)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await
        .unwrap();
    assert_eq!(actual, expected);
    let sequence = events.lock().unwrap();
    for event in [
        "started",
        "paused",
        "cleared",
        "opened",
        "resumed",
        "completed",
    ] {
        assert!(sequence.contains(&event), "{sequence:?}");
    }
}

// spec: CRAWL-128
// spec: CRAWL-143
#[tokio::test]
async fn completed_empty_and_failed_are_not_resumable_but_limited_queue_is() {
    let site = os_testsite::TestSite::start_dir(fixture(), 0)
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let mut config = cfg();
    config.limits.max_urls = 3;
    config.save_default(&app.config().paths.settings).unwrap();
    call(
        &app,
        "crawl.start",
        serde_json::to_value(request(site.url("/"))).unwrap(),
    )
    .await;
    completed(&app).await;
    app.write_queue()
        .submit_file(&key(&app), |f| {
            let mut c = f
                .config()
                .map_err(|e| os_app::runtime::error::store_error(&e))?;
            c.limits.max_urls = 100;
            f.set_config(&c)
                .map_err(|e| os_app::runtime::error::store_error(&e))
        })
        .await
        .unwrap();
    call(&app, "crawl.resume", json!(null)).await;
    completed(&app).await;
    let err = app
        .dispatch(ctx(), "crawl.resume", json!(null))
        .await
        .unwrap_err();
    assert_eq!(err.code, os_app::ErrorCode::InvalidInput);
    app.write_queue()
        .submit_file(&key(&app), |f| {
            f.set_meta("state", "Failed")
                .map_err(|e| os_app::runtime::error::store_error(&e))
        })
        .await
        .unwrap();
    assert_eq!(
        app.dispatch(ctx(), "crawl.resume", json!(null))
            .await
            .unwrap_err()
            .code,
        os_app::ErrorCode::InvalidInput
    );
}

// spec: ONPAGE-193
// spec: CRAWL-131
#[tokio::test]
async fn remove_and_recrawl_are_operations_and_keep_the_feed_valid() {
    let site = os_testsite::TestSite::start_dir(fixture(), 0)
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    cfg().save_default(&app.config().paths.settings).unwrap();
    call(
        &app,
        "crawl.start",
        serde_json::to_value(request(site.url("/"))).unwrap(),
    )
    .await;
    completed(&app).await;
    let args = json!({"selection":{"kind":"ids","ids":[1,1,999999]}});
    let result = operation(&app, "crawl.recrawl", args.clone()).await;
    assert_eq!(result.state, OpState::Done, "{:?}", result.error);
    assert_eq!(result.result.unwrap()["count"], 1);
    let result = operation(&app, "crawl.remove", args).await;
    assert_eq!(result.state, OpState::Done, "{:?}", result.error);
    assert_eq!(result.result.unwrap()["count"], 1);
    app.read_pool()
        .run(key(&app), |f, _| {
            assert_eq!(
                f.conn()
                    .query_row("SELECT COUNT(*) FROM urls WHERE id=1", [], |r| r
                        .get::<_, i64>(0))?,
                0
            );
            assert!(f.get_meta("reset_seq")?.is_some());
            assert_eq!(f.get_meta("analysis_stale")?.as_deref(), Some("1"));
            Ok(())
        })
        .await
        .unwrap();
}

// spec: UI-003
#[tokio::test]
async fn validation_lists_invalid_open_and_own_engine_replacement_guards() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let url = call(
        &app,
        "crawl.validateStartUrl",
        json!({"input":"example.test"}),
    )
    .await;
    assert_eq!(url["normalized"], "http://example.test/");
    assert_eq!(
        app.dispatch(
            ctx(),
            "crawl.validateStartUrl",
            json!({"input":"ht tp://x"})
        )
        .await
        .unwrap_err()
        .code,
        os_app::ErrorCode::InvalidInput
    );
    let list=call(&app,"crawl.parseList",json!({"kind":"text","text":"https://a.test/\nhttps://a.test/\nnot a URL\nhttps://b.test/"})).await;
    assert_eq!(list["valid"], 2);
    assert_eq!(list["duplicates"], 1);
    assert_eq!(list["invalid"], 1);
    let bad = dir.path().join("not-a-crawl.ospider");
    std::fs::write(&bad, b"not SQLite").unwrap();
    assert!(app
        .dispatch(ctx(), "crawl.open", json!({"kind":"path","path":bad}))
        .await
        .is_err());
    let fetch = os_engine::testing::SimFetcher::new(
        os_engine::testing::SimSite::new()
            .delay(Duration::from_millis(50))
            .page("https://fixture.test/", "<a href='/a'>A</a>")
            .page("https://fixture.test/a", "A"),
    );
    session::control::start_with_fetcher(
        &app,
        ctx(),
        request("https://fixture.test/".into()),
        cfg(),
        fetch,
    )
    .await
    .unwrap();
    assert_eq!(
        app.dispatch(ctx(), "crawl.clear", json!(null))
            .await
            .unwrap_err()
            .code,
        os_app::ErrorCode::Busy
    );
    call(&app, "crawl.stop", json!(null)).await;
    assert_eq!(
        app.sessions().current(&window()).unwrap().state,
        os_model::CrawlState::Completed
    );
}
