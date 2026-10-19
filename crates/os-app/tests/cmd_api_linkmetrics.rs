// Additive I05-Tests des bestehenden Dispatchers (Planer-Entscheidung 19.10.).
#![allow(clippy::disallowed_methods)]
#[path = "cmd_api_support.rs"]
mod support;
use os_model::ApiLevel;
use os_testsite::mocks::{ahrefs::AhrefsMock, majestic::MajesticMock, moz::MozMock};
use support::*;
async fn configure(h: &Harness, kind: &str, base: String, metrics: &[&str], options: Value) {
    let settings = json!({"kind":kind,"values":{"enabled":true,"base_url":base,"metrics":metrics,"levels":["domain"],"limits":{"concurrency":1},"options":options}});
    h.call("api.setSettings", json!({"kind":kind,"settings":settings}))
        .await
        .unwrap();
}
// spec: INTEG-153
// spec: INTEG-151
#[tokio::test]
async fn ahrefs_connect_dispatches_oauth_without_key_and_fetches_only_eligible_urls() {
    let h = Harness::new().await;
    let mock = Arc::new(AhrefsMock::with_unit_limit(230));
    let server = MockServer::start(vec![mock.clone()]).await.unwrap();
    configure(
        &h,
        "ahrefs",
        server.base_url("ahrefs"),
        &["backlinks"],
        json!({"oauth_client_id":"fixture-client"}),
    )
    .await;
    let state = h.app.ext::<ApiState>();
    state.set_ahrefs_oauth_endpoints(os_integrations::linkmetrics::oauth::Endpoints {
        authorize: format!("{}/authorize", server.base_url("ahrefs")),
        token: format!("{}/token", server.base_url("ahrefs")),
    });
    state.set_oauth_browser(Arc::new(|url| {
        let url = url.to_string();
        tokio::spawn(async move {
            let client = os_integrations::http::client(&Default::default()).unwrap();
            let response = client.get(url).send().await.unwrap();
            let redirect = response
                .headers()
                .get("location")
                .unwrap()
                .to_str()
                .unwrap();
            client.get(redirect).send().await.unwrap();
        });
        Ok(())
    }));
    let account = h
        .op("api.connect", json!({"kind":"ahrefs","remember":false}))
        .await
        .unwrap();
    assert_eq!(account["account"], "Fixture Workspace");
    assert!(account["keySuffix"].is_null());
    assert_eq!(mock.log.calls().len(), 1);
    h.app.write_queue().submit(h.key.clone(),|conn| {
        conn.execute_batch("UPDATE urls SET content_class='HTML', in_tables=1; UPDATE urls SET is_internal=0 WHERE id=2; UPDATE urls SET content_class='PDF' WHERE id=3;")?; Ok(())
    }).await.unwrap();
    let result = h
        .op(
            "api.fetch",
            json!({"kinds":["ahrefs"],"scope":{"kind":"all"}}),
        )
        .await
        .unwrap();
    assert_eq!(result["requested"], 4);
    assert_eq!(result["fetched"], 4);
    assert_eq!(mock.requests.all().len(), 1);
    assert!(h
        .app
        .assemble_tick()
        .unwrap()
        .notices
        .iter()
        .any(|notice| notice.code == "notice-api-units-insufficient"));
    let values = h
        .app
        .read_pool()
        .run(h.key.clone(), |file, _| {
            Ok(file
                .conn()
                .prepare(
                    "SELECT url_id,v_text FROM api_values WHERE source='ahrefs' ORDER BY url_id",
                )?
                .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
                .collect::<Result<Vec<_>, _>>()?)
        })
        .await
        .unwrap();
    assert_eq!(values.len(), 4);
    assert!(values
        .iter()
        .all(|(id, text)| ![2, 3].contains(id) && text == "5000000000"));
    let mut settings = h
        .call("api.getSettings", json!({"kind":"ahrefs"}))
        .await
        .unwrap();
    settings["values"]["options"]["include_external"] = json!(true);
    h.call(
        "api.setSettings",
        json!({"kind":"ahrefs","settings":settings,"decision":"keep"}),
    )
    .await
    .unwrap();
    h.call("api.testConnection", json!({"kind":"ahrefs"}))
        .await
        .unwrap();
    let result = h
        .op(
            "api.fetch",
            json!({"kinds":["ahrefs"],"scope":{"kind":"all"}}),
        )
        .await
        .unwrap();
    assert_eq!(result["requested"], 5);
    assert_eq!(result["fetched"], 5);
    h.call("api.disconnect", json!({"kind":"ahrefs"}))
        .await
        .unwrap();
    let reconnect = h
        .op(
            "api.connect",
            json!({"kind":"ahrefs","accountId":account["accountId"]}),
        )
        .await
        .unwrap();
    assert_eq!(reconnect["accountId"], account["accountId"]);
    server.stop().await;
    h.server.stop().await;
}
// spec: INTEG-152
// spec: INTEG-154
#[tokio::test]
async fn token_providers_use_their_native_connection_probes() {
    let h = Harness::new().await;
    let server = MockServer::start(vec![
        Arc::new(MajesticMock::default()),
        Arc::new(MozMock::default()),
    ])
    .await
    .unwrap();
    h.app
        .config()
        .secrets
        .set("api/majestic/openapp_private_key", "fixture-private")
        .unwrap();
    for (kind, metric) in [("majestic", "TrustFlow"), ("moz", "domain_authority")] {
        configure(&h, kind, server.base_url(kind), &[metric], json!({})).await;
        let account = h
            .op(
                "api.connect",
                json!({"kind":kind,"apiKey":"fixture-token","remember":false}),
            )
            .await
            .unwrap();
        assert_eq!(account["keySuffix"], "oken");
        assert!(h
            .call("api.testConnection", json!({"kind":kind}))
            .await
            .unwrap()["ok"]
            .as_bool()
            .unwrap());
    }
    server.stop().await;
    h.server.stop().await;
}
// spec: INTEG-157
#[tokio::test]
async fn country_or_volume_change_asks_before_invalidating_existing_values() {
    let h = Harness::new().await;
    configure(
        &h,
        "ahrefs",
        h.server.base_url("common"),
        &["org_traffic"],
        json!({"country":"de"}),
    )
    .await;
    h.app.write_queue().submit(h.key.clone(),|conn| { conn.execute("INSERT INTO api_values(url_id,source,metric,level,v_text,fetched_at) VALUES(1,'ahrefs','org_traffic','domain','1200','2026-10-19T12:00:00Z')",[])?; Ok(()) }).await.unwrap();
    let mut settings = h
        .call("api.getSettings", json!({"kind":"ahrefs"}))
        .await
        .unwrap();
    settings["values"]["options"]["average_volume"] = json!(true);
    let result = h
        .call(
            "api.setSettings",
            json!({"kind":"ahrefs","settings":settings}),
        )
        .await
        .unwrap();
    assert_eq!(result["confirmationRequired"], true);
    let result = h
        .call(
            "api.setSettings",
            json!({"kind":"ahrefs","settings":settings,"decision":"keep"}),
        )
        .await
        .unwrap();
    assert_eq!(result["dataStale"], true);
    let stored = h
        .app
        .sessions()
        .current(&os_app::WindowId("main".into()))
        .unwrap()
        .config;
    assert_eq!(stored.api.ahrefs.levels, [ApiLevel::Domain]);
    assert!(stored.api.ahrefs.options.average_volume);
    h.server.stop().await;
}
