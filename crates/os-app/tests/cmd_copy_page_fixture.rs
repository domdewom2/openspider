// Shared fixture runner for the private view page counter test. Included by view.rs;
// as an integration-test target this file only provides the fixture helper.
use os_app::{
    dto::CmdError, pool::CrawlKey, runtime::sessions::SessionInfo, App, AppConfig, CallCtx,
    WindowId,
};
use os_model::{CrawlConfig, CrawlId, CrawlMode, CrawlState, Timestamp};
use serde_json::json;
use std::sync::{Arc, Mutex};

#[allow(clippy::disallowed_types, clippy::disallowed_methods)]
pub async fn assert_copy_uses_one_page(page_reads: impl Fn(&App) -> usize) {
    use os_store::{CrawlFile, CrawlInit, StorageProfile};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("copy.ospider");
    let file = CrawlFile::create(
        &path,
        &CrawlInit {
            crawl_id: "00000000-0000-0000-0000-000000000001"
                .parse::<CrawlId>()
                .unwrap(),
            mode: CrawlMode::Spider,
            start_url: Some("https://site.test/".into()),
            list_source: None,
            project: "copy".into(),
            name: "copy".into(),
            config: CrawlConfig::default(),
            created_at: Timestamp::UNIX_EPOCH,
        },
        StorageProfile::Balanced,
    )
    .unwrap();
    file.conn()
        .execute_batch(include_str!(
            "../../../apps/desktop/src/ipc/__fixtures__/k06/fx-ui-basic/scenario.sql"
        ))
        .unwrap();
    file.conn().execute_batch("WITH RECURSIVE n(x) AS (SELECT 10 UNION ALL SELECT x+1 FROM n WHERE x<3010) INSERT INTO urls(id,url,address,origin,is_internal,in_tables,state,discovered_via,folder_depth,url_length) SELECT x,'https://site.test/'||x,'https://site.test/'||x,'https://site.test',1,1,'done','link',0,25 FROM n").unwrap();
    drop(file);
    let app = App::new(
        AppConfig::new(dir.path())
            .with_ids(Arc::new(os_model::SeededIds::new(7)))
            .with_clock(Arc::new(os_model::ManualClock::new(Timestamp::UNIX_EPOCH))),
    )
    .unwrap();
    let key = CrawlKey("copy".into());
    app.register_crawl(key.clone(), path.clone());
    app.sessions().set_current(
        WindowId("main".into()),
        SessionInfo {
            key: key.clone(),
            path,
            config: Arc::new(CrawlConfig::default()),
            state: CrawlState::Completed,
            writable: true,
            engine: None,
        },
    );
    #[derive(Default)]
    struct Clipboard(Mutex<String>);
    impl os_app::runtime::hooks::DesktopActions for Clipboard {
        fn write_clipboard(&self, text: &str) -> Result<(), CmdError> {
            *self.0.lock().unwrap() = text.into();
            Ok(())
        }
        fn reveal_paths(&self, _: &[std::path::PathBuf]) -> Result<(), CmdError> {
            Ok(())
        }
    }
    let clipboard = Arc::new(Clipboard::default());
    app.set_desktop_actions(clipboard.clone());
    let ctx = CallCtx::for_window("main");
    // Exercise the asynchronous handle even when the fixture builds quickly.
    let readers = app.read_pool().exclusive(&key).await;
    let mut h = app.dispatch(ctx.clone(), "view.open", json!({"spec":{"kind":"tab","tab":"Internal","filter":"All","sort":[{"column":"Address","dir":"desc"}]},"columns":["Address"]})).await.unwrap();
    assert_eq!(h["state"], "building");
    drop(readers);
    let build = app
        .ops()
        .wait(h["op"]["opId"].as_str().unwrap())
        .await
        .unwrap();
    assert_eq!(build.state, os_app::dto::OpState::Done, "{build:?}");
    h = build.result.unwrap();
    assert_eq!(h["state"], "ready");
    assert!(h["total"].as_u64().unwrap() > 2000);
    let before = page_reads(&app);
    let result = app.dispatch(ctx, "view.copy", json!({"viewId":h["viewId"],"selection":{"kind":"ids","ids":[20,30,20,6]},"columns":["Address"],"withHeader":false})).await.unwrap();
    assert_eq!(result["rows"], 2);
    assert_eq!(page_reads(&app) - before, 1);
    let eol = if cfg!(windows) { "\r\n" } else { "\n" };
    assert_eq!(
        *clipboard.0.lock().unwrap(),
        format!("https://site.test/30{eol}https://site.test/20{eol}")
    );
}
