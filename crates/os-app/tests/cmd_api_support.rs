// Gemeinsames Hilfsmodul für cmd_api_* und pending_w1_i01 via #[path = "cmd_api_support.rs"].
// Fixture-Aufbau und Assertions prüfen die gespeicherten Fakten direkt.
#![allow(
    dead_code,
    unused_imports,
    clippy::disallowed_methods,
    clippy::disallowed_types
)]
pub use os_app::{
    commands::api::ApiState, pool::CrawlKey, runtime::sessions::SessionInfo, App, AppConfig,
    CallCtx, ErrorCode,
};
pub use os_integrations::{
    credentials::{Credential, Credentials},
    ApiCtx, ApiError, ApiSource, ApiStatus, UrlRef,
};
pub use os_model::{
    ApiKind, ApiValueWrite, CrawlConfig, CrawlMode, CrawlState, IdSource, SeededIds, Timestamp,
};
pub use os_store::{CrawlFile, CrawlInit, StorageProfile};
pub use os_testsite::mocks::{common::FixtureApi, MockServer};
pub use serde_json::{json, Value};
pub use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
pub fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/integ-api/framework.json"
    ))
    .unwrap()
}
pub struct Harness {
    pub app: App,
    pub _dir: tempfile::TempDir,
    pub key: CrawlKey,
    pub mock: Arc<FixtureApi>,
    pub server: MockServer,
}
impl Harness {
    pub async fn new() -> Self {
        Self::new_with_registry(os_app::default_registry()).await
    }
    pub async fn new_with_registry(registry: os_app::Registry) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let mock = Arc::new(FixtureApi::default());
        let server = MockServer::start(vec![mock.clone()]).await.unwrap();
        let mut cfg = CrawlConfig::default();
        cfg.api.psi.base_url = Some(server.base_url("common"));
        cfg.api.ga4.base_url = Some(server.base_url("common"));
        cfg.api.ga4.oauth_client_id = Some("fixture-client".into());
        let now = Timestamp::from_db("2026-10-18T12:00:00Z").unwrap();
        let path = dir.path().join("fixture.ospider");
        let init = CrawlInit {
            crawl_id: SeededIds::new(1).new_crawl_id(now),
            mode: CrawlMode::Spider,
            start_url: Some(fixture()["urls"][0].as_str().unwrap().into()),
            list_source: None,
            project: "API fixture".into(),
            name: "API fixture".into(),
            config: cfg.clone(),
            created_at: now,
        };
        let file = CrawlFile::create(&path, &init, StorageProfile::default()).unwrap();
        for (i, url) in fixture()["urls"].as_array().unwrap().iter().enumerate() {
            let url = url.as_str().unwrap();
            file.conn().execute("INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,folder_depth,url_length) VALUES(?1,?2,?2,'https://integ.test:8080',1,'fetched','seed',0,?3)",rusqlite::params![i as i64+1,url,url.len()]).unwrap();
        }
        let mut app_config = AppConfig::new(dir.path());
        app_config.clock = Arc::new(os_model::ManualClock::new(now));
        app_config.ids = Arc::new(SeededIds::new(42));
        let app = App::with_registry(app_config, registry).unwrap();
        let key = CrawlKey("api-fixture".into());
        app.register_crawl(key.clone(), path.clone());
        app.sessions().set_current(
            os_app::WindowId("main".into()),
            SessionInfo {
                key: key.clone(),
                path,
                config: Arc::new(cfg),
                state: CrawlState::Paused,
                writable: true,
                engine: None,
            },
        );
        Self {
            app,
            _dir: dir,
            key,
            mock,
            server,
        }
    }
    pub async fn call(&self, name: &str, args: Value) -> Result<Value, os_app::CmdError> {
        self.app
            .dispatch(CallCtx::for_window("main"), name, args)
            .await
    }
    pub async fn op(&self, name: &str, args: Value) -> Result<Value, os_app::CmdError> {
        let handle = self.call(name, args).await?;
        // 15 s were too tight on loaded Windows runners (cmd_api_support.rs:109, K2-64); the operations finish in < 1 s locally.
        let status = tokio::time::timeout(
            Duration::from_secs(90),
            self.app.ops().wait(handle["opId"].as_str().unwrap()),
        )
        .await
        .unwrap()
        .unwrap();
        if let Some(error) = status.error {
            Err(error)
        } else {
            Ok(status.result.unwrap())
        }
    }
    pub async fn connect(&self) {
        self.op(
            "api.connect",
            json!({"kind":"psi","apiKey":fixture()["secret"],"remember":true}),
        )
        .await
        .unwrap();
    }
    pub fn source(&self, kind: ApiKind) -> Arc<Source> {
        let source = Arc::new(Source {
            kind,
            value: AtomicU64::new(42),
            fail: AtomicBool::new(false),
            calls: AtomicU64::new(0),
        });
        self.app.ext::<ApiState>().register_source(source.clone());
        source
    }
    pub async fn connect_ga4(&self) {
        let credentials = Credentials::new(self.app.config().secrets.clone());
        credentials
            .put(
                ApiKind::Ga4,
                Credential {
                    account_id: "fixture-google".into(),
                    label: "Fixture Google".into(),
                    api_key: None,
                    access_token: Some("good".into()),
                    refresh_token: Some("fixture-refresh".into()),
                    expires_at: Some(Timestamp::from_db("2027-10-18T12:00:00Z").unwrap()),
                    client_secret: None,
                    scopes: vec![],
                },
                true,
            )
            .unwrap();
        self.op(
            "api.connect",
            json!({"kind":"ga4","accountId":"fixture-google"}),
        )
        .await
        .unwrap();
    }
}
pub struct Source {
    pub kind: ApiKind,
    pub value: AtomicU64,
    pub fail: AtomicBool,
    pub calls: AtomicU64,
}
impl ApiSource for Source {
    fn kind(&self) -> ApiKind {
        self.kind
    }
    fn status(&self) -> ApiStatus {
        ApiStatus {
            kind: self.kind,
            state: os_integrations::ApiState::Idle,
            urls_total: 0,
            urls_done: 0,
            urls_failed: 0,
            detail: None,
        }
    }
    fn fetch<'a>(
        &'a self,
        urls: &'a [UrlRef],
        _: &'a ApiCtx,
    ) -> os_integrations::BoxFuture<'a, Result<Vec<ApiValueWrite>, ApiError>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail.load(Ordering::SeqCst) {
                return Err(ApiError::Transport("fixture failure".into()));
            }
            Ok(urls
                .iter()
                .map(|u| ApiValueWrite {
                    url_id: u.url_id,
                    url: u.url.clone(),
                    source: self.kind.to_string(),
                    metric: "fixture.score".into(),
                    level: "url".into(),
                    v_real: Some(self.value.load(Ordering::SeqCst) as f64),
                    v_text: None,
                    fetched_at: Timestamp::UNIX_EPOCH,
                })
                .collect())
        })
    }
}
