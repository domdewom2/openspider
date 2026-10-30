use os_app::{
    update::{self, DisableReason, UpdateContext, UpdateState},
    App, AppConfig, CallCtx, ErrorCode,
};

// spec: OUT-250
#[tokio::test]
async fn update_commands_are_connected_and_u02_remains_fail_closed() {
    assert!(matches!(
        update::check(
            &UpdateContext {
                pubkey: String::new(),
                disabled_by_environment: false,
                ..Default::default()
            },
            true
        )
        .await,
        UpdateState::Disabled(DisableReason::MissingPublicKey)
    ));
    let temp = tempfile::tempdir().unwrap();
    let app = App::new(AppConfig::new(temp.path())).unwrap();
    app.dispatch(
        CallCtx::for_window("main"),
        "update.set_auto",
        serde_json::json!({"check":false}),
    )
    .await
    .unwrap();
    let state = app
        .dispatch(
            CallCtx::for_window("main"),
            "update.check",
            serde_json::Value::Null,
        )
        .await
        .unwrap();
    assert_eq!(state, serde_json::json!({"disabled":"settings"}));
    assert!(!app.settings_snapshot().updates.check);
    assert!(std::fs::read_to_string(app.settings().path())
        .unwrap()
        .contains("check = false"));
    for (command, args) in [
        ("update.download", serde_json::Value::Null),
        ("update.install", serde_json::json!({"when":"afterCrawl"})),
    ] {
        assert_eq!(
            app.dispatch(CallCtx::for_window("main"), command, args)
                .await
                .unwrap_err()
                .code,
            ErrorCode::Unsupported
        );
    }
}
