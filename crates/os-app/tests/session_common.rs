#![allow(dead_code, clippy::disallowed_types, clippy::disallowed_methods)]
use os_app::{dto::*, pool::CrawlKey, App, AppConfig, CallCtx, WindowId};
use os_model::{CrawlConfig, CrawlState, ManualClock, Timestamp};
use os_store::{CrawlFile, CrawlInit, StorageProfile};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
pub fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/ui/fx-ui-basic")
}
pub fn app(home: &Path) -> App {
    App::new(AppConfig::new(home)).unwrap()
}
pub fn manual_app(home: &Path) -> (App, Arc<ManualClock>) {
    let clock = Arc::new(ManualClock::new(Timestamp::from_unix_millis(
        1_800_000_000_000,
    )));
    let mut config = AppConfig::new(home);
    config.clock = clock.clone();
    (App::new(config).unwrap(), clock)
}
pub fn ctx() -> CallCtx {
    CallCtx::for_window("main")
}
pub fn window() -> WindowId {
    WindowId("main".into())
}
pub async fn call(app: &App, name: &str, args: serde_json::Value) -> serde_json::Value {
    app.dispatch(ctx(), name, args)
        .await
        .unwrap_or_else(|e| panic!("{name}: {e:?}"))
}
pub fn cfg() -> CrawlConfig {
    let mut c = CrawlConfig::default();
    c.analysis.auto_at_end = false;
    c.speed.threads = 2;
    c
}
pub fn create(path: &Path, config: CrawlConfig) -> CrawlFile {
    let id = os_model::IdSource::new_crawl_id(
        &os_model::RandomIds,
        Timestamp::from_unix_millis(1_800_000_000_000),
    );
    CrawlFile::create(
        path,
        &CrawlInit {
            crawl_id: id,
            mode: os_model::CrawlMode::Spider,
            start_url: Some("https://fixture.test/".into()),
            list_source: None,
            project: String::new(),
            name: "K04 fixture".into(),
            config,
            created_at: Timestamp::from_unix_millis(1_800_000_000_000),
        },
        StorageProfile::Balanced,
    )
    .unwrap()
}
pub async fn open(app: &App, path: &Path) -> CrawlSummaryDto {
    serde_json::from_value(
        call(
            app,
            "crawl.open",
            serde_json::json!({"kind":"path","path":path}),
        )
        .await,
    )
    .unwrap()
}
pub fn key(app: &App) -> CrawlKey {
    app.sessions().current(&window()).unwrap().key
}
pub async fn operation(app: &App, name: &str, args: serde_json::Value) -> OpStatus {
    let op: OpHandle = serde_json::from_value(call(app, name, args).await).unwrap();
    app.ops()
        .wait(&op.op_id)
        .await
        .expect("operation result retained before next tick")
}
pub async fn completed(app: &App) {
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            os_app::session::poll(app).await.unwrap();
            if app
                .sessions()
                .current(&window())
                .is_some_and(|s| s.state == CrawlState::Completed)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
}
pub fn request(url: String) -> StartRequest {
    StartRequest {
        source: StartSource::Spider { url },
        scope: StartScope::Domain,
        profile: None,
    }
}
pub fn insert(file: &CrawlFile, id: i64, seq: u64) {
    file.conn().execute("INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,folder_depth,url_length) VALUES (?1,?2,?2,'https://fixture.test',1,'done','link',0,30)",rusqlite::params![id,format!("https://fixture.test/{id}")]).unwrap();
    file.conn()
        .execute(
            "UPDATE urls SET change_seq=?1 WHERE id=?2",
            rusqlite::params![seq, id],
        )
        .unwrap();
    file.set_meta("change_seq", &seq.to_string()).unwrap();
}
