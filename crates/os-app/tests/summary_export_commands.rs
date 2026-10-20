// Fixture-only database setup. Production commands use the read pool.
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
use os_app::{
    dto::{CustomSummaryMetric, OpHandle, OpState},
    pool::CrawlKey,
    runtime::sessions::SessionInfo,
    App, AppConfig, CallCtx, WindowId,
};
use os_integrations::{credentials::Credential, drive::SCOPE};
use os_model::{
    time::{IdSource, SeededIds},
    ApiKind, CrawlConfig, CrawlMode, CrawlState, Timestamp,
};
use os_store::{CrawlFile, CrawlInit, StorageProfile};
use os_testsite::mocks::{drive::DriveMock, MockServer};
use serde_json::json;
use std::{path::Path, sync::Arc};
fn fixture(app: &App, home: &Path, base: &str) {
    let at = Timestamp::from_db("2026-10-13T07:35:00Z").unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.api.drive.base_url = Some(base.into());
    let path = home.join("summary.ospider");
    let file = CrawlFile::create(
        &path,
        &CrawlInit {
            crawl_id: SeededIds::new(73).new_crawl_id(at),
            mode: CrawlMode::Spider,
            start_url: Some("https://site.test/".into()),
            list_source: None,
            project: "Project".into(),
            name: "Crawl".into(),
            config: cfg.clone(),
            created_at: at,
        },
        StorageProfile::Balanced,
    )
    .unwrap();
    file.conn()
        .execute_batch(include_str!(
            "../../../tests/fixtures/out-m2-export/crawl.sql"
        ))
        .unwrap();
    file.set_meta("started_at", &at.to_db()).unwrap();
    let key = CrawlKey("summary-fixture".into());
    app.register_crawl(key.clone(), &path);
    app.sessions().set_current(
        WindowId("main".into()),
        SessionInfo {
            key,
            path,
            config: Arc::new(cfg),
            state: CrawlState::Completed,
            writable: true,
            engine: None,
        },
    );
    os_app::commands::api::export_credentials(app)
        .put(
            ApiKind::Drive,
            Credential {
                account_id: "fixture".into(),
                label: "Google fixture".into(),
                api_key: None,
                access_token: Some("fixture-access".into()),
                refresh_token: None,
                expires_at: None,
                client_secret: None,
                scopes: vec![SCOPE.into()],
            },
            false,
        )
        .unwrap();
}
async fn start(app: &App, metrics: Vec<&str>) -> OpHandle {
    serde_json::from_value(
        app.dispatch(
            CallCtx::for_window("main"),
            "export.customSummary",
            json!({"account":"fixture","target":"Audit","metrics":metrics}),
        )
        .await
        .unwrap(),
    )
    .unwrap()
}
// spec: OUT-210
// spec: OUT-212
#[tokio::test]
async fn commands_measure_fixture_and_append_three_crawls_using_session_only_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let mock = DriveMock::default();
    let server = MockServer::start(vec![Arc::new(mock.clone())])
        .await
        .unwrap();
    let app = App::new(AppConfig::new(dir.path())).unwrap();
    fixture(&app, dir.path(), &server.base_url("drive"));
    let catalog: Vec<CustomSummaryMetric> = serde_json::from_value(
        app.dispatch(
            CallCtx::for_window("main"),
            "export.customSummaryMetrics",
            json!(null),
        )
        .await
        .unwrap(),
    )
    .unwrap();
    assert!(catalog.iter().any(|m| m.id == "Internal:HTML"));
    assert!(catalog.iter().any(|m| m.id == "Average Word Count"));
    for metrics in [
        vec!["Internal:HTML", "Response Codes:Client Error (4xx)"],
        vec!["Average Word Count", "Internal:HTML"],
        vec!["Internal:HTML"],
    ] {
        let handle = start(&app, metrics).await;
        assert!(handle.cancellable);
        let status = app.ops().wait(&handle.op_id).await.unwrap();
        assert_eq!(status.state, OpState::Done, "{:?}", status.error);
        assert!(status.result.unwrap()["path"]
            .as_str()
            .unwrap()
            .starts_with("https://docs.google.com/spreadsheets/d/"));
        assert_eq!(status.done, 1);
    }
    let files = mock.files();
    let file = files
        .iter()
        .find(|f| f.name == "Audit_custom_summary_report")
        .unwrap();
    let rows = &file.sheets[0].rows;
    assert_eq!(rows.len(), 4);
    assert_eq!(rows[0][2], "Internal:HTML");
    assert_eq!(rows[0][3], "Response Codes:Client Error (4xx)");
    assert_eq!(rows[0][4], "Average Word Count");
    assert!(rows[1][2].is_number());
    assert_eq!(rows[2][3], "");
    assert_eq!(rows[3][4], "");
}
// spec: OUT-210
#[tokio::test]
async fn validation_no_crawl_and_revoked_token_are_reported_without_secrets() {
    let dir = tempfile::tempdir().unwrap();
    let app = App::new(AppConfig::new(dir.path())).unwrap();
    assert!(app
        .dispatch(
            CallCtx::for_window("main"),
            "export.customSummary",
            json!({"account":"fixture","target":"","metrics":[]})
        )
        .await
        .is_err());
    assert!(app
        .dispatch(
            CallCtx::for_window("main"),
            "export.customSummary",
            json!({"account":"fixture","target":"Task","metrics":[]})
        )
        .await
        .is_err());
    let mock = DriveMock::default();
    let server = MockServer::start(vec![Arc::new(mock.clone())])
        .await
        .unwrap();
    fixture(&app, dir.path(), &server.base_url("drive"));
    mock.fail_next("GET", "/drive/v3/files", 401);
    let handle = start(&app, vec!["Internal:HTML"]).await;
    let status = app.ops().wait(&handle.op_id).await.unwrap();
    assert_eq!(status.state, OpState::Failed);
    let error = serde_json::to_string(&status.error).unwrap();
    assert!(error.contains("Google-Konto erneut verbinden"));
    assert!(!error.contains("fixture-access"));
}

// spec: OUT-210
#[test]
fn summary_resolves_winter_and_summer_at_crawl_start() {
    // Isolate the OS timezone from every other test; no shell or global TZ mutation.
    if std::env::var_os("X03_TIMEZONE_CHILD").is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "summary_resolves_winter_and_summer_at_crawl_start",
                "--nocapture",
            ])
            // Windows' CRT expects the POSIX abbreviation/offset form.
            // Both fixture zones give January UTC+1 and July UTC+2.
            .env(
                "TZ",
                if cfg!(windows) {
                    "GST-1GDT"
                } else {
                    "Europe/Berlin"
                },
            )
            .env("X03_TIMEZONE_CHILD", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    os_model::time::init_local_offset();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let dir = tempfile::tempdir().unwrap();
            let mock = DriveMock::default();
            let server = MockServer::start(vec![Arc::new(mock.clone())])
                .await
                .unwrap();
            let mut config = AppConfig::new(dir.path());
            // The application runs in summer while exporting both historical seasons.
            config.clock = Arc::new(os_model::time::FixedClock::new(
                Timestamp::from_db("2026-07-15T12:00:00Z").unwrap(),
                std::time::Duration::ZERO,
            ));
            let app = App::new(config).unwrap();
            fixture(&app, dir.path(), &server.base_url("drive"));
            for (started, expected) in [
                ("2026-01-15T12:00:00Z", "2026-01-15T13:00:00Z"),
                ("2026-07-15T12:00:00Z", "2026-07-15T14:00:00Z"),
            ] {
                let file = CrawlFile::open(
                    &dir.path().join("summary.ospider"),
                    os_store::OpenMode::ReadWrite,
                )
                .unwrap();
                file.set_meta("started_at", started).unwrap();
                drop(file);
                let handle = start(&app, vec!["Internal:HTML"]).await;
                let status = app.ops().wait(&handle.op_id).await.unwrap();
                assert_eq!(status.state, OpState::Done, "{:?}", status.error);
                let files = mock.files();
                let rows = &files
                    .iter()
                    .find(|f| f.name == "Audit_custom_summary_report")
                    .unwrap()
                    .sheets[0]
                    .rows;
                let actual = rows.last().unwrap()[0].as_f64().unwrap();
                let expected = 25569.0
                    + Timestamp::from_db(expected).unwrap().unix_millis() as f64 / 86_400_000.0;
                assert!(
                    (actual - expected).abs() < 1e-8,
                    "{started}: {actual} != {expected}"
                );
            }
        });
}

async fn command_op(app: &App, command: &str, args: serde_json::Value) -> os_app::dto::OpStatus {
    let handle: OpHandle = serde_json::from_value(
        app.dispatch(CallCtx::for_window("main"), command, args)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(handle.cancellable);
    app.ops().wait(&handle.op_id).await.unwrap()
}

// spec: OUT-005
#[tokio::test]
async fn drive_connect_lists_oauth_account_refreshes_exports_and_forgets_it() {
    use os_app::commands::api::{export_credentials, ApiState};
    use os_integrations::oauth;
    use os_testsite::mocks::oauth::OAuthMock;
    let directory = tempfile::tempdir().unwrap();
    let drive = DriveMock::default();
    let oauth = Arc::new(OAuthMock::default());
    let server = MockServer::start(vec![Arc::new(drive.clone()), oauth.clone()])
        .await
        .unwrap();
    let app = App::new(AppConfig::new(directory.path())).unwrap();
    fixture(&app, directory.path(), &server.base_url("drive"));
    // No pre-seeded account: export must use the account obtained via api.connect.
    export_credentials(&app)
        .remove(ApiKind::Drive, "fixture")
        .unwrap();
    let mut session = app.sessions().current(&WindowId("main".into())).unwrap();
    Arc::make_mut(&mut session.config).api.drive.oauth_client_id = Some("fixture-client".into());
    app.sessions().set_current(WindowId("main".into()), session);
    let base = server.base_url("oauth");
    let state = app.ext::<ApiState>();
    state.set_oauth_endpoints(oauth::Endpoints {
        authorize: format!("{base}/authorize"),
        token: format!("{base}/token"),
        revoke: format!("{base}/revoke"),
    });
    let browser = os_integrations::http::client(&Default::default()).unwrap();
    state.set_oauth_browser(Arc::new(move |url| {
        let request = browser.get(url).build().unwrap();
        let url = request.url();
        assert_eq!(
            url.query_pairs().find(|(k, _)| k == "scope").unwrap().1,
            SCOPE
        );
        assert_eq!(
            url.query_pairs()
                .find(|(k, _)| k == "code_challenge_method")
                .unwrap()
                .1,
            "S256"
        );
        let browser = browser.clone();
        tokio::spawn(async move {
            let response = browser.execute(request).await.unwrap();
            let callback = response.headers()["location"].to_str().unwrap();
            browser.get(callback).send().await.unwrap();
        });
        Ok(())
    }));
    let connected = command_op(
        &app,
        "api.connect",
        json!({"kind":"drive","remember":false}),
    )
    .await;
    assert_eq!(connected.state, OpState::Done, "{:?}", connected.error);
    let id = connected.result.unwrap()["accountId"]
        .as_str()
        .unwrap()
        .to_owned();
    let rows = app
        .dispatch(CallCtx::for_window("main"), "api.list", json!(null))
        .await
        .unwrap();
    let row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["kind"] == "drive")
        .unwrap();
    assert_eq!(row["auth"], "oauth");
    assert_eq!(row["connected"], true);
    assert_eq!(row["accounts"].as_array().unwrap().len(), 1);
    assert_eq!(row["accounts"][0]["accountId"], id);
    assert!(!rows.to_string().contains("fixture-access"));
    let result = command_op(
        &app,
        "export.customSummary",
        json!({
            "account":id,"target":"Connected","metrics":["Internal:HTML"]
        }),
    )
    .await;
    assert_eq!(result.state, OpState::Done, "{:?}", result.error);
    assert!(drive
        .files()
        .iter()
        .any(|f| f.name == "Connected_custom_summary_report"));
    app.dispatch(
        CallCtx::for_window("main"),
        "api.disconnect",
        json!({"kind":"drive"}),
    )
    .await
    .unwrap();
    let credentials = export_credentials(&app);
    let mut expired = credentials.list(ApiKind::Drive).unwrap().pop().unwrap();
    expired.expires_at = Some(Timestamp::UNIX_EPOCH);
    credentials.put(ApiKind::Drive, expired, false).unwrap();
    let refreshed = command_op(
        &app,
        "api.connect",
        json!({"kind":"drive","accountId":id,"remember":false}),
    )
    .await;
    assert_eq!(refreshed.state, OpState::Done, "{:?}", refreshed.error);
    assert_eq!(
        oauth
            .log
            .calls()
            .iter()
            .filter(|c| c.path == "token")
            .count(),
        2
    );
    app.dispatch(
        CallCtx::for_window("main"),
        "api.removeAccount",
        json!({"kind":"drive","accountId":id}),
    )
    .await
    .unwrap();
    assert!(credentials.list(ApiKind::Drive).unwrap().is_empty());
}

// spec: OUT-005
#[tokio::test]
async fn drive_connect_requires_own_client_id_before_opening_browser() {
    let directory = tempfile::tempdir().unwrap();
    let app = App::new(AppConfig::new(directory.path())).unwrap();
    app.ext::<os_app::commands::api::ApiState>()
        .set_oauth_browser(Arc::new(|_| {
            panic!("missing client ID must fail before opening the browser")
        }));
    let result = command_op(
        &app,
        "api.connect",
        json!({"kind":"drive","remember":false}),
    )
    .await;
    assert_eq!(result.state, OpState::Failed);
    let error = result.error.unwrap();
    assert_eq!(error.message_key, "err-api-client-id");
    assert!(serde_json::to_string(&error)
        .unwrap()
        .contains("Eigene Google-Client-ID erforderlich"));
}

// spec: OUT-005
#[tokio::test]
async fn sheets_command_consolidates_tabs_preserves_raw_text_and_reports_binary_skips() {
    let directory = tempfile::tempdir().unwrap();
    let mock = DriveMock::default();
    let server = MockServer::start(vec![Arc::new(mock.clone())])
        .await
        .unwrap();
    let app = App::new(AppConfig::new(directory.path())).unwrap();
    fixture(&app, directory.path(), &server.base_url("drive"));
    let file = CrawlFile::open(
        &directory.path().join("summary.ospider"),
        os_store::OpenMode::ReadWrite,
    )
    .unwrap();
    file.conn()
        .execute("UPDATE urls SET title_1 = '=1+1' WHERE id = 1", [])
        .unwrap();
    drop(file);
    let result = command_op(
        &app,
        "export.sheets",
        json!({
            "account":"fixture", "name":"Combined", "selections":[
                {"kind":"tab","name":"Internal:All"},
                {"kind":"tab","name":"Page Titles:All"},
                {"kind":"bulk","name":"Web:Screenshots"}
            ]
        }),
    )
    .await;
    assert_eq!(result.state, OpState::Done, "{:?}", result.error);
    let files = mock.files();
    let sheets: Vec<_> = files
        .iter()
        .filter(|f| f.mime == os_integrations::drive::SPREADSHEET)
        .collect();
    assert_eq!(sheets.len(), 1);
    let sheet = sheets[0];
    assert_eq!(sheet.name, "Combined");
    assert_eq!(sheet.sheets.len(), 2);
    assert_eq!(sheet.sheets[0].title, "Internal All");
    assert_eq!(sheet.sheets[1].title, "Page Titles All");
    assert!(sheet.sheets[1].rows.iter().flatten().any(|v| v == "=1+1"));
    assert_eq!(
        result.result.unwrap()["path"],
        os_integrations::drive::link(&sheet.id)
    );
    let notices = app.assemble_tick().unwrap().notices;
    assert!(notices.iter().any(|n| n.code == "notice-export-warning"
        && n.args["message"].as_str().unwrap().contains("Binärdateien")));
    assert!(mock
        .requests()
        .iter()
        .filter(|r| r.method == "PUT")
        .all(|r| r.query.get("valueInputOption").is_some_and(|v| v == "RAW")));
}

// spec: OUT-005
#[tokio::test]
async fn cancelled_sheets_command_sends_no_puts() {
    let directory = tempfile::tempdir().unwrap();
    let mock = DriveMock::default();
    let server = MockServer::start(vec![Arc::new(mock.clone())])
        .await
        .unwrap();
    let app = App::new(AppConfig::new(directory.path())).unwrap();
    fixture(&app, directory.path(), &server.base_url("drive"));
    // On this current-thread runtime, cancel before yielding to the spawned operation.
    let early: OpHandle = serde_json::from_value(app.dispatch(CallCtx::for_window("main"), "export.sheets", json!({
        "account":"fixture", "name":"Never started", "selections":[{"kind":"tab","name":"Internal:All"}]
    })).await.unwrap()).unwrap();
    assert!(app.ops().cancel(&early.op_id));
    assert_eq!(
        app.ops().wait(&early.op_id).await.unwrap().state,
        OpState::Cancelled
    );
    assert!(mock.requests().is_empty());
    // Pause before upload, then cancel the actual operation while it is in flight.
    mock.timeout_next("GET", "/drive/v3/files", std::time::Duration::from_secs(5));
    let handle: OpHandle = serde_json::from_value(app.dispatch(CallCtx::for_window("main"), "export.sheets", json!({
        "account":"fixture", "name":"Cancelled", "selections":[{"kind":"tab","name":"Internal:All"}]
    })).await.unwrap()).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while mock.requests().is_empty() {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert!(app.ops().cancel(&handle.op_id));
    let status = app.ops().wait(&handle.op_id).await.unwrap();
    assert_eq!(status.state, OpState::Cancelled);
    assert!(!mock.requests().iter().any(|r| r.method == "PUT"));
}
