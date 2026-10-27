#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
use super::*;
use crate::dto::{OpHandle, OpState};
use crate::pool::CrawlKey;
use crate::runtime::sessions::SessionInfo;
use crate::AppConfig;
use os_model::{CrawlMode, CrawlState, IdSource, SeededIds, Timestamp};
use os_store::{CrawlFile, CrawlInit, OpenMode, StorageProfile};
use serde_json::json;
use std::sync::Arc;

async fn run_export(app: &App, out: &Path) -> FileResult {
    let handle: OpHandle = serde_json::from_value(
        app.dispatch(
            CallCtx::for_window("main"),
            "export.bulk",
            json!({"name":"Web:Screenshots","dir":out,"format":"csv","singleWorkbook":false}),
        )
        .await
        .unwrap(),
    )
    .unwrap();
    let start = std::time::Instant::now();
    loop {
        let status = app.ops().status(&handle.op_id).unwrap();
        if status.state == OpState::Done {
            return serde_json::from_value(status.result.unwrap()).unwrap();
        }
        assert_eq!(status.state, OpState::Running, "{status:?}");
        assert!(start.elapsed() < std::time::Duration::from_secs(20));
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
    }
}

// spec: OUT-043
#[tokio::test]
async fn app_screenshot_export_replaces_a_larger_mixed_capture_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fixture.ospider");
    let mut cfg = CrawlConfig::default();
    cfg.rendering.mode = os_model::RenderingMode::JavaScript;
    cfg.rendering.screenshots.enabled = true;
    let at = Timestamp::UNIX_EPOCH;
    let file = CrawlFile::create(
        &path,
        &CrawlInit {
            crawl_id: SeededIds::new(2).new_crawl_id(at),
            mode: CrawlMode::Spider,
            start_url: Some("https://site.test/".into()),
            list_source: None,
            project: "Screenshot".into(),
            name: "Fixture".into(),
            config: cfg.clone(),
            created_at: at,
        },
        StorageProfile::Balanced,
    )
    .unwrap();
    file.conn()
        .execute_batch(include_str!("facts.sql"))
        .unwrap();
    let png = include_bytes!("screenshot.png");
    let jpeg = include_bytes!("screenshot.jpg");
    for (id, bytes) in [
        (1, png.as_slice()),
        (2, jpeg.as_slice()),
        (3, png.as_slice()),
    ] {
        file.conn()
            .execute(
                "INSERT INTO screenshots(url_id,tile,png,width,height) VALUES (?1,0,?2,32,24)",
                (id, bytes),
            )
            .unwrap();
    }
    drop(file);
    let app = App::new(AppConfig::new(dir.path())).unwrap();
    let key = CrawlKey("screenshots".into());
    app.register_crawl(key.clone(), path.clone());
    app.sessions().set_current(
        crate::WindowId("main".into()),
        SessionInfo {
            key,
            path: path.clone(),
            config: Arc::new(cfg),
            state: CrawlState::Completed,
            writable: true,
            engine: None,
        },
    );
    let out = dir.path().join("export");
    let first = run_export(&app, &out).await;
    assert_eq!(std::fs::read_dir(&first.path).unwrap().count(), 4);
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    file.conn()
        .execute("DELETE FROM screenshots WHERE url_id=3", [])
        .unwrap();
    drop(file);
    let second = run_export(&app, &out).await;
    assert_eq!(second.path, first.path);
    let folder = Path::new(&second.path);
    let index = std::fs::read_to_string(folder.join("index.csv")).unwrap();
    assert_eq!(index.lines().count(), 3);
    assert_eq!(std::fs::read_dir(folder).unwrap().count(), 3);
    let total: u64 = std::fs::read_dir(folder)
        .unwrap()
        .map(|e| e.unwrap().metadata().unwrap().len())
        .sum();
    assert_eq!(second.bytes, total);
    for entry in std::fs::read_dir(folder).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|ext| ext == "png") {
            assert!(std::fs::read(path)
                .unwrap()
                .starts_with(b"\x89PNG\r\n\x1a\n"));
        }
    }
    assert_eq!(std::fs::read_dir(&out).unwrap().count(), 1);
}
