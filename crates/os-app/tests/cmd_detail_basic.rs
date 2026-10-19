#[path = "cmd_catalog_fixture.rs"]
mod fixture;
use fixture::*;
use serde_json::json;
// spec: UI-031
// spec: UI-081
// spec: TECH-066 [M2]
#[tokio::test]
async fn url_details_have_stable_order_null_fields_and_shared_table_values() {
    let fx = Fx::new();
    let rows = fx.call("detail.urlDetails", json!({"urlId":1})).await;
    assert!(rows.as_array().unwrap().len() >= 25);
    assert_eq!(rows[0]["name"], "Address");
    assert_eq!(rows[11]["name"], "Link Score");
    assert!(rows[11]["value"].is_null());
    assert_eq!(rows[24]["name"], "Segment");
    assert_eq!(rows[3]["value"], 200);
    assert_eq!(rows[9]["value"], 0.512);
    golden(
        "UrlDetails.json",
        &serde_json::json!(&rows.as_array().unwrap()[..25]),
    );
    let redirect = fx.call("detail.urlDetails", json!({"urlId":3})).await;
    assert_eq!(redirect[17]["value"], "https://site.test/blog/a");
    assert_eq!(redirect[18]["value"], "HTTP Redirect");
    let external = fx.call("detail.urlDetails", json!({"urlId":6})).await;
    assert_eq!(external[0]["value"], "https://external.test/");
    assert!(external[3]["value"].is_null());
}
// spec: UI-042
// spec: OUT-021 [P1]
#[tokio::test]
async fn headers_preserve_duplicate_order_and_redirect_response_and_redact_credentials() {
    let fx = Fx::new();
    let h = fx.call("detail.headers", json!({"urlId":1})).await;
    assert_eq!(h["statusLine"], "HTTP/2 200");
    assert_eq!(h["request"][0]["value"], "<redacted>");
    assert_eq!(h["response"][0]["name"], "Set-Cookie");
    assert_eq!(h["response"][1]["name"], "Set-Cookie");
    assert_eq!(h["response"][2]["value"], "ok");
    golden("HeadersDto.json", &h);
    let r = fx.call("detail.headers", json!({"urlId":3})).await;
    assert_eq!(r["statusLine"], "HTTP/1.1 301");
    assert_eq!(r["response"][0]["name"], "Location");
}

// spec: UI-042
#[tokio::test]
async fn headers_report_disabled_storage_instead_of_empty_lists() {
    let fx = Fx::new();
    fx.app.sessions().update_config(
        &os_app::pool::CrawlKey("fx-ui-basic".into()),
        std::sync::Arc::new(os_model::CrawlConfig::default()),
    );
    let e = fx
        .app
        .dispatch(
            os_app::CallCtx::for_window("main"),
            "detail.headers",
            json!({"urlId":1}),
        )
        .await
        .unwrap_err();
    assert_eq!(e.code, os_app::dto::ErrorCode::Unsupported);
    assert_eq!(e.message_key, "err-detail-unavailable");
    assert_eq!(e.args["state"]["code"], "FEATURE_DISABLED");
    assert_eq!(e.args["state"]["arg"], "extraction.http_headers");
}
