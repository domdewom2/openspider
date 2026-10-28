use os_fetch::secrets::MemorySecretStore;
use os_integrations::{
    credentials::{Credential, Credentials},
    drive::{DriveClient, SCOPE},
    ApiCtx,
};
use os_model::{ApiKind, CrawlConfig, Timestamp};
use os_testsite::mocks::{drive::DriveMock, MockServer};
use std::sync::Arc;
pub async fn fixture() -> (MockServer, DriveMock, DriveClient, ApiCtx, Arc<Credentials>) {
    let mock = DriveMock::default();
    let server = MockServer::start(vec![Arc::new(mock.clone())])
        .await
        .unwrap();
    let credentials = Arc::new(Credentials::new(Arc::new(MemorySecretStore::new())));
    credentials
        .put(
            ApiKind::Drive,
            Credential {
                account_id: "fixture".into(),
                label: "Fixture Google".into(),
                api_key: None,
                access_token: Some("fixture-token".into()),
                refresh_token: Some("fixture-refresh".into()),
                expires_at: None,
                client_secret: None,
                scopes: vec![SCOPE.into()],
            },
            true,
        )
        .unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.api.drive.base_url = Some(server.base_url("drive"));
    let mut network = os_model::settings::NetworkSettings::default();
    network.proxy.mode = os_model::ProxyMode::None;
    let ctx = ApiCtx {
        cfg,
        secrets: credentials.clone(),
        network,
        allow_api_override: true,
        quota_ledger: None,
    };
    let client = DriveClient::new(&ctx, "fixture", Timestamp::UNIX_EPOCH).unwrap();
    (server, mock, client, ctx, credentials)
}
