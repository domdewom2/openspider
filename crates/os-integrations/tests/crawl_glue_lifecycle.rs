use os_fetch::MemorySecretStore;
use os_integrations::{
    crawl_glue::{self, ApiRun},
    credentials::{Credential, Credentials},
    ApiCtx,
};
use os_model::{ApiKind, Clock, CrawlConfig, CrawlMode, ManualClock, Timestamp};
use os_store::{CrawlFile, CrawlInit, StorageProfile};
use os_testsite::mocks::{
    ahrefs::AhrefsMock,
    ga4::{Ga4Mock, MockRow, STREAM},
    gsc::GscMock,
    psi::PsiMock,
    MockServer,
};
use std::{collections::BTreeSet, sync::Arc};

// Env overrides are process-global; isolate the real lifecycle in a child rather than
// mutating OPENSPIDER_HOME beside parallel tests. The parent owns the temporary home.
fn isolated_quota_home(test: &str) -> bool {
    if std::env::var("OPENSPIDER_FIX_N02_QUOTA_CHILD").as_deref() == Ok(test) {
        return false;
    }
    let home = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test, "--nocapture"])
        .env("OPENSPIDER_FIX_N02_QUOTA_CHILD", test)
        .env("OPENSPIDER_HOME", home.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    true
}

fn clock() -> Arc<dyn Clock> {
    Arc::new(ManualClock::new(
        Timestamp::from_db("2026-10-19T12:00:00Z").unwrap(),
    ))
}
fn file(dir: &tempfile::TempDir, cfg: &CrawlConfig) -> CrawlFile {
    let file = CrawlFile::create(
        &dir.path().join("crawl.ospider"),
        &CrawlInit {
            crawl_id: "01900000-0000-7000-8000-000000000001".parse().unwrap(),
            mode: CrawlMode::List,
            start_url: None,
            list_source: Some("APIs".into()),
            project: "fixture".into(),
            name: "fixture".into(),
            config: cfg.clone(),
            created_at: clock().now(),
        },
        StorageProfile::default(),
    )
    .unwrap();
    let urls: BTreeSet<_> = include_str!("../src/crawl_glue/apis-mode-list.txt")
        .lines()
        .collect();
    for (i, url) in urls.iter().enumerate() {
        file.conn().execute("INSERT INTO urls(id,url,address,origin,is_internal,in_tables,state,discovered_via,folder_depth,url_length) VALUES(?1,?2,?2,'https://integ.test',1,1,'discovered','list_input',1,?3)", (i as i64+1,*url,url.len() as i64)).unwrap();
    }
    file
}
fn credential(kind: ApiKind) -> Credential {
    Credential {
        account_id: format!("fixture-{kind}"),
        label: "Fixture".into(),
        api_key: Some("fixture-key".into()),
        access_token: Some("fixture-token".into()),
        refresh_token: None,
        expires_at: None,
        client_secret: None,
        scopes: vec![],
    }
}
fn context(cfg: CrawlConfig) -> ApiCtx {
    let secrets = Arc::new(MemorySecretStore::new());
    let credentials = Credentials::new(secrets.clone());
    for kind in [ApiKind::Ga4, ApiKind::Gsc, ApiKind::Psi, ApiKind::Ahrefs] {
        credentials.put(kind, credential(kind), true).unwrap();
    }
    ApiCtx {
        cfg,
        secrets,
        network: Default::default(),
        allow_api_override: true,
    }
}

// spec: INTEG-006
// spec: INTEG-012
// spec: CRAWL-037
#[tokio::test]
async fn apis_mode_assigns_bulk_and_per_url_results_without_crawling() {
    if isolated_quota_home("apis_mode_assigns_bulk_and_per_url_results_without_crawling") {
        return;
    }
    let target_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        target_dir.path().join("index.html"),
        "<title>Target</title>",
    )
    .unwrap();
    let target = os_testsite::TestSite::start_dir(target_dir.path(), 0)
        .await
        .unwrap();
    let target_url = format!("{}/a/", target.base_url());
    let ga = Ga4Mock::default();
    ga.configure(|cfg| cfg.rows = vec![MockRow::sessions(&target_url, 12.0)]);
    let gsc = GscMock::new();
    gsc.set_rows(&[(&target_url, 3.0, 50.0, 0.06, 2.0)]);
    let psi = PsiMock::default();
    let ahrefs = AhrefsMock::default();
    let server = MockServer::start(vec![
        Arc::new(ga.clone()),
        Arc::new(gsc.clone()),
        Arc::new(psi.clone()),
        Arc::new(ahrefs.clone()),
    ])
    .await
    .unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.api.ga4.enabled = true;
    cfg.api.ga4.property = Some("properties/2001".into());
    cfg.api.ga4.base_url = Some(server.base_url("ga4"));
    cfg.api.gsc.enabled = true;
    cfg.api.gsc.site = Some("sc-domain:127.0.0.1".into());
    cfg.api.gsc.base_url = Some(server.base_url("gsc"));
    cfg.api.gsc.url_inspection = true;
    cfg.api.ahrefs.enabled = true;
    cfg.api.ahrefs.base_url = Some(server.base_url("ahrefs"));
    cfg.api.ahrefs.levels = vec![os_model::ApiLevel::Url];
    cfg.api.ahrefs.metrics = vec!["backlinks".into()];
    cfg.api.psi.enabled = true;
    cfg.api.psi.base_url = Some(server.base_url("psi"));
    let dir = tempfile::tempdir().unwrap();
    let file = file(&dir, &cfg);
    file.conn().execute("UPDATE urls SET url=replace(url,'https://integ.test',?1),address=replace(address,'https://integ.test',?1),origin=?1",[target.base_url()]).unwrap();
    crawl_glue::start_apis_mode_with(&file, context(cfg), clock())
        .await
        .unwrap();
    assert_eq!(psi.calls().len(), 4, "one request per unique input");
    assert!(ga.bodies("runReport").len() <= 2, "bulk report cached");
    let conn = file.conn();
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM urls WHERE status_code IS NOT NULL OR title_1 IS NOT NULL",
            [],
            |r| r.get::<_, u64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row(
            "SELECT v_real FROM api_values WHERE source='ga4' AND metric='sessions' AND url_id=1",
            [],
            |r| r.get::<_, f64>(0)
        )
        .unwrap(),
        12.0
    );
    assert_eq!(
        conn.query_row(
            "SELECT v_real FROM api_values WHERE source='gsc' AND metric='clicks' AND url_id=1",
            [],
            |r| r.get::<_, f64>(0)
        )
        .unwrap(),
        3.0
    );
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM api_errors", [], |r| r
            .get::<_, u64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM api_status WHERE urls_done=urls_total",
            [],
            |r| r.get::<_, u64>(0)
        )
        .unwrap(),
        4
    );
    assert_eq!(gsc.requests_to("inspect").len(), 4);
    // Exercise start_apis_mode_with -> ApiRun::new -> app_paths, not an injected helper path.
    let home = std::path::PathBuf::from(std::env::var_os("OPENSPIDER_HOME").unwrap());
    assert_eq!(
        crawl_glue::quota_ledger_path(),
        home.join("api-quotas.json")
    );
    assert!(home.join("api-quotas.json").is_file());
    assert_eq!(ahrefs.requests.all().len(), 1);
    assert_eq!(
        target.total_requests(),
        0,
        "APIs mode never requests target pages or robots.txt"
    );
    server.stop().await;
}

// spec: INTEG-012
// spec: OUT-201 [P1]
#[test]
fn missing_credentials_are_validated_without_requests() {
    let mut cfg = CrawlConfig::default();
    cfg.api.gsc.enabled = true;
    cfg.api.gsc.account = Some("kontoA".into());
    let mut ctx = context(cfg);
    ctx.secrets = Arc::new(MemorySecretStore::new());
    assert_eq!(
        crawl_glue::preflight(&ctx, false).unwrap(),
        ["gsc: Konto kontoA nicht verbunden"]
    );
    assert!(crawl_glue::preflight(&ctx, true)
        .unwrap_err()
        .to_string()
        .contains("kontoA"));
    let dir = tempfile::tempdir().unwrap();
    let mut run = ApiRun::new(
        context(CrawlConfig::default()),
        clock(),
        dir.path().join("quota.json"),
    );
    assert!(run.validate(true).is_err());
    assert!(!dir.path().join("quota.json").exists());
}

// spec: INTEG-015
// spec: OUT-099 [P1]
#[tokio::test]
async fn api_only_urls_are_recorded_and_same_host_discovery_is_opt_in() {
    let ga = Ga4Mock::default();
    ga.configure(|cfg| {
        cfg.rows = vec![
            MockRow::sessions("https://integ.test/orphan", 12.0),
            MockRow::sessions("https://fremd.test/x", 20.0),
        ]
    });
    let server = MockServer::start(vec![Arc::new(ga)]).await.unwrap();
    for (crawl_new, sitemap_only) in [(false, false), (true, false), (true, true)] {
        let mut cfg = CrawlConfig::default();
        cfg.api.ga4.enabled = true;
        cfg.api.ga4.property = Some(STREAM.into());
        cfg.api.ga4.base_url = Some(server.base_url("ga4"));
        cfg.api.ga4.crawl_new_urls = crawl_new;
        let dir = tempfile::tempdir().unwrap();
        let file = file(&dir, &cfg);
        if sitemap_only {
            file.conn().execute("DELETE FROM urls", []).unwrap();
            file.set_meta("mode", "spider").unwrap();
            file.set_meta("start_url", "https://integ.test/map.xml")
                .unwrap();
        }
        let run = ApiRun::new(context(cfg), clock(), dir.path().join("quota.json"));
        let candidates = run.discover(&file).await.unwrap();
        assert_eq!(
            candidates,
            if crawl_new {
                vec!["https://integ.test/orphan"]
            } else {
                vec![]
            }
        );
        assert_eq!(
            file.conn()
                .query_row(
                    "SELECT COUNT(*) FROM api_urls WHERE source='ga4'",
                    [],
                    |r| r.get::<_, u64>(0)
                )
                .unwrap(),
            1
        );
    }
    server.stop().await;
}

// spec: INTEG-017
#[test]
fn scheduling_selection_keeps_account_property_and_stream() {
    let mut cfg = CrawlConfig::default();
    cfg.api.ga4.account = Some("kontoA".into());
    cfg.api.ga4.property = Some(STREAM.into());
    cfg.api.gsc.site = Some("sc-domain:integ.test".into());
    crawl_glue::select(&mut cfg, &["ga4".into(), "gsc".into()]).unwrap();
    assert!(cfg.api.ga4.enabled && cfg.api.gsc.enabled);
    assert_eq!(cfg.api.ga4.account.as_deref(), Some("kontoA"));
    assert_eq!(cfg.api.ga4.property.as_deref(), Some(STREAM));
    assert_eq!(cfg.api.gsc.site.as_deref(), Some("sc-domain:integ.test"));
    let restored = CrawlConfig::from_toml_str(&cfg.to_toml_string().unwrap()).unwrap();
    assert_eq!(restored, cfg);
}

// spec: OUT-201 [P1]
// spec: INTEG-017
#[tokio::test]
async fn cli_google_selection_reaches_exact_account_property_and_stream() {
    if isolated_quota_home("cli_google_selection_reaches_exact_account_property_and_stream") {
        return;
    }
    let ga = Ga4Mock::default();
    let gsc = GscMock::new();
    let server = MockServer::start(vec![Arc::new(ga.clone()), Arc::new(gsc.clone())])
        .await
        .unwrap();
    let mut cfg = CrawlConfig::default();
    crawl_glue::configure_google(
        &mut cfg,
        Some(&[
            "fixture-ga4".into(),
            "1001".into(),
            "2001".into(),
            "3001".into(),
        ]),
        Some(&["fixture-gsc".into(), "sc-domain:integ.test".into()]),
    )
    .unwrap();
    cfg.api.ga4.base_url = Some(server.base_url("ga4"));
    cfg.api.gsc.base_url = Some(server.base_url("gsc"));
    let dir = tempfile::tempdir().unwrap();
    let file = file(&dir, &cfg);
    assert_eq!(cfg.api.ga4.analytics_account.as_deref(), Some("1001"));
    assert_eq!(cfg.api.ga4.property.as_deref(), Some(STREAM));
    crawl_glue::start_apis_mode_with(&file, context(cfg), clock())
        .await
        .unwrap();
    assert!(ga
        .requests()
        .iter()
        .any(|r| r.path.contains("properties/2001/dataStreams")));
    assert!(ga
        .requests()
        .iter()
        .filter(|r| r.path.contains(":runReport"))
        .all(|r| r.path.contains("properties/2001")));
    assert!(gsc
        .requests_to("query")
        .iter()
        .all(|r| r.property.as_deref() == Some("sc-domain:integ.test")));
    assert!(!gsc.requests_to("query").is_empty());
    server.stop().await;
}

// spec: OUT-201 [P1]
#[test]
fn environment_keys_work_for_named_profile_accounts_without_logging_secrets() {
    if std::env::var_os("OPENSPIDER_I06_ENV_CHILD").is_some() {
        let mut cfg = CrawlConfig::default();
        cfg.api.psi.account = Some("profile-psi".into());
        cfg.api.psi.enabled = true;
        cfg.api.ahrefs.account = Some("profile-ahrefs".into());
        let secrets = crawl_glue::environment_view_for(Arc::new(MemorySecretStore::new()), &cfg);
        let c = os_integrations::credentials::read_account(
            secrets.as_ref(),
            ApiKind::Psi,
            Some("profile-psi"),
        )
        .unwrap();
        assert!(c.api_key.as_deref() == Some("fixture-env-key"));
        assert!(!format!("{c:?}").contains("fixture-env-key"));
        let c = os_integrations::credentials::read_account(
            secrets.as_ref(),
            ApiKind::Ahrefs,
            Some("profile-ahrefs"),
        )
        .unwrap();
        assert!(c.access_token.as_deref() == Some("fixture-env-key"));

        return;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "environment_keys_work_for_named_profile_accounts_without_logging_secrets",
        ])
        .env("OPENSPIDER_I06_ENV_CHILD", "1")
        .env("OPENSPIDER_PSI_KEY", "fixture-env-key")
        .env("OPENSPIDER_AHREFS_KEY", "fixture-env-key")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains("fixture-env-key"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("fixture-env-key"));
}

async fn discovery_failure_preserves_other_source(failed: ApiKind) {
    let ga = Ga4Mock::default();
    ga.configure(|cfg| cfg.rows = vec![MockRow::sessions("https://integ.test/ga-orphan", 12.0)]);
    let gsc = GscMock::new();
    gsc.set_rows(&[("https://integ.test/gsc-orphan", 3.0, 50.0, 0.06, 2.0)]);
    if failed == ApiKind::Ga4 {
        ga.set_scenario("400-invalid-date-range");
    } else {
        gsc.set_scenario(Some("error"));
    }
    let server = MockServer::start(vec![Arc::new(ga.clone()), Arc::new(gsc.clone())])
        .await
        .unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.api.ga4.enabled = true;
    cfg.api.ga4.crawl_new_urls = true;
    cfg.api.ga4.property = Some(STREAM.into());
    cfg.api.ga4.base_url = Some(server.base_url("ga4"));
    cfg.api.gsc.enabled = true;
    cfg.api.gsc.crawl_new_urls = true;
    cfg.api.gsc.site = Some("sc-domain:integ.test".into());
    cfg.api.gsc.base_url = Some(server.base_url("gsc"));
    let dir = tempfile::tempdir().unwrap();
    let file = file(&dir, &cfg);
    let run = ApiRun::new(context(cfg), clock(), dir.path().join("quota.json"));
    let expected = if failed == ApiKind::Ga4 {
        "https://integ.test/gsc-orphan"
    } else {
        "https://integ.test/ga-orphan"
    };
    // Repeated discovery neither duplicates the error nor loses successful provenance.
    for _ in 0..2 {
        assert_eq!(run.discover(&file).await.unwrap(), vec![expected]);
        let errors =
            os_store::queries::api::errors(file.conn(), &failed.to_string(), 0, 10).unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "api_discovery");
        assert_eq!(errors[0].url, None);
        assert!(!errors[0].message.is_empty());
        assert!(!errors[0].message.contains("fixture-token"));
        let (url, source): (String, String) = file
            .conn()
            .query_row("SELECT url,source FROM api_urls", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(url, expected);
        assert_eq!(source, if failed == ApiKind::Ga4 { "gsc" } else { "ga4" });
    }
    assert!(!ga.requests().is_empty());
    assert!(!gsc.requests_to("query").is_empty());
    server.stop().await;
}

// spec: INTEG-015
#[tokio::test]
async fn ga4_failure_does_not_skip_gsc_orphan_discovery() {
    discovery_failure_preserves_other_source(ApiKind::Ga4).await;
}

// spec: INTEG-015
#[tokio::test]
async fn gsc_failure_does_not_discard_ga4_orphan_discovery() {
    discovery_failure_preserves_other_source(ApiKind::Gsc).await;
}
