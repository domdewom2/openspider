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
