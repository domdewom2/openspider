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

async fn url_fixture(h: &Harness, count: usize) {
    h.app.write_queue().submit(h.key.clone(), move |conn| {
        conn.execute("DELETE FROM urls", [])?;
        for index in 0..count {
            let address = format!("https://integ.test/{index}");
            conn.execute("INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,folder_depth,url_length,content_class,in_tables) VALUES(?1,?2,?2,'https://integ.test',1,'fetched','seed',0,?3,'HTML',1)",rusqlite::params![index as i64 + 1,address,address.len()])?;
        }
        Ok(())
    }).await.unwrap();
}
async fn url_settings(
    h: &Harness,
    kind: &str,
    base: String,
    metric: &str,
    limit: Option<u64>,
    options: Value,
) {
    let settings = json!({"kind":kind,"values":{"enabled":true,"base_url":base,"metrics":[metric],"levels":["url"],"limits":{"concurrency":1,"retries":0,"quota_limit":limit},"options":options}});
    h.call("api.setSettings", json!({"kind":kind,"settings":settings}))
        .await
        .unwrap();
}
fn saved_ahrefs(h: &Harness, token: &str, expired: bool) {
    let now = h.app.config().clock.now();
    Credentials::new(h.app.config().secrets.clone())
        .put(
            ApiKind::Ahrefs,
            Credential {
                account_id: "workspace-1".into(),
                label: "Saved Workspace".into(),
                api_key: None,
                access_token: Some(token.into()),
                refresh_token: None,
                expires_at: Some(if expired {
                    now
                } else {
                    now.plus(Duration::from_secs(31536000))
                }),
                client_secret: None,
                scopes: vec![],
            },
            true,
        )
        .unwrap();
}
fn ahrefs_browser(h: &Harness, server: &MockServer, opened: Arc<AtomicU64>) {
    let state = h.app.ext::<ApiState>();
    state.set_ahrefs_oauth_endpoints(os_integrations::linkmetrics::oauth::Endpoints {
        authorize: format!("{}/authorize", server.base_url("ahrefs")),
        token: format!("{}/token", server.base_url("ahrefs")),
    });
    state.set_oauth_browser(Arc::new(move |url| {
        opened.fetch_add(1, Ordering::SeqCst);
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
            assert!(client
                .get(redirect)
                .send()
                .await
                .unwrap()
                .status()
                .is_success());
        });
        Ok(())
    }));
}

// spec: INTEG-153
#[tokio::test]
async fn selected_workspace_can_reauthorize_expired_and_revoked_credentials() {
    for expired in [true, false] {
        let h = Harness::new().await;
        let server = MockServer::start(vec![Arc::new(AhrefsMock::default())])
            .await
            .unwrap();
        url_settings(
            &h,
            "ahrefs",
            server.base_url("ahrefs"),
            "backlinks",
            None,
            json!({"workspace":"workspace-1","oauth_client_id":"fixture-client"}),
        )
        .await;
        saved_ahrefs(&h, if expired { "expired" } else { "bad" }, expired);
        let opened = Arc::new(AtomicU64::new(0));
        ahrefs_browser(&h, &server, opened.clone());
        let args = if expired {
            json!({"kind":"ahrefs","remember":true})
        } else {
            json!({"kind":"ahrefs","accountId":"workspace-1","remember":true})
        };
        let account = h.op("api.connect", args).await.unwrap();
        assert_eq!(account["accountId"], "workspace-1");
        assert_eq!(opened.load(Ordering::SeqCst), 1);
        let stored = Credentials::new(h.app.config().secrets.clone())
            .list(ApiKind::Ahrefs)
            .unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(
            stored[0].access_token.as_deref(),
            Some("ahrefs-fixture-token")
        );
        assert!(stored[0].expires_at.unwrap() > h.app.config().clock.now());
        server.stop().await;
        h.server.stop().await;
    }
}

// spec: INTEG-153
#[tokio::test]
async fn app_crawl_cap_one_sends_only_one_of_101_targets() {
    let h = Harness::new().await;
    let mock = Arc::new(AhrefsMock::default());
    let server = MockServer::start(vec![mock.clone()]).await.unwrap();
    url_fixture(&h, 101).await;
    url_settings(
        &h,
        "ahrefs",
        server.base_url("ahrefs"),
        "backlinks",
        Some(1),
        json!({}),
    )
    .await;
    saved_ahrefs(&h, "valid", false);
    h.op(
        "api.connect",
        json!({"kind":"ahrefs","accountId":"workspace-1","remember":false}),
    )
    .await
    .unwrap();
    let result = h
        .op(
            "api.fetch",
            json!({"kinds":["ahrefs"],"scope":{"kind":"all"}}),
        )
        .await
        .unwrap();
    assert_eq!(result["requested"], 101);
    assert_eq!(result["fetched"], 1);
    assert_eq!(result["failed"], 100);
    assert_eq!(mock.requests.all().len(), 1);
    assert_eq!(
        mock.requests.all()[0].body["targets"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let count = h
        .app
        .read_pool()
        .run(h.key.clone(), |file, _| {
            Ok(file.conn().query_row(
                "SELECT COUNT(*) FROM api_values WHERE source='ahrefs'",
                [],
                |row| row.get::<_, i64>(0),
            )?)
        })
        .await
        .unwrap();
    assert_eq!(count, 1);
    server.stop().await;
    h.server.stop().await;
}

// spec: INTEG-153
#[tokio::test]
async fn app_preserves_first_100_paid_results_when_second_bundle_fails() {
    let h = Harness::new().await;
    let mock = Arc::new(AhrefsMock::failing_from_batch(2));
    let server = MockServer::start(vec![mock.clone()]).await.unwrap();
    url_fixture(&h, 101).await;
    url_settings(
        &h,
        "ahrefs",
        server.base_url("ahrefs"),
        "backlinks",
        None,
        json!({}),
    )
    .await;
    saved_ahrefs(&h, "valid", false);
    h.op(
        "api.connect",
        json!({"kind":"ahrefs","accountId":"workspace-1","remember":false}),
    )
    .await
    .unwrap();
    let result = h
        .op(
            "api.fetch",
            json!({"kinds":["ahrefs"],"scope":{"kind":"all"}}),
        )
        .await
        .unwrap();
    assert_eq!(result["fetched"], 100);
    assert_eq!(result["failed"], 1);
    assert_eq!(mock.requests.all().len(), 2);
    let count = h
        .app
        .read_pool()
        .run(h.key.clone(), |file, _| {
            Ok(file.conn().query_row(
                "SELECT COUNT(*) FROM api_values WHERE source='ahrefs' AND v_text='5000000000'",
                [],
                |row| row.get::<_, i64>(0),
            )?)
        })
        .await
        .unwrap();
    assert_eq!(count, 100);
    server.stop().await;
    h.server.stop().await;
}

// spec: INTEG-152
// spec: INTEG-154
#[tokio::test]
async fn app_preserves_existing_values_and_errors_for_missing_or_failed_sites() {
    for (kind, metric) in [("majestic", "TrustFlow"), ("moz", "domain_authority")] {
        let h = Harness::new().await;
        let missing = vec!["https://integ.test/1".into()];
        let failed = vec!["https://integ.test/2".into()];
        let mock: Arc<dyn os_testsite::mocks::MockApi> = if kind == "majestic" {
            Arc::new(MajesticMock {
                missing_targets: missing,
                error_targets: failed,
                ..Default::default()
            })
        } else {
            Arc::new(MozMock {
                missing_targets: missing,
                error_targets: failed,
                ..Default::default()
            })
        };
        let server = MockServer::start(vec![mock]).await.unwrap();
        url_fixture(&h, 3).await;
        url_settings(
            &h,
            kind,
            server.base_url(kind),
            metric,
            None,
            json!({"moz_paid":true}),
        )
        .await;
        h.app
            .config()
            .secrets
            .set("api/majestic/openapp_private_key", "fixture-private")
            .unwrap();
        h.op(
            "api.connect",
            json!({"kind":kind,"apiKey":"valid","remember":false}),
        )
        .await
        .unwrap();
        let source = kind.to_string();
        let metric = metric.to_string();
        let now = h.app.config().clock.now();
        h.app.write_queue().submit(h.key.clone(), move |conn| {
            for id in 1..=3 {
                conn.execute("INSERT INTO api_values(url_id,source,metric,level,v_text,fetched_at) VALUES(?1,?2,?3,'url','17',?4)",rusqlite::params![id, source, metric, now.to_db()])?;
                os_store::queries::api::record_error(conn, &source, &format!("https://integ.test/{}",id-1), "old", "existing error", now)?;
            }
            Ok(())
        }).await.unwrap();
        let result = h
            .op("api.fetch", json!({"kinds":[kind],"scope":{"kind":"all"}}))
            .await
            .unwrap();
        assert_eq!(result["fetched"], 1);
        assert_eq!(result["failed"], 2);
        let source = kind.to_string();
        let (values, errors) = h
            .app
            .read_pool()
            .run(h.key.clone(), move |file, _| {
                let values = file
                    .conn()
                    .prepare("SELECT v_text FROM api_values WHERE source=?1 ORDER BY url_id")?
                    .query_map([&source], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                let errors = os_store::queries::api::error_count(file.conn(), &source)?;
                Ok((values, errors))
            })
            .await
            .unwrap();
        assert_eq!(
            values,
            [if kind == "majestic" { "75" } else { "65" }, "17", "17"]
        );
        assert_eq!(errors, 2);
        server.stop().await;
        h.server.stop().await;
    }
}

// spec: INTEG-153
#[tokio::test]
async fn provider_quota_exhaustion_still_stores_all_100_paid_results() {
    let h = Harness::new().await;
    let mock = Arc::new(AhrefsMock::with_unit_limit(250));
    let server = MockServer::start(vec![mock.clone()]).await.unwrap();
    url_fixture(&h, 101).await;
    url_settings(
        &h,
        "ahrefs",
        server.base_url("ahrefs"),
        "backlinks",
        None,
        json!({}),
    )
    .await;
    saved_ahrefs(&h, "valid", false);
    h.op(
        "api.connect",
        json!({"kind":"ahrefs","accountId":"workspace-1","remember":false}),
    )
    .await
    .unwrap();
    let result = h
        .op(
            "api.fetch",
            json!({"kinds":["ahrefs"],"scope":{"kind":"all"}}),
        )
        .await
        .unwrap();
    assert_eq!(result["fetched"], 100);
    assert_eq!(result["failed"], 1);
    assert_eq!(result["quotaLeft"], 0);
    assert_eq!(mock.requests.all().len(), 1);
    let count = h
        .app
        .read_pool()
        .run(h.key.clone(), |file, _| {
            Ok(file.conn().query_row(
                "SELECT COUNT(*) FROM api_values WHERE source='ahrefs'",
                [],
                |row| row.get::<_, i64>(0),
            )?)
        })
        .await
        .unwrap();
    assert_eq!(count, 100);
    server.stop().await;
    h.server.stop().await;
}
