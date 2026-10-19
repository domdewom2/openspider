#[path = "drive_support.rs"]
mod support;
use os_integrations::{
    credentials::read_account,
    drive::{self, DriveClient},
    oauth,
};
use os_model::{ApiKind, Timestamp};
use serde_json::json;
use std::{sync::Arc, time::Duration};

// spec: OUT-005
#[tokio::test]
async fn folder_identity_escaping_and_deleted_or_trashed_files() {
    let (server, mock, client, _, _) = support::fixture().await;
    let folder = client
        .folder("OpenSpider", "A's \\ project", Some("Crawl"))
        .await
        .unwrap();
    assert_eq!(
        folder,
        client
            .folder("OpenSpider", "A's \\ project", Some("Crawl"))
            .await
            .unwrap()
    );
    let id = client
        .find_or_create(&folder, "Export", drive::SPREADSHEET)
        .await
        .unwrap();
    assert_eq!(
        id,
        client
            .find_or_create(&folder, "Export", drive::SPREADSHEET)
            .await
            .unwrap()
    );
    mock.trash(&id);
    let next = client
        .find_or_create(&folder, "Export", drive::SPREADSHEET)
        .await
        .unwrap();
    assert_ne!(id, next);
    mock.delete(&next);
    assert_ne!(
        next,
        client
            .find_or_create(&folder, "Export", drive::SPREADSHEET)
            .await
            .unwrap()
    );
    assert_eq!(
        mock.files()
            .iter()
            .filter(|f| f.mime == drive::FOLDER)
            .count(),
        3
    );
    server.stop().await;
}

// spec: OUT-005
#[tokio::test(start_paused = true)]
async fn retries_429_all_5xx_exactly_five_times_and_redacts_revoked_tokens() {
    let (_server, mock, client, ctx, credentials) = support::fixture().await;
    let client = client.with_timeout(Duration::from_secs(86400));
    with_clock(async {
        for code in [429, 500, 501, 502, 503] {
            mock.fail_next("GET", "/drive/v3/files", code);
        }
        let before = tokio::time::Instant::now();
        client
            .find_or_create("root", "OpenSpider", drive::FOLDER)
            .await
            .unwrap();
        assert!(before.elapsed() >= Duration::from_secs(31));
        assert_eq!(
            mock.requests().iter().filter(|r| r.method == "GET").count(),
            6
        );
        for _ in 0..6 {
            mock.fail_next("GET", "/drive/v3/files", 503);
        }
        let count = mock.requests().len();
        assert!(client
            .find_or_create("root", "Fail", drive::FOLDER)
            .await
            .is_err());
        assert_eq!(mock.requests().len() - count, 6);
        mock.fail_next("GET", "/drive/v3/files", 401);
        let error = client
            .find_or_create("root", "Fail", drive::FOLDER)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains(drive::RECONNECT));
        assert!(!error.contains("fixture-token"));
        let mut c = read_account(credentials.as_ref(), ApiKind::Drive, Some("fixture")).unwrap();
        c.expires_at = Some(Timestamp::UNIX_EPOCH);
        credentials.put(ApiKind::Drive, c, true).unwrap();
        assert!(DriveClient::new(&ctx, "fixture", Timestamp::UNIX_EPOCH).is_err());
    })
    .await;
}

// Keep the runtime runnable while explicitly advancing virtual time. Automatic
// time jumps can otherwise outrun loopback socket readiness on macOS.
async fn with_clock<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::pin!(future);
    loop {
        tokio::select! {
            biased;
            result=&mut future => return result,
            _=async {
                for _ in 0..100 { tokio::task::yield_now().await; }
                tokio::time::advance(Duration::from_millis(100)).await;
            }=>{}
        }
    }
}

// spec: OUT-005
#[tokio::test]
async fn timed_out_raw_write_retries_the_same_range_without_duplicates() {
    let (_server, mock, client, _, _) = support::fixture().await;
    let id = client
        .find_or_create("root", "Timeout", drive::SPREADSHEET)
        .await
        .unwrap();
    mock.timeout_next_write(Duration::from_millis(100));
    let client = client.with_timeout(Duration::from_millis(30));
    client
        .write(&id, "'Sheet1'!A1", &[vec![json!("=1+1")]])
        .await
        .unwrap();
    assert_eq!(
        client.values(&id, "'Sheet1'").await.unwrap(),
        vec![vec![json!("=1+1")]]
    );
    assert_eq!(
        mock.requests().iter().filter(|r| r.method == "PUT").count(),
        2
    );
}

// spec: OUT-183 [M2]
#[tokio::test]
async fn sheet_list_reads_all_cells_in_upload_order_with_duplicates_and_validates_url() {
    use os_testsite::mocks::drive::Sheet;
    let (_server, mock, client, _, _) = support::fixture().await;
    let id = mock.seed(
        "List",
        "root",
        drive::SPREADSHEET,
        vec![
            Sheet {
                id: 0,
                title: "First ' tab".into(),
                rows: vec![
                    vec![json!("https://site.test/b"), json!("https://site.test/a")],
                    vec![json!("https://site.test/b"), json!("ignored")],
                ],
                date_format: None,
            },
            Sheet {
                id: 1,
                title: "Next".into(),
                rows: vec![vec![json!("https://site.test/c")]],
                date_format: None,
            },
        ],
    );
    assert_eq!(
        client.list_items(&drive::link(&id)).await.unwrap(),
        [
            "https://site.test/b",
            "https://site.test/a",
            "https://site.test/b",
            "ignored",
            "https://site.test/c"
        ]
    );
    assert!(drive::spreadsheet_id("https://evil.test/spreadsheets/d/123").is_err());
    assert!(drive::spreadsheet_id("https://docs.google.com/spreadsheets/d/..").is_err());
}

// spec: OUT-005
#[tokio::test]
async fn pkce_drive_file_only_and_secret_store_roundtrip() {
    use os_testsite::mocks::{oauth::OAuthMock, MockServer};
    let (_server, _mock, _client, ctx, credentials) = support::fixture().await;
    let oauth = MockServer::start(vec![Arc::new(OAuthMock::default())])
        .await
        .unwrap();
    let base = oauth.base_url("oauth");
    let endpoints = oauth::Endpoints {
        authorize: format!("{base}/authorize"),
        token: format!("{base}/token"),
        revoke: format!("{base}/revoke"),
    };
    let browser = reqwest::Client::new();
    let credential = drive::connect(
        &ctx,
        &credentials,
        "fixture-client",
        None,
        &endpoints,
        Arc::new(os_model::time::FixedClock::new(
            Timestamp::UNIX_EPOCH,
            Duration::ZERO,
        )),
        true,
        move |url| {
            let url = url::Url::parse(url).unwrap();
            assert_eq!(
                url.query_pairs().find(|(k, _)| k == "scope").unwrap().1,
                drive::SCOPE
            );
            assert_eq!(
                url.query_pairs()
                    .find(|(k, _)| k == "code_challenge_method")
                    .unwrap()
                    .1,
                "S256"
            );
            tokio::spawn(async move {
                browser.get(url).send().await.unwrap();
            });
            Ok(())
        },
    )
    .await
    .unwrap();
    assert_eq!(credential.scopes, [drive::SCOPE]);
    assert!(credentials.is_persistent(ApiKind::Drive, &credential.account_id));
    assert!(format!("{credential:?}").contains("redacted"));
    let mut expired = credential.clone();
    expired.expires_at = Some(Timestamp::UNIX_EPOCH);
    credentials
        .put(ApiKind::Drive, expired.clone(), true)
        .unwrap();
    let clock = os_model::time::FixedClock::new(Timestamp::UNIX_EPOCH, Duration::ZERO);
    drive::refresh_account(
        &ctx,
        &credentials,
        &expired.account_id,
        "fixture-client",
        &endpoints,
        &clock,
    )
    .await
    .unwrap();
    assert_eq!(
        read_account(
            credentials.as_ref(),
            ApiKind::Drive,
            Some(&expired.account_id)
        )
        .unwrap()
        .access_token
        .as_deref(),
        Some("fixture-access")
    );
    expired.refresh_token = Some("invalid".into());
    credentials
        .put(ApiKind::Drive, expired.clone(), true)
        .unwrap();
    assert!(drive::refresh_account(
        &ctx,
        &credentials,
        &expired.account_id,
        "fixture-client",
        &endpoints,
        &clock
    )
    .await
    .unwrap_err()
    .to_string()
    .contains(drive::RECONNECT));
}

// spec: OUT-005
#[tokio::test]
async fn ambiguous_create_and_structural_batch_timeouts_do_not_duplicate_files_or_sheets() {
    let (_server, mock, client, _, _) = support::fixture().await;
    let client = client.with_timeout(Duration::from_millis(50));
    mock.timeout_next("POST", "/drive/v3/files", Duration::from_millis(200));
    let id = client
        .find_or_create("root", "Ambiguous", drive::SPREADSHEET)
        .await
        .unwrap();
    assert_eq!(
        mock.files()
            .iter()
            .filter(|f| f.name == "Ambiguous")
            .count(),
        1
    );
    mock.timeout_next("POST", ":batchUpdate", Duration::from_millis(200));
    client
        .batch(
            &id,
            vec![
                json!({"addSheet":{"properties":{"sheetId":1,"title":"data"}}}),
                json!({"deleteSheet":{"sheetId":0}}),
            ],
        )
        .await
        .unwrap();
    let sheets = client.sheets(&id).await.unwrap();
    assert_eq!(sheets.len(), 1);
    assert_eq!(sheets[0].title, "data");
}
