//! Operationen (M2-F03): `OpHandle` sofort, Fortschritt und Ergebnis über den Tick, Abbruch ≤ 100 ms über den
//! SQLite-Interrupt (UI-141), `op.cancel`.
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]

#[path = "runtime_common.rs"]
mod common;

use std::time::{Duration, Instant};

use common::*;
use os_app::dto::{OpState, UiEvent};
use os_app::events::CollectingSink;
use os_app::pool::CrawlKey;
use os_app::{ErrorCode, WindowId};
use os_store::StoreError;
use serde_json::json;

fn key(s: &str) -> CrawlKey {
    CrawlKey(s.to_string())
}

// spec: UI-141
#[tokio::test]
async fn spawn_returns_the_handle_at_once_and_the_result_arrives_in_the_tick() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    let sink = std::sync::Arc::new(CollectingSink::new());
    app.subscribe(WindowId("main".into()), sink.clone());

    let (release_tx, release_rx) = tokio::sync::oneshot::channel::<()>();
    let handle = app.ops().spawn("op-test", true, |_cancel, progress| {
        Box::pin(async move {
            progress.report(1, Some(4));
            let _ = release_rx.await;
            progress.report(4, Some(4));
            Ok(json!({ "rows": 4 }))
        })
    });
    assert_eq!(handle.label_key, "op-test");
    assert!(handle.cancellable);
    assert!(!handle.op_id.is_empty());

    // erster Tick: die Operation läuft mit Fortschritt
    assert!(
        eventually(Duration::from_secs(2), || app
            .ops()
            .status(&handle.op_id)
            .is_some_and(|s| s.done == 1))
        .await
    );
    assert_eq!(app.flush_tick(), 1);
    let first = match sink.take().remove(0) {
        UiEvent::Tick { tick } => tick,
        other => panic!("{other:?}"),
    };
    assert_eq!(first.seq, 1);
    let op = first.ops.iter().find(|o| o.op_id == handle.op_id).unwrap();
    assert_eq!(
        (op.state, op.done, op.total),
        (OpState::Running, 1, Some(4))
    );
    assert!(op.result.is_none());

    // ohne Änderung kein Tick
    assert_eq!(app.flush_tick(), 0);
    assert!(sink.is_empty());

    release_tx.send(()).unwrap();
    app.ops().wait(&handle.op_id).await.unwrap();
    assert_eq!(app.flush_tick(), 1);
    let second = match sink.take().remove(0) {
        UiEvent::Tick { tick } => tick,
        other => panic!("{other:?}"),
    };
    assert_eq!(second.seq, 2);
    let op = second.ops.iter().find(|o| o.op_id == handle.op_id).unwrap();
    assert_eq!(op.state, OpState::Done);
    assert_eq!(op.result, Some(json!({ "rows": 4 })));
    assert_eq!(op.done, 4);

    // die gemeldete Operation ist aus der Verwaltung verschwunden
    assert_eq!(app.ops().running(), 0);
    assert!(app.ops().status(&handle.op_id).is_none());
    assert_eq!(app.flush_tick(), 0);
}

// spec: UI-141
#[tokio::test]
async fn failures_and_panics_end_as_failed_with_the_error() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    let failing = app.ops().spawn("op-fail", false, |_c, _p| {
        Box::pin(async {
            Err(os_app::CmdError::new(
                ErrorCode::Io,
                "err-export-dir-not-writable",
            ))
        })
    });
    let panicking = app.ops().spawn("op-panic", false, |_c, _p| {
        Box::pin(async { panic!("kaputt") })
    });
    let early_panic = app
        .ops()
        .spawn("op-panic-early", false, |_c, _p| panic!("sofort"));

    let f = app.ops().wait(&failing.op_id).await.unwrap();
    assert_eq!(f.state, OpState::Failed);
    assert_eq!(f.error.unwrap().message_key, "err-export-dir-not-writable");
    for h in [&panicking, &early_panic] {
        let p = app.ops().wait(&h.op_id).await.unwrap();
        assert_eq!(p.state, OpState::Failed);
        assert_eq!(p.error.unwrap().code, ErrorCode::Internal);
    }
}

// spec: UI-141
#[tokio::test]
async fn cancel_stops_a_cooperative_operation() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    let h = app.ops().spawn("op-coop", true, |cancel, _p| {
        Box::pin(async move {
            while !cancel.is_cancelled() {
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
            Err(os_app::CmdError::new(ErrorCode::Cancelled, "err-cancelled"))
        })
    });
    assert!(app.ops().cancel(&h.op_id));
    let s = app.ops().wait(&h.op_id).await.unwrap();
    assert_eq!(s.state, OpState::Cancelled);
    assert!(s.error.is_none());
    // eine beendete Operation lässt sich nicht noch einmal abbrechen
    assert!(!app.ops().cancel(&h.op_id));
    assert!(!app.ops().cancel("unbekannt"));
}

// spec: UI-141
#[tokio::test]
async fn a_database_operation_is_cancelled_within_100_ms_by_the_sqlite_interrupt() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    app.register_crawl(key("a"), make_crawl(dir.path(), "a.ospider"));
    let mut samples = Vec::new();
    for _ in 0..5 {
        let pool = app.read_pool().clone();
        let h = app.ops().spawn("op-sql", true, move |_cancel, _p| {
            Box::pin(async move {
                // die Marke der Operation gilt im Pool ohne Weiterreichen (Task-Kontext)
                let n = pool
                    .run(key("a"), |file, _c| {
                        file.conn()
                            .query_row(SLOW_SQL, [], |r| r.get::<_, i64>(0))
                            .map_err(StoreError::from)
                    })
                    .await?;
                Ok(json!(n))
            })
        });
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(app.ops().status(&h.op_id).unwrap().state, OpState::Running);

        let started = Instant::now();
        assert!(app.ops().cancel(&h.op_id));
        let s = app.ops().wait(&h.op_id).await.unwrap();
        samples.push(started.elapsed());
        assert_eq!(s.state, OpState::Cancelled);
    }
    assert_cancel_latency(samples);
}

// spec: UI-141
#[tokio::test]
async fn op_cancel_command_maps_unknown_and_non_cancellable_operations() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    let fixed = app.ops().spawn("op-fixed", false, |_c, _p| {
        Box::pin(async {
            tokio::time::sleep(Duration::from_millis(200)).await;
            Ok(json!(1))
        })
    });
    let e = app
        .dispatch(ctx(), "op.cancel", json!({ "opId": fixed.op_id }))
        .await
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Unsupported);
    assert_eq!(e.message_key, "err-op-not-cancellable");

    let e = app
        .dispatch(ctx(), "op.cancel", json!({ "opId": "gibt-es-nicht" }))
        .await
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound);

    let e = app
        .dispatch(ctx(), "op.cancel", json!({}))
        .await
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidInput);

    let running = app.ops().spawn("op-run", true, |cancel, _p| {
        Box::pin(async move {
            while !cancel.is_cancelled() {
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
            Err(os_app::CmdError::new(ErrorCode::Cancelled, "err-cancelled"))
        })
    });
    let ok = app
        .dispatch(ctx(), "op.cancel", json!({ "opId": running.op_id }))
        .await
        .unwrap();
    assert!(ok.is_null());
    assert_eq!(
        app.ops().wait(&running.op_id).await.unwrap().state,
        OpState::Cancelled
    );
}

// spec: UI-141
#[tokio::test]
async fn cancel_all_stops_every_cancellable_operation_and_op_ids_are_deterministic() {
    let dir = tempfile::tempdir().unwrap();
    let a = new_app(dir.path());
    let dir2 = tempfile::tempdir().unwrap();
    let b = new_app(dir2.path());
    let spawn = |app: &os_app::App| {
        app.ops().spawn("op-x", true, |cancel, _p| {
            Box::pin(async move {
                while !cancel.is_cancelled() {
                    tokio::time::sleep(Duration::from_millis(2)).await;
                }
                Err(os_app::CmdError::new(ErrorCode::Cancelled, "err-cancelled"))
            })
        })
    };
    let (h1, h2) = (spawn(&a), spawn(&a));
    assert_ne!(h1.op_id, h2.op_id);
    // gleiche Quelle der Kennungen → gleiche Folge
    assert_eq!(spawn(&b).op_id, h1.op_id);
    assert_eq!(a.ops().cancel_all(), 2);
    for id in [&h1.op_id, &h2.op_id] {
        assert_eq!(a.ops().wait(id).await.unwrap().state, OpState::Cancelled);
    }
    b.ops().cancel_all();
}
