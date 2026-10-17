#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "session_common.rs"]
mod common;
use common::*;
use os_app::{dto::*, session::feed};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

// spec: ONPAGE-193
// spec: UI-018 [M2]
#[tokio::test]
async fn feed_includes_counter_only_changes_and_reset_without_unbounded_id_lists() {
    let dir = tempfile::tempdir().unwrap();
    let (app, clock) = manual_app(dir.path());
    let f = create(&dir.path().join("feed.ospider"), cfg());
    f.set_meta("state", "Paused").unwrap();
    insert(&f, 1, 1);
    open(&app, f.path()).await;
    let changes = Arc::new(Mutex::new(Vec::new()));
    let copy = changes.clone();
    feed::subscribe(
        &app,
        Arc::new(move |_, c| copy.lock().unwrap().push(c.clone())),
    );
    os_app::session::poll(&app).await.unwrap();
    assert!(changes.lock().unwrap()[0].reset);
    f.conn()
        .execute(
            "UPDATE url_counters SET change_seq=2,inlinks=7 WHERE url_id=1",
            [],
        )
        .unwrap();
    f.set_meta("change_seq", "2").unwrap();
    clock.advance(Duration::from_secs(1));
    os_app::session::poll(&app).await.unwrap();
    assert_eq!(changes.lock().unwrap()[1].ids, vec![os_model::UrlId(1)]);
    f.conn().execute("DELETE FROM urls WHERE id=1", []).unwrap();
    f.set_meta("change_seq", "3").unwrap();
    f.set_meta("reset_seq", "3").unwrap();
    clock.advance(Duration::from_secs(1));
    os_app::session::poll(&app).await.unwrap();
    assert!(changes.lock().unwrap()[2].reset);
    f.conn().execute_batch("BEGIN").unwrap();
    for id in 2..=4100 {
        insert(&f, id, 4);
    }
    f.conn().execute_batch("COMMIT").unwrap();
    clock.advance(Duration::from_secs(1));
    os_app::session::poll(&app).await.unwrap();
    {
        let all = changes.lock().unwrap();
        let last = all.last().unwrap();
        assert!(last.reset);
        assert!(last.ids.is_empty());
    }
    clock.advance(Duration::from_secs(1));
    os_app::session::poll(&app).await.unwrap();
    assert_eq!(changes.lock().unwrap().len(), 4);
}

// spec: AGENT-020 [M2]
// spec: UI-013 [M2]
#[tokio::test]
async fn live_foreign_crawl_is_read_only_and_uses_persisted_progress() {
    let dir = tempfile::tempdir().unwrap();
    let (app, clock) = manual_app(dir.path());
    let f = create(&dir.path().join("foreign.ospider"), cfg());
    insert(&f, 1, 1);
    f.set_meta("state", "Running").unwrap();
    f.set_meta(
        "heartbeat_at",
        &os_model::Clock::now(clock.as_ref()).to_db(),
    )
    .unwrap();
    f.set_meta("owner_pid", "4294967294").unwrap();
    f.set_meta("owner_kind", "mcp-stdio").unwrap();
    let p = serde_json::json!({"state":"Running","end_reason":null,"crawled":12,"queued":20,"percent":37,"rate":2.5,"avg_response_time":0.05,"elapsed_s":30,"eta_s":8,"active_threads":2,"started_at":os_model::Clock::now(clock.as_ref()),"avg_rate":1.25,"paused_s":10,"discovered":32,"memory_stage":"warning","file_bytes":123456,"memory_used_bytes":800,"memory_limit_bytes":1000});
    f.set_meta("progress_snapshot", &p.to_string()).unwrap();
    let summary = open(&app, f.path()).await;
    assert!(!summary.owner.controllable);
    assert_eq!(summary.owner.kind, OwnerKind::Mcp);
    assert!(summary.owner.live);
    os_app::session::poll(&app).await.unwrap();
    let tick = app.assemble_tick().unwrap();
    let s = tick.crawl.unwrap();
    assert_eq!(s.crawled, 12);
    assert_eq!(s.avg_rate, 1.25);
    assert_eq!(s.memory_used_bytes, Some(800));
    assert_eq!(s.file_bytes, Some(123456));
    for cmd in [
        "crawl.pause",
        "crawl.resume",
        "crawl.stop",
        "analysis.start",
        "crawl.remove",
        "crawl.recrawl",
        "crawl.saveAs",
    ] {
        let args = match cmd {
            "analysis.start" => serde_json::json!({"steps":null}),
            "crawl.remove" | "crawl.recrawl" => {
                serde_json::json!({"selection":{"kind":"ids","ids":[1]}})
            }
            "crawl.saveAs" => serde_json::json!({"dest":dir.path().join("copy.ospider")}),
            _ => serde_json::Value::Null,
        };
        let err = app.dispatch(ctx(), cmd, args).await.unwrap_err();
        assert_eq!(err.code, os_app::ErrorCode::Unsupported, "{cmd}");
    }
    assert_eq!(
        f.get_meta("owner_pid").unwrap().as_deref(),
        Some("4294967294")
    );
    let write = app
        .write_queue()
        .submit_file(&key(&app), |_| Ok(()))
        .await
        .unwrap_err();
    assert_eq!(write.code, os_app::ErrorCode::NoCrawl);
    clock.advance(Duration::from_secs(11));
    os_app::session::poll(&app).await.unwrap();
    assert_eq!(
        app.assemble_tick().unwrap().crawl.unwrap().phase,
        CrawlPhase::Incomplete
    );
}

// spec: UI-018 [M2]
// spec: ONPAGE-193
#[tokio::test]
async fn counters_have_a_one_second_deadline_even_with_five_second_rows() {
    let dir = tempfile::tempdir().unwrap();
    let (app, clock) = manual_app(dir.path());
    let f = create(&dir.path().join("cadence.ospider"), cfg());
    f.set_meta("state", "Paused").unwrap();
    insert(&f, 1, 1);
    app.set_live_interval_ms(5000).unwrap();
    open(&app, f.path()).await;
    let changes = Arc::new(Mutex::new(Vec::new()));
    let captured = changes.clone();
    feed::subscribe(
        &app,
        Arc::new(move |_, c| captured.lock().unwrap().push(c.clone())),
    );
    let sink = Arc::new(os_app::events::CollectingSink::new());
    app.subscribe(window(), sink.clone());
    os_app::session::poll(&app).await.unwrap();
    app.flush_tick();
    assert_eq!(changes.lock().unwrap().len(), 1);
    insert(&f, 2, 2);
    for _ in 0..10 {
        clock.advance(Duration::from_millis(100));
        os_app::session::poll(&app).await.unwrap();
        app.flush_tick();
    }
    let events = sink.take();
    assert_eq!(events.len(), 1);
    let UiEvent::Tick { tick } = &events[0] else {
        panic!()
    };
    assert!(tick.counts_version >= 2);
    assert_eq!(
        changes.lock().unwrap().len(),
        1,
        "table cadence remains five seconds"
    );
    clock.advance(Duration::from_secs(4));
    os_app::session::poll(&app).await.unwrap();
    assert_eq!(changes.lock().unwrap().len(), 2);
    assert_eq!(changes.lock().unwrap()[1].ids, vec![os_model::UrlId(2)]);
}
