//! M2-N25 (Befund 5, CI windows-x64 auf fdcf9578, `pending_w1_o02`): `crawl.resume` direkt nach `crawl.open` eines
//! duplizierten pausierten Crawls scheiterte mit „database is locked“.
//!
//! Ursache: Das Duplikat entsteht mit `VACUUM INTO` und liegt im Rollback-Journal-Modus. Erst der erste Schreiber
//! (beim Fortsetzen die Engine) stellt auf WAL um und braucht dafür die Datei exklusiv. Halten die Leser der eben
//! geöffneten Sitzung eine Lesetransaktion (z. B. die Overview-Vorberechnung), wartet er den Busy-Timeout ab und
//! scheitert. Der Test hält eine solche Lesetransaktion deterministisch, während fortgesetzt wird.
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "runtime_common.rs"]
mod common;
use os_app::{dto::OpState, CallCtx};
use os_store::{registry, CrawlFile, OpenMode};
use serde_json::json;

// spec: CRAWL-126
// spec: CRAWL-143
#[tokio::test]
async fn resume_right_after_opening_a_duplicate_is_not_blocked_by_a_reader() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    std::fs::create_dir_all(&app.config().paths.crawls).unwrap();
    let id = common::crawl_id(1);
    let path = common::make_crawl(&app.config().paths.crawls, &registry::file_name(&id));
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    file.set_meta("state", "Paused").unwrap();
    drop(file);
    let op = app
        .dispatch(
            common::ctx(),
            "crawls.duplicate",
            json!({"ids":[id.to_string()]}),
        )
        .await
        .unwrap();
    let status = app.ops().wait(op["opId"].as_str().unwrap()).await.unwrap();
    assert_eq!(status.state, OpState::Done, "{:?}", status.error);
    let list = app
        .dispatch(common::ctx(), "crawls.list", json!({"show":"all"}))
        .await
        .unwrap();
    let copied_id = list
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["crawl"]["crawlId"] != id.to_string())
        .unwrap()["crawl"]["crawlId"]
        .as_str()
        .unwrap()
        .to_string();
    let copied = registry::find(&app.config().paths.crawls, &copied_id.parse().unwrap()).unwrap();
    app.dispatch(
        common::ctx(),
        "crawl.open",
        json!({"kind":"id","crawlId":copied_id}),
    )
    .await
    .unwrap();

    // A reader of the opened crawl holds a read transaction (as the overview precomputation does).
    let reader =
        rusqlite::Connection::open_with_flags(&copied, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    reader.execute_batch("BEGIN").unwrap();
    let _: i64 = reader
        .query_row("SELECT COUNT(*) FROM urls", [], |r| r.get(0))
        .unwrap();

    let resumed = app
        .dispatch(
            CallCtx::for_window("main"),
            "crawl.resume",
            serde_json::Value::Null,
        )
        .await;
    reader.execute_batch("ROLLBACK").unwrap();
    drop(reader);
    resumed.expect("resume is not blocked by a reader of the opened crawl");
    assert!(
        common::eventually(std::time::Duration::from_secs(10), || {
            app.sessions()
                .current(&os_app::WindowId("main".into()))
                .is_some_and(|s| s.state == os_model::CrawlState::Completed)
        })
        .await
    );
}
