//! Vollständige Integrationsfälle gegen die noch ausstehenden F05-Fachrümpfe (K2-26).
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "runtime_common.rs"]
mod common;
use serde_json::{json, Value};

async fn call(app: &os_app::App, name: &str, args: Value) -> Value {
    app.dispatch(common::ctx(), name, args).await.unwrap()
}

// spec: CRAWL-126 [M2]
#[tokio::test]
async fn rules_test_returns_per_rule_matches_from_the_url_trace() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let result = app.dispatch(common::ctx(),"config.testRules",json!({"kind":"includeExclude","input":"https://site.test/blog/one","rules":[{"pattern":"/blog/","include":true},{"pattern":"/admin/","include":false}],"regexPartialMatch":true})).await.unwrap();
    assert_eq!(result["included"], true);
    assert_eq!(result["steps"][0]["matched"], true);
    assert_eq!(result["steps"][1]["matched"], false);
}

// spec: CRAWL-126 [M2]
#[tokio::test]
async fn rules_test_passes_query_removal_and_ordered_replacements() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let result = app.dispatch(common::ctx(),"config.testRules",json!({"kind":"rewrite","input":"https://site.test/old/one?tracking=1&keep=2","removeParameters":["tracking"],"rules":[{"pattern":"/old/","replacement":"/new/"},{"pattern":"/new/one","replacement":"/new/two"}]})).await.unwrap();
    assert_eq!(result["output"], "https://site.test/new/two?keep=2");
    assert_eq!(
        result["steps"][0]["before"],
        "https://site.test/old/one?keep=2"
    );
    assert_eq!(
        result["steps"][0]["after"],
        "https://site.test/new/one?keep=2"
    );
    assert_eq!(result["steps"][1]["matched"], true);
}

// spec: INTEG-082 [M2]
#[tokio::test]
#[ignore = "Klärung K2-68: N01-C13, Besitzer M2-N01b (config.rs an recompute/d16 anbinden)"]
async fn config_set_reclassifies_stored_link_paths_without_requests() {
    use os_app::{dto::CrawlConfigDto, pool::CrawlKey, runtime::sessions::SessionInfo, WindowId};
    use os_model::{CrawlConfig, CrawlState, LinkPositionRule};
    use std::sync::Arc;
    let fx = os_engine::testing::crawl_fixture("crawl-site-100", CrawlConfig::default()).await;
    let app = common::new_app(fx.dir.path());
    let key = CrawlKey("positions".into());
    let path = fx.file.path().to_path_buf();
    app.register_crawl(key.clone(), path.clone());
    app.sessions().set_current(
        WindowId("main".into()),
        SessionInfo {
            key: key.clone(),
            path,
            config: Arc::new(CrawlConfig::default()),
            state: CrawlState::Completed,
            writable: true,
            engine: None,
        },
    );
    let requests = fx.site.requests().len();
    let mut cfg = CrawlConfig::default();
    cfg.link_positions.rules.insert(
        0,
        LinkPositionRule {
            position: "Body".into(),
            contains: vec!["body".into()],
        },
    );
    let dto = CrawlConfigDto {
        schema_version: cfg.schema_version,
        generator: cfg.generator.clone(),
        config: serde_json::to_value(&cfg).unwrap(),
    };
    let result = app
        .dispatch(
            common::ctx(),
            "config.set",
            serde_json::to_value(dto).unwrap(),
        )
        .await
        .unwrap();
    assert!(result["invalidatedSteps"]
        .as_array()
        .unwrap()
        .contains(&json!("links")));
    let links = app
        .read_pool()
        .run(key, |file, _| {
            // Ein vorhandener Detail-Leser prüft die gespeicherte Klassifikation.
            let mut stmt = file
                .conn()
                .prepare("SELECT link_position FROM links WHERE link_position IS NOT NULL")?;
            let positions = stmt
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(positions)
        })
        .await
        .unwrap();
    assert!(links.iter().any(|position| position == "Body"));
    assert_eq!(fx.site.requests().len(), requests);
}

// spec: INTEG-002 [M2]
#[tokio::test]
#[ignore = "Klärung K2-68: N01-C12, Besitzer M2-N01b (includeSecrets ist noch Stub)"]
async fn export_uses_explicit_consent_and_authenticated_password_encryption() {
    let dir = tempfile::tempdir().unwrap();
    let secrets = std::sync::Arc::new(os_fetch::MemorySecretStore::new());
    secrets.set("api/psi", "sk-TEST-INTEG-002");
    let app = os_app::App::new(os_app::AppConfig::new(dir.path()).with_secrets(secrets)).unwrap();
    let cfg = os_model::CrawlConfig::load_str(include_str!(
        "../../../tests/fixtures/config/commented.osconfig"
    ))
    .unwrap()
    .config;
    let config = os_app::dto::CrawlConfigDto {
        schema_version: cfg.schema_version,
        generator: cfg.generator.clone(),
        config: serde_json::to_value(&cfg).unwrap(),
    };
    call(
        &app,
        "profile.save",
        json!({"name":"safe","config":config,"overwrite":false}),
    )
    .await;
    let plain = dir.path().join("plain.osconfig");
    let encrypted = dir.path().join("encrypted.osconfig");
    call(&app, "profile.export", json!({"name":"safe","path":plain})).await;
    assert!(!std::fs::read_to_string(&plain)
        .unwrap()
        .contains("sk-TEST-INTEG-002"));
    let err = app
        .dispatch(
            common::ctx(),
            "profile.export",
            json!({"name":"safe","path":encrypted,"includeSecrets":true}),
        )
        .await
        .unwrap_err();
    assert_eq!(err.field.as_deref(), Some("password"));
    assert!(!encrypted.exists());
    call(
        &app,
        "profile.export",
        json!({"name":"safe","path":encrypted,"includeSecrets":true,"password":"fixture-password"}),
    )
    .await;
    let bytes = std::fs::read(&encrypted).unwrap();
    assert!(!bytes
        .windows(b"sk-TEST-INTEG-002".len())
        .any(|w| w == b"sk-TEST-INTEG-002"));
    // Passwort-/Manipulationsprüfungen des Helfers gehören M2-I01.
    // Plaintext secrets never appear in any stored profile, index, default or exported file.
    for path in [plain, encrypted, dir.path().join("profiles/safe.osconfig")] {
        assert!(!std::fs::read(path)
            .unwrap()
            .windows(b"sk-TEST-INTEG-002".len())
            .any(|w| w == b"sk-TEST-INTEG-002"));
    }
}
