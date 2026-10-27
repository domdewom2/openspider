// Fixture SQL assertions inspect the persisted engine result.
#![allow(clippy::disallowed_methods, clippy::disallowed_types)]
use os_app::{commands::auth_login::LoginBrowserMode, App, AppConfig, CallCtx, WindowId};
use os_fetch::secrets::MemorySecretStore;
use os_model::{CrawlConfig, RenderingMode};
use os_testsite::mocks::form_login::FormLoginSite;
use serde_json::{json, Value};
use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};
static CHROMIUM: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
async fn call(app: &App, name: &str, args: Value) -> Value {
    app.dispatch(CallCtx::for_window("main"), name, args)
        .await
        .unwrap()
}
fn automate(app: &App) {
    let mode = app.ext::<LoginBrowserMode>();
    mode.headless.store(true, Ordering::Relaxed);
    *mode.on_ready.lock().unwrap() = Some(Arc::new(|browser| {
        Box::pin(async move {
            for _ in 0..100 {
                if browser
                    .page()
                    .evaluate("!!document.querySelector('form')")
                    .await
                    .ok()
                    .and_then(|v| v.into_value::<bool>().ok())
                    == Some(true)
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            browser.page().evaluate("document.querySelector('[name=user]').value='fixture';document.querySelector('[name=password]').value='secret';document.querySelector('[name=code]').value='123456';document.querySelector('form').requestSubmit();").await.unwrap();
            for _ in 0..200 {
                if browser
                    .page()
                    .url()
                    .await
                    .ok()
                    .flatten()
                    .is_some_and(|u| u.ends_with("/mitglieder/"))
                {
                    return Ok(());
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            panic!("form login did not complete")
        })
    }));
}
async fn login(app: &App, site: &FormLoginSite) -> Value {
    automate(app);
    let op = call(app, "auth.openLogin", json!({"url":site.url("/login-2fa")})).await;
    let info = call(
        app,
        "auth.finishLogin",
        json!({"opId":op["opId"],"saveAs":"fixture-form"}),
    )
    .await;
    let done = app.ops().wait(op["opId"].as_str().unwrap()).await.unwrap();
    assert_eq!(done.result.as_ref().unwrap()["cookies"], 1);
    assert!(!serde_json::to_string(&done).unwrap().contains("FIXTURE"));
    info
}
async fn activate(app: &App, site: &FormLoginSite) {
    let info = login(app, site).await;
    let rows = call(app, "auth.loginStatus", Value::Null).await;
    let row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["profile"] == info["name"])
        .unwrap();
    let mut dto = call(app, "config.get", Value::Null).await;
    dto["config"]["auth"]["form"] = json!([{
        "login_url":row["loginUrl"],"profile":info["name"],
        "captured_at":row["capturedAt"],"cookie_count":row["cookies"]
    }]);
    dto["config"]["auth"]["session_guard"] = json!(os_render::login::SESSION_GUARD_DEFAULTS);
    call(app, "config.set", dto).await;
}
async fn crawl(
    app: &App,
    site: &FormLoginSite,
    cfg: CrawlConfig,
    path: std::path::PathBuf,
) -> os_store::CrawlFile {
    let deps = os_app::session::control::deps(app);
    let fetch = Arc::new(
        os_fetch::HttpFetcher::new(
            os_fetch::FetchConfig::from_crawl_config(&cfg, &*deps.secrets, deps.clock.clone())
                .unwrap(),
        )
        .unwrap(),
    );
    let summary = os_engine::start_crawl(
        fetch,
        os_engine::CrawlRequest {
            config: cfg,
            source: os_engine::CrawlSource::Spider {
                start_url: site.url("/mitglieder/"),
            },
            file: path.clone(),
            project: "fixture".into(),
            name: "form-login".into(),
        },
        deps,
    )
    .await
    .unwrap()
    .wait()
    .await
    .unwrap();
    assert_eq!(summary.state, os_model::CrawlState::Completed);
    os_store::CrawlFile::open(&path, os_store::OpenMode::ReadOnly).unwrap()
}
// spec: CRAWL-120
// spec: OUT-223
#[tokio::test]
async fn encrypted_form_profile_roundtrip_protects_crawl_and_restores_renderer_storage() {
    let _lock = CHROMIUM.lock().await;
    let site = FormLoginSite::start().await.unwrap();
    let home = tempfile::tempdir().unwrap();
    let app =
        App::new(AppConfig::new(home.path()).with_secrets(Arc::new(MemorySecretStore::new())))
            .unwrap();
    activate(&app, &site).await;
    let dto = call(&app, "config.get", Value::Null).await;
    assert!(!dto.to_string().contains("FIXTURE"));
    let mut cfg: CrawlConfig = serde_json::from_value(dto["config"].clone()).unwrap();
    assert_eq!(cfg.auth.form[0].cookie_count, 1);
    assert_eq!(cfg.auth.session_guard.len(), 4);
    // Export and import with an independent memory keyring, then crawl from the imported profile.
    let export = home.path().join("portable.osauth");
    call(&app,"auth.exportProfile",json!({"name":"fixture-form","path":export,"passphrase":"fixture-passphrase","bindToMachine":false})).await;
    for file in [
        export.clone(),
        std::fs::read_dir(home.path().join("auth"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path(),
    ] {
        assert!(!String::from_utf8_lossy(&std::fs::read(file).unwrap()).contains("FIXTURE"));
    }
    let other_home = tempfile::tempdir().unwrap();
    let other = App::new(
        AppConfig::new(other_home.path()).with_secrets(Arc::new(MemorySecretStore::new())),
    )
    .unwrap();
    call(
        &other,
        "auth.importProfile",
        json!({"path":export,"passphrase":"fixture-passphrase","bindToMachine":true}),
    )
    .await;
    cfg.speed.threads = 1;
    cfg.rendering.ajax_timeout_s = 0.1;
    cfg.rendering.screenshots.enabled = false;
    site.clear_requests();
    let file = crawl(
        &other,
        &site,
        cfg.clone(),
        other_home.path().join("guarded.ospider"),
    )
    .await;
    assert!(!site
        .requests()
        .iter()
        .any(|(p, _)| p == "/logout" || p.starts_with("/konto/")));
    let skipped: i64 = file
        .conn()
        .query_row(
            "SELECT value FROM crawl_counters WHERE key='session_guard_skipped'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(skipped, 2);
    let bad: i64 = file
        .conn()
        .query_row(
            "SELECT count(*) FROM urls WHERE url LIKE '%/mitglieder/%' AND status_code != 200",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(bad, 0);
    let overview = os_export::reports::overview::build(&file, &cfg, &[]).unwrap();
    assert!(overview
        .rows
        .iter()
        .any(|r| r.item == "Durch Sitzungsschutz übersprungen"
            && r.urls == os_export::reports::overview::Cell::Int(2)));
    // Resume starts with a fresh fetcher/jar and must restore the encrypted profile again.
    let mut limited = cfg.clone();
    limited.limits.max_urls = 2;
    let resume_path = other_home.path().join("resume.ospider");
    drop(crawl(&other, &site, limited, resume_path.clone()).await);
    let deps = os_app::session::control::deps(&other);
    let fetch = Arc::new(
        os_fetch::HttpFetcher::new(
            os_fetch::FetchConfig::from_crawl_config(&cfg, &*deps.secrets, deps.clock.clone())
                .unwrap(),
        )
        .unwrap(),
    );
    os_engine::resume_crawl(fetch, resume_path.clone(), Some(cfg.clone()), deps)
        .await
        .unwrap()
        .wait()
        .await
        .unwrap();
    let resumed = os_store::CrawlFile::open(&resume_path, os_store::OpenMode::ReadOnly).unwrap();
    let unauthorized: i64 = resumed
        .conn()
        .query_row(
            "SELECT count(*) FROM urls WHERE url LIKE '%/mitglieder/%' AND status_code != 200",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(unauthorized, 0);
    cfg.rendering.mode = RenderingMode::JavaScript;
    cfg.limits.max_depth = Some(1);
    let rendered = crawl(
        &other,
        &site,
        cfg,
        other_home.path().join("rendered.ospider"),
    )
    .await;
    let title: String = rendered
        .conn()
        .query_row(
            "SELECT title_1 FROM urls WHERE url LIKE '%/mitglieder/app'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(title, "Token ok");
}
// spec: CRAWL-120
#[tokio::test]
async fn clearing_guard_allows_logout_and_records_session_loss_without_stopping() {
    let _lock = CHROMIUM.lock().await;
    let site = FormLoginSite::start().await.unwrap();
    let home = tempfile::tempdir().unwrap();
    let app =
        App::new(AppConfig::new(home.path()).with_secrets(Arc::new(MemorySecretStore::new())))
            .unwrap();
    activate(&app, &site).await;
    let dto = call(&app, "config.get", Value::Null).await;
    let mut cfg: CrawlConfig = serde_json::from_value(dto["config"].clone()).unwrap();
    // The fixture's guarded pages redirect to /login after logout.
    cfg.auth.form[0].login_url = site.url("/login");
    cfg.auth.session_guard.clear();
    cfg.speed.threads = 1;
    site.clear_requests();
    let file = crawl(
        &app,
        &site,
        cfg.clone(),
        home.path().join("unguarded.ospider"),
    )
    .await;
    assert!(site.requests().iter().any(|(p, _)| p == "/logout"));
    let lost: i64 = file
        .conn()
        .query_row(
            "SELECT value FROM crawl_counters WHERE key='session_lost'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(lost, 1);
    let events: i64 = file
        .conn()
        .query_row(
            "SELECT count(*) FROM crawl_events WHERE code='session_lost'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(events, 1);
    let key = os_app::pool::CrawlKey("loss".into());
    app.register_crawl(key.clone(), file.path());
    app.sessions().set_current(
        WindowId("main".into()),
        os_app::runtime::sessions::SessionInfo {
            key,
            path: file.path().into(),
            config: Arc::new(cfg),
            state: os_model::CrawlState::Completed,
            writable: true,
            engine: None,
        },
    );
    assert_eq!(
        call(&app, "auth.loginStatus", Value::Null).await[0]["state"],
        "lost"
    );
}
// spec: CRAWL-120
#[tokio::test]
async fn cancellation_discards_capture_and_foreign_window_cannot_finish() {
    let _lock = CHROMIUM.lock().await;
    let site = FormLoginSite::start().await.unwrap();
    let home = tempfile::tempdir().unwrap();
    let app =
        App::new(AppConfig::new(home.path()).with_secrets(Arc::new(MemorySecretStore::new())))
            .unwrap();
    app.ext::<LoginBrowserMode>()
        .headless
        .store(true, Ordering::Relaxed);
    let op = call(&app, "auth.openLogin", json!({"url":site.url("/login")})).await;
    assert!(app
        .dispatch(
            CallCtx::for_window("other"),
            "auth.finishLogin",
            json!({"opId":op["opId"]})
        )
        .await
        .is_err());
    call(&app, "op.cancel", json!({"opId":op["opId"]})).await;
    app.ops().wait(op["opId"].as_str().unwrap()).await.unwrap();
    assert!(app
        .dispatch(
            CallCtx::for_window("main"),
            "auth.finishLogin",
            json!({"opId":op["opId"]})
        )
        .await
        .is_err());
    assert_eq!(
        call(&app, "auth.listProfiles", Value::Null).await,
        json!([])
    );
    assert_eq!(call(&app, "auth.loginStatus", Value::Null).await, json!([]));
}

// spec: CRAWL-120
#[tokio::test]
async fn status_derives_expiry_from_encrypted_cookie_metadata_without_exposing_values() {
    let home = tempfile::tempdir().unwrap();
    let app =
        App::new(AppConfig::new(home.path()).with_secrets(Arc::new(MemorySecretStore::new())))
            .unwrap();
    let session = os_render::login::CapturedSession {
        cookies: vec![os_render::login::LoginCookie {
            name: "session".into(),
            value: "EXPIRY-SECRET".into(),
            domain: "fixture.test".into(),
            path: "/".into(),
            secure: false,
            http_only: true,
            expires: Some(100),
        }],
        ..Default::default()
    };
    call(&app,"auth.saveProfile",json!({"profile":{"name":"expired-form","kind":"form","host":"fixture.test","username":null,"secret":serde_json::to_string(&session).unwrap()}})).await;
    let mut cfg = CrawlConfig::default();
    cfg.auth.form.push(os_model::FormLogin {
        login_url: "https://fixture.test/login".into(),
        profile: Some("expired-form".into()),
        cookie_count: 1,
        captured_at: None,
    });
    call(
        &app,
        "config.set",
        json!({"schemaVersion":cfg.schema_version,"generator":cfg.generator,"config":cfg}),
    )
    .await;
    let statuses = call(&app, "auth.loginStatus", Value::Null).await;
    assert_eq!(statuses[0]["state"], "expired");
    assert!(!statuses.to_string().contains("EXPIRY-SECRET"));
}

// spec: CRAWL-120
#[tokio::test]
async fn draft_status_tracks_expiry_before_configuration_is_applied() {
    let _lock = CHROMIUM.lock().await;
    let site = FormLoginSite::start().await.unwrap();
    let home = tempfile::tempdir().unwrap();
    let clock = Arc::new(os_model::ManualClock::new(os_model::Timestamp::UNIX_EPOCH));
    let app = App::new(
        AppConfig::new(home.path())
            .with_clock(clock.clone())
            .with_secrets(Arc::new(MemorySecretStore::new())),
    )
    .unwrap();
    let info = login(&app, &site).await;
    let name = info["name"].as_str().unwrap();
    let mut session = os_engine::session_guard::read_profile(
        &home.path().join("auth"),
        name,
        &*app.config().secrets,
    )
    .unwrap();
    session.cookies[0].expires = Some(100);
    call(&app, "auth.saveProfile", json!({"profile":{"name":name,"kind":"form","host":"127.0.0.1","username":null,"secret":serde_json::to_string(&session).unwrap()}})).await;
    assert_eq!(
        call(&app, "auth.loginStatus", Value::Null).await[0]["state"],
        "active"
    );
    clock.advance(Duration::from_secs(100));
    assert_eq!(
        call(&app, "auth.loginStatus", Value::Null).await[0]["state"],
        "expired"
    );
    assert!(
        call(&app, "config.get", Value::Null).await["config"]["auth"]["form"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

// spec: CRAWL-120
// spec: OUT-223
#[tokio::test]
async fn capturing_then_discarding_configuration_preserves_config_and_active_profile() {
    let _lock = CHROMIUM.lock().await;
    let site = FormLoginSite::start().await.unwrap();
    let home = tempfile::tempdir().unwrap();
    let app =
        App::new(AppConfig::new(home.path()).with_secrets(Arc::new(MemorySecretStore::new())))
            .unwrap();
    let original = call(&app, "config.get", Value::Null).await;
    let info = login(&app, &site).await;
    assert_eq!(info["kind"], "form");
    // Dropping the UI draft sends no config.set. The command itself must not activate it.
    assert_eq!(call(&app, "config.get", Value::Null).await, original);
    assert_eq!(
        call(&app, "auth.loginStatus", Value::Null).await[0]["cookies"],
        1
    );
    activate(&app, &site).await;
    let original = call(&app, "config.get", Value::Null).await;
    let active = original["config"]["auth"]["form"][0]["profile"]
        .as_str()
        .unwrap();
    let dir = home.path().join("auth");
    let before =
        os_engine::session_guard::read_profile(&dir, active, &*app.config().secrets).unwrap();
    let op = call(
        &app,
        "auth.openLogin",
        json!({"url":site.url("/login-2fa"),"profile":active}),
    )
    .await;
    let renewed = call(
        &app,
        "auth.finishLogin",
        json!({"opId":op["opId"],"saveAs":active}),
    )
    .await;
    assert_ne!(renewed["name"], active);
    assert_eq!(call(&app, "config.get", Value::Null).await, original);
    let after =
        os_engine::session_guard::read_profile(&dir, active, &*app.config().secrets).unwrap();
    assert_eq!(
        serde_json::to_value(before).unwrap(),
        serde_json::to_value(after).unwrap()
    );
    assert!(!original.to_string().contains("FIXTURE"));
}
