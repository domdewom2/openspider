use os_app::dto::{RuleNode, RuleOp};
use os_app::{App, AppConfig, CallCtx, ErrorCode};
use serde_json::json;

// spec: UI-011 [P1]
#[tokio::test]
async fn parse_dispatch_returns_contract_dto_and_character_position() {
    let dir = tempfile::tempdir().unwrap();
    let app = App::new(AppConfig::new(dir.path())).unwrap();
    let result = app
        .dispatch(
            CallCtx::for_window("main"),
            "search.parse",
            json!({"text": "Address ~ \"/blog/\" AND \"Word Count\" < 200"}),
        )
        .await
        .unwrap();
    assert_eq!(
        result,
        json!({"kind":"group","op":"and","children":[
            {"kind":"cond","column":"Address","op":"contains","value":"/blog/"},
            {"kind":"cond","column":"Word Count","op":"lessThan","value":"200"}
        ]})
    );
    let dto: RuleNode = serde_json::from_value(result).unwrap();
    let core = os_app::commands::search::to_core(&dto);
    assert_eq!(
        core,
        os_analyze::search::parse("Address ~ \"/blog/\" AND \"Word Count\" < 200").unwrap()
    );
    let error = app
        .dispatch(
            CallCtx::for_window("main"),
            "search.parse",
            json!({"text":"Ä = x AND"}),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidInput);
    assert_eq!(error.field.as_deref(), Some("text"));
    assert_eq!(error.args["position"], 9);
    assert_eq!(
        app.dispatch(CallCtx::for_window("main"), "search.parse", json!({}))
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
}

// spec: UI-011 [P1]
#[tokio::test]
async fn parse_handles_all_operators_and_boolean_normalization() {
    let dir = tempfile::tempdir().unwrap();
    let app = App::new(AppConfig::new(dir.path())).unwrap();
    for query in [
        "Address ^= blog",
        "Address $= html",
        "Address != x",
        "Address !~ x",
        "Address NOTREGEX x",
        "Address IS EMPTY",
        "Address IS NOT EMPTY",
        "NOT (Address ~ blog OR Address = shop)",
        "\"Word Count\" >= 200",
        "NOT (\"Word Count\" < 200)",
        "* ~ blog",
    ] {
        let result = app
            .dispatch(
                CallCtx::for_window("main"),
                "search.parse",
                json!({"text":query}),
            )
            .await
            .unwrap();
        let dto: RuleNode = serde_json::from_value(result).unwrap();
        let core = os_app::commands::search::to_core(&dto);
        assert!(
            os_analyze::search::parse(&os_analyze::search::format(&core)).is_ok(),
            "{query}"
        );
    }
    let unsupported = app
        .dispatch(
            CallCtx::for_window("main"),
            "search.parse",
            json!({"text":"NOT Address ^= x"}),
        )
        .await
        .unwrap_err();
    assert_eq!(unsupported.code, ErrorCode::InvalidInput);
    let result = app
        .dispatch(
            CallCtx::for_window("main"),
            "search.parse",
            json!({"text":"* ~ blog"}),
        )
        .await
        .unwrap();
    assert!(matches!(
        serde_json::from_value::<RuleNode>(result).unwrap(),
        RuleNode::Cond {
            column: None,
            op: RuleOp::Contains,
            ..
        }
    ));
}

// spec: UI-011 [P1]
#[tokio::test]
async fn validate_regex_uses_core_dialect_and_keeps_command_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let app = App::new(AppConfig::new(dir.path())).unwrap();
    for pattern in [r"/([^/]+)/\1/", r"(?<=/)blog(?=/)", "(?i)blog"] {
        assert!(app
            .dispatch(
                CallCtx::for_window("main"),
                "search.validateRegex",
                json!({"pattern":pattern})
            )
            .await
            .unwrap()
            .is_null());
    }
    let err = app
        .dispatch(
            CallCtx::for_window("main"),
            "search.validateRegex",
            json!({"pattern":"["}),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidInput);
    assert_eq!(err.field.as_deref(), Some("pattern"));
    assert!(err.detail.is_some());
    let ctx = CallCtx::for_window("main");
    ctx.cancel.cancel();
    assert_eq!(
        app.dispatch(ctx, "search.parse", json!({"text":"Address = x"}))
            .await
            .unwrap_err()
            .code,
        ErrorCode::Cancelled
    );
    for name in ["search.parse", "search.validateRegex"] {
        let meta = app
            .registry()
            .manifest()
            .into_iter()
            .find(|m| m.name == name)
            .unwrap();
        assert!(!meta.long_running);
    }
}
