//! Shared fx-ui-basic store fixture. The source and generated DTO goldens live in K06's owned IPC fixture tree.
#![allow(dead_code, clippy::disallowed_types, clippy::disallowed_methods)]
use os_app::{
    dto::{CmdError, FileResult, OpHandle, OpState},
    pool::CrawlKey,
    runtime::{hooks::DesktopActions, sessions::SessionInfo},
    App, AppConfig, CallCtx,
};
use os_model::{CrawlConfig, CrawlState, ManualClock, SeededIds, Timestamp};
use os_store::{CrawlFile, OpenMode};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
#[path = "runtime_common.rs"]
mod common;
pub struct Fx {
    pub dir: tempfile::TempDir,
    pub app: App,
    pub path: PathBuf,
    pub clipboard: Arc<Clipboard>,
}
#[derive(Default)]
pub struct Clipboard(pub Mutex<String>);
impl DesktopActions for Clipboard {
    fn write_clipboard(&self, text: &str) -> Result<(), CmdError> {
        *self.0.lock().unwrap() = text.into();
        Ok(())
    }
    fn reveal_paths(&self, _: &[PathBuf]) -> Result<(), CmdError> {
        Ok(())
    }
}
impl Fx {
    pub fn new() -> Self {
        Self::config(CrawlConfig::default())
    }
    pub fn config(mut cfg: CrawlConfig) -> Self {
        cfg.extraction.http_headers = true;
        let dir = tempfile::tempdir().unwrap();
        let path = common::make_crawl(dir.path(), "basic.ospider");
        let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
        file.conn()
            .execute_batch(include_str!(
                "../../../apps/desktop/src/ipc/__fixtures__/k06/fx-ui-basic/scenario.sql"
            ))
            .unwrap();
        for id in 1..=8 {
            let c = os_store::counters::recompute(file.conn(), id).unwrap();
            file.conn().execute("UPDATE url_counters SET inlinks=?2,unique_inlinks=?3,self_inlinks=?4,inlinks_all=?5,outlinks=?6,unique_outlinks=?7,self_outlinks=?8,external_outlinks=?9,unique_external_outlinks=?10,img_inlinks=?11,html_2xx_sources=?12,image_link_inlinks=?13 WHERE url_id=?1",rusqlite::params![id,c.inlinks,c.unique_inlinks,c.self_inlinks,c.inlinks_all,c.outlinks,c.unique_outlinks,c.self_outlinks,c.external_outlinks,c.unique_external_outlinks,c.img_inlinks,c.html_2xx_sources,c.image_link_inlinks]).unwrap();
        }
        drop(file);
        let app = App::new(
            AppConfig::new(dir.path())
                .with_ids(Arc::new(SeededIds::new(7)))
                .with_clock(Arc::new(ManualClock::new(Timestamp::UNIX_EPOCH))),
        )
        .unwrap();
        let key = CrawlKey("fx-ui-basic".into());
        app.register_crawl(key.clone(), path.clone());
        app.sessions().set_current(
            os_app::WindowId("main".into()),
            SessionInfo {
                key,
                path: path.clone(),
                config: Arc::new(cfg),
                state: CrawlState::Completed,
                writable: true,
                engine: None,
            },
        );
        let clipboard = Arc::new(Clipboard::default());
        app.set_desktop_actions(clipboard.clone());
        Self {
            dir,
            app,
            path,
            clipboard,
        }
    }
    pub async fn call(&self, name: &str, args: Value) -> Value {
        self.app
            .dispatch(CallCtx::for_window("main"), name, args)
            .await
            .unwrap_or_else(|e| panic!("{name}: {e:?}"))
    }
    pub fn write(&self, sql: &str) {
        let file = CrawlFile::open(&self.path, OpenMode::ReadWrite).unwrap();
        file.conn().execute_batch(sql).unwrap();
    }
    pub async fn open(&self, spec: Value, columns: Value) -> Value {
        self.call("view.open", json!({"spec":spec,"columns":columns}))
            .await
    }
    pub async fn rows(&self, h: &Value, offset: u64, limit: u32, columns: Value) -> Value {
        self.call("view.rows",json!({"viewId":h["viewId"],"version":h["version"],"offset":offset,"limit":limit,"columns":columns})).await
    }
    pub async fn op(&self, name: &str, args: Value) -> Result<FileResult, CmdError> {
        let h: OpHandle = serde_json::from_value(self.call(name, args).await).unwrap();
        let start = std::time::Instant::now();
        loop {
            let status = self.app.ops().status(&h.op_id).unwrap();
            match status.state {
                OpState::Done => return Ok(serde_json::from_value(status.result.unwrap()).unwrap()),
                OpState::Failed => return Err(status.error.unwrap()),
                OpState::Cancelled => {
                    return Err(CmdError::new(
                        os_app::dto::ErrorCode::Cancelled,
                        "err-cancelled",
                    ))
                }
                _ => {}
            }
            assert!(
                start.elapsed() < Duration::from_secs(20),
                "operation timeout: {status:?}"
            );
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    }
}
pub fn tab() -> Value {
    json!({"kind":"tab","tab":"Internal","filter":"All","search":null,"sort":[],"segment":null,"compare":null})
}
pub fn links(incoming: bool) -> Value {
    json!({"kind":if incoming{"inlinks"}else{"outlinks"},"urlIds":if incoming{vec![2]}else{vec![1]},"filter":{"id":"All"},"search":null,"sort":[],"compare":null})
}
pub fn golden(name: &str, value: &Value) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/desktop/src/ipc/__fixtures__/k06/fx-ui-basic")
        .join(name);
    if std::env::var_os("UPDATE_FIXTURES").is_some() {
        std::fs::write(
            &path,
            format!("{}\n", serde_json::to_string_pretty(value).unwrap()),
        )
        .unwrap();
    }
    let expected: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(*value, expected, "{name}");
}

impl Default for Fx {
    fn default() -> Self {
        Self::new()
    }
}
