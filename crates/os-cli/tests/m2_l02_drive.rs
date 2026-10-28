use clap::Parser;
use os_cli::{args::Cli, google, reporter::Reporter};
use os_integrations::{
    credentials::{Credential, Credentials},
    drive::SCOPE,
};
use os_model::{ApiKind, CancelToken, CrawlConfig};
use os_testsite::mocks::{drive::DriveMock, MockServer};
use std::sync::Arc;

// spec: OUT-198
// spec: OUT-190 [M2]
#[tokio::test]
async fn selected_account_exports_and_replaces_in_timestamp_folder() {
    let home = tempfile::tempdir().unwrap();
    let paths = os_store::paths::app_paths_with_home(Some(home.path().as_os_str()));
    let secrets = Arc::new(Credentials::new(Arc::new(
        os_fetch::MemorySecretStore::new(),
    )));
    let args = Cli::try_parse_from([
        "openspider",
        "--google-drive-account",
        "fixture@example.test",
    ])
    .unwrap()
    .run;
    let error = google::validate_with_secrets(&args, &paths, secrets.clone()).unwrap_err();
    assert_eq!(error.exit, os_cli::exit::Exit::Usage);
    assert!(
        error.message.contains("fixture@example.test")
            && error.message.contains("in der App verbinden")
    );
    secrets
        .put(
            ApiKind::Drive,
            Credential {
                account_id: "fixture@example.test".into(),
                label: "Fixture".into(),
                api_key: None,
                access_token: Some("private-fixture-token".into()),
                refresh_token: None,
                expires_at: None,
                client_secret: None,
                scopes: vec![SCOPE.into()],
            },
            true,
        )
        .unwrap();
    let target = google::validate_with_secrets(&args, &paths, secrets.clone())
        .unwrap()
        .unwrap();
    let mock = DriveMock::default();
    let server = MockServer::start(vec![Arc::new(mock.clone())])
        .await
        .unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.api.drive.base_url = Some(server.base_url("drive"));
    let target = google::GoogleTarget {
        allow_override: true,
        ..target
    };
    let client = target.client_with_secrets(&cfg, &paths, secrets).unwrap();
    let csv = home.path().join("Übersicht.csv");
    std::fs::write(&csv, "Address,Status Code\nhttps://site.test/,200\n").unwrap();
    for _ in 0..2 {
        assert_eq!(
            google::upload_at(
                &client,
                "Projekt 日本語",
                "Crawl",
                std::slice::from_ref(&csv),
                false,
                Some("2026-10-28_12-00-00"),
                &CancelToken::new(),
                &Reporter::new(false)
            )
            .await
            .unwrap(),
            1
        );
    }
    let files = mock.files();
    let folder = files
        .iter()
        .find(|f| f.name == "2026-10-28_12-00-00")
        .unwrap();
    let sheets: Vec<_> = files.iter().filter(|f| f.parent == folder.id).collect();
    assert_eq!(
        sheets.len(),
        1,
        "same-name export replaces the existing spreadsheet"
    );
    assert_eq!(sheets[0].sheets[0].rows[1][1], 200);
    assert!(!mock
        .requests()
        .iter()
        .any(|r| r.body.to_string().contains("private-fixture-token")));
}
