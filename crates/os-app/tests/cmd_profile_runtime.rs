#[path = "runtime_common.rs"]
mod common;
use os_app::{
    dto::CrawlConfigDto, pool::CrawlKey, runtime::sessions::SessionInfo, App, AppConfig, ErrorCode,
    WindowId,
};
use os_model::{CrawlConfig, CrawlState};
use serde_json::{json, Value};
use std::sync::Arc;
fn dto(cfg: &CrawlConfig) -> Value {
    serde_json::to_value(CrawlConfigDto {
        schema_version: cfg.schema_version,
        generator: cfg.generator.clone(),
        config: serde_json::to_value(cfg).unwrap(),
    })
    .unwrap()
}
async fn call(app: &App, name: &str, args: Value) -> Value {
    app.dispatch(common::ctx(), name, args).await.unwrap()
}

// spec: OUT-220 [M2]
// spec: CRAWL-132
// spec: CRAWL-102 [M2]
#[tokio::test]
async fn profile_commands_roundtrip_import_export_default_delete() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let mut cfg = CrawlConfig::load_str(include_str!(
        "../../../tests/fixtures/config/commented.osconfig"
    ))
    .unwrap()
    .config;
    cfg.generator = CrawlConfig::default().generator;
    cfg.robots.custom_files.insert(
        "sub.site.test".into(),
        "User-agent: *\nDisallow: /private".into(),
    );
    cfg.ui
        .columns
        .insert("internal".into(), vec!["address".into()]);
    cfg.scope.include.push("https://.*".into());
    let original = dto(&cfg);
    assert_eq!(call(&app, "profile.list", Value::Null).await, json!([]));
    let info = call(
        &app,
        "profile.save",
        json!({"name":"SEO","config":original,"overwrite":false}),
    )
    .await;
    assert_eq!(info["name"], "SEO");
    assert_eq!(info["isDefault"], false);
    assert!(app
        .dispatch(
            common::ctx(),
            "profile.save",
            json!({"name":"SEO","config":original,"overwrite":false})
        )
        .await
        .is_err());
    assert_eq!(
        call(&app, "profile.load", json!({"name":"SEO"})).await,
        original
    );
    // load is only a draft for the dialog; applying needs config.set.
    assert_eq!(
        call(&app, "config.get", Value::Null).await,
        dto(&CrawlConfig::default())
    );
    call(&app, "profile.setDefault", json!({"name":"SEO"})).await;
    assert_eq!(
        call(&app, "profile.list", Value::Null).await[0]["isDefault"],
        true
    );
    let restarted = App::new(AppConfig::new(dir.path())).unwrap();
    assert_eq!(call(&restarted, "config.get", Value::Null).await, original);
    let path = dir.path().join("Export.osconfig");
    call(&app, "profile.export", json!({"name":"SEO","path":path})).await;
    assert_eq!(CrawlConfig::load_file(&path).unwrap().config, cfg);
    assert_eq!(
        std::fs::read(&path).unwrap(),
        std::fs::read(info["path"].as_str().unwrap()).unwrap()
    );
    assert_eq!(
        call(&app, "profile.import", json!({"path":path})).await["name"],
        "Export"
    );
    call(&app, "profile.setDefault", json!({"name":null})).await;
    assert_eq!(
        call(&app, "config.get", Value::Null).await,
        dto(&CrawlConfig::default())
    );
    call(&app, "profile.delete", json!({"name":"SEO"})).await;
    call(&app, "profile.delete", json!({"name":"Export"})).await;
    assert_eq!(call(&app, "profile.list", Value::Null).await, json!([]));
}

// spec: OUT-220 [M2]
// spec: CRAWL-126 [M2]
#[tokio::test]
async fn running_rejects_profile_load_and_paused_requires_explicit_apply() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    call(
        &app,
        "profile.save",
        json!({"name":"default","config":dto(&CrawlConfig::default()),"overwrite":false}),
    )
    .await;
    let mut session = SessionInfo {
        key: CrawlKey("fixture".into()),
        path: common::make_crawl(dir.path(), "crawl.ospider"),
        config: Arc::new(CrawlConfig::default()),
        state: CrawlState::Running,
        writable: true,
        engine: None,
    };
    app.sessions()
        .set_current(WindowId("main".into()), session.clone());
    assert_eq!(
        app.dispatch(common::ctx(), "profile.load", json!({"name":"default"}))
            .await
            .unwrap_err()
            .code,
        ErrorCode::CrawlRunning
    );
    session.state = CrawlState::Paused;
    app.sessions().set_current(WindowId("main".into()), session);
    assert_eq!(
        call(&app, "profile.load", json!({"name":"default"})).await,
        dto(&CrawlConfig::default())
    );
}

// spec: CRAWL-132
#[tokio::test]
async fn partial_unknown_future_and_malformed_file_handling() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    for (name, source) in [
        (
            "partial",
            include_str!("../../../tests/fixtures/config/partial.osconfig"),
        ),
        (
            "unknown",
            include_str!("../../../tests/fixtures/config/unknown-fields.osconfig"),
        ),
        (
            "future",
            include_str!("../../../tests/fixtures/config/future-version.osconfig"),
        ),
    ] {
        let path = dir.path().join(format!("{name}.osconfig"));
        std::fs::write(&path, source).unwrap();
        call(&app, "profile.import", json!({"path":path})).await;
        call(&app, "profile.load", json!({"name":name})).await;
    }
    let tick = app.assemble_tick().unwrap();
    assert!(tick
        .notices
        .iter()
        .any(|n| n.code == "notice-config-warning"));
    let path = dir.path().join("invalid.osconfig");
    std::fs::write(
        &path,
        include_str!("../../../tests/fixtures/config/invalid/type-error.osconfig"),
    )
    .unwrap();
    let err = app
        .dispatch(common::ctx(), "profile.import", json!({"path":path}))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidInput);
    assert!(err.args.contains_key("line"));
    assert!(err.args.contains_key("column"));
    assert_eq!(
        call(&app, "profile.list", Value::Null)
            .await
            .as_array()
            .unwrap()
            .len(),
        3
    );
}

// spec: INTEG-002 [M2]
#[tokio::test]
async fn encrypted_exports_cannot_replace_managed_or_default_profiles() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    for name in ["source", "other"] {
        call(
            &app,
            "profile.save",
            json!({"name":name,"config":dto(&CrawlConfig::default()),"overwrite":false}),
        )
        .await;
    }
    call(&app, "profile.setDefault", json!({"name":"other"})).await;
    for path in [
        dir.path().join("profiles/other.osconfig"),
        CrawlConfig::default_profile_path(dir.path()),
    ] {
        let before = std::fs::read(&path).unwrap();
        let err = app.dispatch(common::ctx(),"profile.export",json!({"name":"source","path":path,"includeSecrets":true,"password":"fixture-password"})).await.unwrap_err();
        assert_eq!(err.field.as_deref(), Some("path"));
        assert_eq!(std::fs::read(path).unwrap(), before);
    }
}

// spec: OUT-220 [M2]
// spec: CRAWL-132
#[tokio::test]
async fn import_name_collision_reports_path_and_preserves_existing_profile() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let source = dir.path().join("SEO.osconfig");
    std::fs::write(
        &source,
        include_str!("../../../tests/fixtures/config/commented.osconfig"),
    )
    .unwrap();
    call(&app, "profile.import", json!({"path":source})).await;
    let profile = dir.path().join("profiles/SEO.osconfig");
    let before = std::fs::read(&profile).unwrap();
    CrawlConfig::default().save_file(&source).unwrap();
    let err = app
        .dispatch(common::ctx(), "profile.import", json!({"path":source}))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidInput);
    assert_eq!(err.field.as_deref(), Some("path"));
    assert_eq!(err.args["reason"], "exists");
    assert_eq!(std::fs::read(profile).unwrap(), before);
}

// spec: OUT-220 [M2]
// spec: INTEG-002 [M2]
#[tokio::test]
async fn exports_require_absolute_paths() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    call(
        &app,
        "profile.save",
        json!({"name":"safe","config":dto(&CrawlConfig::default()),"overwrite":false}),
    )
    .await;
    for path in ["relative.osconfig", "../relative.osconfig", ""] {
        for include_secrets in [false, true] {
            let err = app.dispatch(common::ctx(), "profile.export", json!({"name":"safe","path":path,"includeSecrets":include_secrets,"password":"fixture-password"})).await.unwrap_err();
            assert_eq!(err.code, ErrorCode::InvalidInput);
            assert_eq!(err.field.as_deref(), Some("path"));
        }
    }
}

// spec: INTEG-002 [M2]
#[tokio::test]
async fn secret_references_and_plain_exports_never_persist_credentials() {
    fn scan(path: &std::path::Path) {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                scan(&path);
            } else {
                assert!(
                    !std::fs::read(&path)
                        .unwrap()
                        .windows(b"sk-TEST-INTEG-002".len())
                        .any(|w| w == b"sk-TEST-INTEG-002"),
                    "{}",
                    path.display()
                );
            }
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(os_fetch::MemorySecretStore::new());
    secrets.set("api/psi", "sk-TEST-INTEG-002");
    let app = App::new(AppConfig::new(dir.path()).with_secrets(secrets)).unwrap();
    let mut cfg = CrawlConfig::load_str(include_str!(
        "../../../tests/fixtures/config/commented.osconfig"
    ))
    .unwrap()
    .config;
    cfg.generator = CrawlConfig::default().generator;
    call(
        &app,
        "profile.save",
        json!({"name":"safe","config":dto(&cfg),"overwrite":false}),
    )
    .await;
    call(&app, "profile.setDefault", json!({"name":"safe"})).await;
    call(&app, "profile.load", json!({"name":"safe"})).await;
    for include_secrets in [None, Some(false)] {
        let plain = dir
            .path()
            .join(format!("plain-{include_secrets:?}.osconfig"));
        call(
            &app,
            "profile.export",
            json!({"name":"safe","path":plain,"includeSecrets":include_secrets}),
        )
        .await;
        assert_eq!(CrawlConfig::load_file(&plain).unwrap().config, cfg);
    }
    let encrypted = dir.path().join("encrypted.osconfig");
    let err = app
        .dispatch(
            common::ctx(),
            "profile.export",
            json!({"name":"safe","path":encrypted,"includeSecrets":true}),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidInput);
    assert_eq!(err.field.as_deref(), Some("password"));
    // INTEG-002 [M2]: with the explicit consent and a password the export is written, sealed (not Unsupported anymore).
    call(&app, "profile.export", json!({"name":"safe","path":encrypted,"includeSecrets":true,"password":"fixture-password"})).await;
    assert!(encrypted.exists());
    scan(dir.path());
}
