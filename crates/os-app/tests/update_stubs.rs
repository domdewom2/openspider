use os_app::{
    update::{self, DisableReason, UpdateContext, UpdateState},
    App, AppConfig, CallCtx, ErrorCode, WindowId,
};

// spec: OUT-250 [M3]
#[tokio::test]
async fn update_scaffold_is_fail_closed() {
    assert!(matches!(
        update::check(&UpdateContext, true).await,
        UpdateState::Disabled(DisableReason::NotImplemented)
    ));
    assert!(update::verify(
        include_bytes!("../../../tests/fixtures/update/manifest.json"),
        "invalid",
        ""
    )
    .is_err());
    assert!(matches!(
        update::download::download(&UpdateContext).await,
        UpdateState::Failed(_)
    ));
    assert!(matches!(
        update::install::install(&UpdateContext, update::InstallWhen::Now).await,
        UpdateState::Failed(_)
    ));
    let temp = tempfile::tempdir().unwrap();
    let app = App::new(AppConfig::new(temp.path())).unwrap();
    for (name, args) in [
        ("update.check", serde_json::Value::Null),
        ("update.status", serde_json::Value::Null),
        ("update.download", serde_json::Value::Null),
        ("update.install", serde_json::json!({"when":"afterCrawl"})),
        ("update.set_auto", serde_json::json!({"check":false})),
    ] {
        let error = app
            .dispatch(
                CallCtx {
                    window: WindowId("main".into()),
                    cancel: Default::default(),
                },
                name,
                args,
            )
            .await
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::Unsupported, "{name}");
        assert_eq!(error.message_key, "err-not-implemented");
        assert_eq!(error.args["command"], name);
        assert!(error.args["owner"].as_str().unwrap().starts_with("M3-U0"));
    }
}
