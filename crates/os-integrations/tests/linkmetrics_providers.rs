use os_fetch::MemorySecretStore;
use os_integrations::{
    credentials::{Credential, Credentials},
    linkmetrics::{self, LinkMetrics},
    ApiCtx, ApiError, ApiSource, ApiState, UrlRef,
};
use os_model::{
    config::{LinkMetricsConfig, LinkMetricsOptions},
    ApiKind, ApiLevel, Clock, CrawlConfig, ManualClock, Timestamp, UrlId,
};
use os_testsite::mocks::{ahrefs::AhrefsMock, majestic::MajesticMock, moz::MozMock, MockServer};
use std::{sync::Arc, time::Duration};
fn clock() -> Arc<ManualClock> {
    Arc::new(ManualClock::new(
        Timestamp::from_db("2026-10-19T12:00:00Z").unwrap(),
    ))
}
fn urls(n: usize) -> Vec<UrlRef> {
    (0..n)
        .map(|i| UrlRef {
            url_id: Some(UrlId(i as i64 + 1)),
            url: format!("https://www.integ.test/{i}"),
        })
        .collect()
}
fn credential(kind: ApiKind) -> Credential {
    Credential {
        account_id: "fixture".into(),
        label: "Fixture".into(),
        api_key: if kind == ApiKind::Ahrefs {
            None
        } else {
            Some("fixture-token".into())
        },
        access_token: (kind == ApiKind::Ahrefs).then(|| "fixture-token".into()),
        refresh_token: None,
        expires_at: Some(clock().now().plus(Duration::from_secs(31536000))),
        client_secret: None,
        scopes: vec![],
    }
}
fn context(
    kind: ApiKind,
    base: String,
    levels: Vec<ApiLevel>,
    metrics: &[&str],
    options: LinkMetricsOptions,
) -> ApiCtx {
    let secrets = Arc::new(MemorySecretStore::default());
    secrets.set("api/majestic/openapp_private_key", "fixture-private");
    let credentials = Arc::new(Credentials::new(secrets));
    credentials.put(kind, credential(kind), false).unwrap();
    let mut cfg = CrawlConfig::default();
    let source = LinkMetricsConfig {
        enabled: true,
        base_url: Some(base),
        levels,
        metrics: metrics.iter().map(|s| s.to_string()).collect(),
        options,
        ..Default::default()
    };
    match kind {
        ApiKind::Majestic => cfg.api.majestic = source,
        ApiKind::Ahrefs => cfg.api.ahrefs = source,
        _ => cfg.api.moz = source,
    }
    ApiCtx {
        cfg,
        secrets: credentials,
        network: Default::default(),
        allow_api_override: true,
    }
}
// spec: INTEG-150
#[tokio::test]
async fn domain_is_psl_based_deduplicated_and_distributed_to_every_url() {
    let mock = Arc::new(AhrefsMock::default());
    let server = MockServer::start(vec![mock.clone()]).await.unwrap();
    let ctx = context(
        ApiKind::Ahrefs,
        server.base_url("ahrefs"),
        vec![ApiLevel::Domain],
        &["backlinks"],
        Default::default(),
    );
    let source = LinkMetrics::with_clock(ApiKind::Ahrefs, clock());
    let data = source.fetch(&urls(100), &ctx).await.unwrap();
    assert_eq!(data.len(), 100);
    assert!(data
        .iter()
        .all(|v| v.v_text.as_deref() == Some("5000000000")));
    assert_eq!(mock.requests.all().len(), 1);
    assert_eq!(
        mock.requests.all()[0].body["targets"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        mock.requests.all()[0].body["targets"][0]["url"],
        "https://integ.test"
    );
    assert_eq!(
        linkmetrics::target("https://blog.example.co.uk/x", ApiLevel::Domain)
            .unwrap()
            .query,
        "example.co.uk"
    );
    assert_eq!(
        linkmetrics::target("http://x.test/x", ApiLevel::UrlBoth).unwrap(),
        linkmetrics::target("https://x.test/x", ApiLevel::UrlBoth).unwrap()
    );
    assert!(!linkmetrics::catalog::supports_level(
        ApiKind::Moz,
        ApiLevel::UrlBoth
    ));
    server.stop().await;
}
// spec: INTEG-152
#[tokio::test]
async fn majestic_batches_100_uses_selected_index_and_preserves_provider_error_safely() {
    let mock = Arc::new(MajesticMock::default());
    let server = MockServer::start(vec![mock.clone()]).await.unwrap();
    let mut ctx = context(
        ApiKind::Majestic,
        server.base_url("majestic"),
        vec![ApiLevel::Url],
        &["TrustFlow", "CitationFlow", "ExtBackLinks"],
        LinkMetricsOptions {
            historic: true,
            ..Default::default()
        },
    );
    let source = LinkMetrics::with_clock(ApiKind::Majestic, clock());
    assert_eq!(source.fetch(&urls(101), &ctx).await.unwrap().len(), 303);
    let calls = mock.requests.all();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].body["items"], "100");
    assert_eq!(calls[1].body["items"], "1");
    assert_eq!(calls[0].body["datasource"], "historic");
    ctx.cfg.api.majestic.options.historic = false;
    source.fetch(&urls(1), &ctx).await.unwrap();
    assert_eq!(mock.requests.all()[2].body["datasource"], "fresh");
    let mut bad = credential(ApiKind::Majestic);
    bad.api_key = Some("bad".into());
    let error = source.test_connection(&ctx, &bad).await.unwrap_err();
    assert!(error.public_message().contains("API permission missing"));
    assert!(error.public_message().contains("[redacted]"));
    assert!(!calls[0].body.to_string().contains("fixture-private"));
    server.stop().await;
}
// spec: INTEG-154
#[tokio::test]
async fn moz_paid_batches_50_and_maps_counts_at_each_level() {
    let mock = Arc::new(MozMock::default());
    let server = MockServer::start(vec![mock.clone()]).await.unwrap();
    let ctx = context(
        ApiKind::Moz,
        server.base_url("moz"),
        vec![ApiLevel::Url],
        &[
            "page_authority",
            "domain_authority",
            "root_domains_linking",
            "brand_authority_score",
        ],
        LinkMetricsOptions {
            moz_paid: true,
            ..Default::default()
        },
    );
    let source = LinkMetrics::with_clock(ApiKind::Moz, clock());
    let data = source.fetch(&urls(51), &ctx).await.unwrap();
    assert_eq!(data.len(), 204);
    assert_eq!(mock.requests.all().len(), 3);
    assert_eq!(
        mock.requests.all()[2].body["method"],
        "data.site.metrics.brand.authority.fetch"
    );
    assert_eq!(
        mock.requests.all()[0]
            .body
            .pointer("/params/data/site_queries")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        50
    );
    assert_eq!(
        data.iter()
            .find(|v| v.metric == "root_domains_linking")
            .unwrap()
            .v_text
            .as_deref(),
        Some("5000000000")
    );
    server.stop().await;
}
// spec: INTEG-154
#[tokio::test(start_paused = true)]
async fn moz_free_throttles_across_calls_and_rejects_paid_metrics() {
    let mock = Arc::new(MozMock::default());
    let server = MockServer::start(vec![mock.clone()]).await.unwrap();
    let mut ctx = context(
        ApiKind::Moz,
        server.base_url("moz"),
        vec![ApiLevel::Url],
        &["page_authority"],
        Default::default(),
    );
    let source = Arc::new(LinkMetrics::with_clock(ApiKind::Moz, clock()));
    // Keep IO from advancing the paused clock to the HTTP timeout.
    let io_guard = tokio::spawn(async {
        loop {
            tokio::task::yield_now().await;
        }
    });
    source.fetch(&urls(1), &ctx).await.unwrap();
    let task = {
        let source = source.clone();
        let ctx = ctx.clone();
        tokio::spawn(async move { source.fetch(&urls(1), &ctx).await })
    };
    tokio::time::advance(Duration::from_secs(10)).await;
    task.await.unwrap().unwrap();
    let calls = mock.requests.all();
    assert!(calls[1].at.duration_since(calls[0].at) >= Duration::from_secs(10));
    ctx.cfg.api.moz.metrics = vec!["link_propensity".into()];
    assert!(source.fetch(&urls(1), &ctx).await.is_err());
    assert_eq!(mock.requests.all().len(), 2);
    io_guard.abort();
    server.stop().await;
}
// spec: INTEG-157
#[tokio::test]
async fn ahrefs_country_and_volume_apply_only_to_traffic_and_survive_profiles() {
    let mock = Arc::new(AhrefsMock::default());
    let server = MockServer::start(vec![mock.clone()]).await.unwrap();
    let mut ctx = context(
        ApiKind::Ahrefs,
        server.base_url("ahrefs"),
        vec![ApiLevel::Url],
        &["backlinks", "org_traffic", "org_keywords_top10"],
        LinkMetricsOptions {
            country: Some("DE".into()),
            ..Default::default()
        },
    );
    let source = LinkMetrics::with_clock(ApiKind::Ahrefs, clock());
    let monthly = source.fetch(&urls(1), &ctx).await.unwrap();
    assert_eq!(
        monthly
            .iter()
            .find(|v| v.metric == "org_traffic")
            .unwrap()
            .v_text
            .as_deref(),
        Some("1200")
    );
    assert_eq!(
        monthly
            .iter()
            .find(|v| v.metric == "org_keywords_top10")
            .unwrap()
            .v_text
            .as_deref(),
        Some("84")
    );
    ctx.cfg.api.ahrefs.options.average_volume = true;
    let average = source.fetch(&urls(1), &ctx).await.unwrap();
    assert_eq!(
        average
            .iter()
            .find(|v| v.metric == "org_traffic")
            .unwrap()
            .v_text
            .as_deref(),
        Some("950")
    );
    let calls = mock.requests.all();
    assert!(calls[0].body.get("country").is_none());
    assert!(calls[0].body.get("volume_mode").is_none());
    assert_eq!(calls[1].body["country"], "de");
    assert_eq!(calls[3].body["volume_mode"], "average");
    let loaded = CrawlConfig::load_str(&ctx.cfg.to_osconfig_string().unwrap()).unwrap();
    assert_eq!(loaded.config.api.ahrefs, ctx.cfg.api.ahrefs);
    assert!(loaded.warnings.is_empty());
    ctx.cfg.api.ahrefs.options.country = None;
    source.fetch(&urls(1), &ctx).await.unwrap();
    assert!(mock.requests.all()[5].body.get("country").is_none());
    server.stop().await;
}
// spec: INTEG-153
#[tokio::test]
async fn ahrefs_oauth_uses_pkce_has_one_year_expiry_and_reports_units() {
    let mock = Arc::new(AhrefsMock::default());
    let server = MockServer::start(vec![mock.clone()]).await.unwrap();
    let endpoints = linkmetrics::oauth::Endpoints {
        authorize: format!("{}/authorize", server.base_url("ahrefs")),
        token: format!("{}/token", server.base_url("ahrefs")),
    };
    let auth = linkmetrics::oauth::Authorization::begin("fixture-client", &endpoints)
        .await
        .unwrap();
    let url = auth.url.clone();
    let browser = tokio::spawn(async move {
        let response = reqwest::Client::new().get(url).send().await.unwrap();
        assert!(response.status().is_success());
    });
    let credential = auth
        .finish(
            &reqwest::Client::new(),
            "fixture-client",
            None,
            &endpoints,
            clock(),
        )
        .await
        .unwrap();
    browser.await.unwrap();
    assert!(credential.api_key.is_none());
    assert!(credential.refresh_token.is_none());
    assert_eq!(
        credential.expires_at,
        Some(clock().now().plus(Duration::from_secs(31536000)))
    );
    let ctx = context(
        ApiKind::Ahrefs,
        server.base_url("ahrefs"),
        vec![ApiLevel::Domain],
        &["backlinks"],
        Default::default(),
    );
    let time = clock();
    let source = LinkMetrics::with_clock(ApiKind::Ahrefs, time.clone());
    source.test_connection(&ctx, &credential).await.unwrap();
    assert_eq!(source.quotas()[0].limit, Some(10000));
    assert_eq!(source.quotas()[0].used, 200);
    assert!(
        linkmetrics::estimate_cost(ApiKind::Ahrefs, &urls(100), &ctx.cfg.api.ahrefs).unwrap() >= 50
    );
    source.fetch(&urls(1), &ctx).await.unwrap();
    assert_eq!(source.quotas()[0].used, 250);
    time.advance(Duration::from_secs(31536001));
    assert!(matches!(
        source.fetch(&urls(1), &ctx).await,
        Err(ApiError::Unauthorized(_))
    ));
    assert_eq!(source.status().state, ApiState::Paused);
    assert_eq!(mock.requests.all().len(), 1);
    server.stop().await;
}
// spec: INTEG-153
#[tokio::test]
async fn oauth_rejects_wrong_state_without_exchanging_code() {
    let endpoints = linkmetrics::oauth::Endpoints::default();
    let auth = linkmetrics::oauth::Authorization::begin("fixture", &endpoints)
        .await
        .unwrap();
    let redirect = auth
        .url
        .query_pairs()
        .find(|(k, _)| k == "redirect_uri")
        .unwrap()
        .1
        .into_owned();
    let browser = tokio::spawn(async move {
        reqwest::Client::new()
            .get(format!("{redirect}?code=no&state=wrong"))
            .send()
            .await
            .unwrap()
    });
    assert!(auth
        .finish(
            &reqwest::Client::new(),
            "fixture",
            None,
            &endpoints,
            clock()
        )
        .await
        .is_err());
    assert_eq!(browser.await.unwrap().status(), 400);
}
// spec: INTEG-154
#[tokio::test(start_paused = true)]
async fn moz_429_retries_obey_both_retry_after_and_free_rate() {
    let io_guard = tokio::spawn(async {
        loop {
            tokio::task::yield_now().await;
        }
    });
    let mock = Arc::new(MozMock::default());
    mock.throttle_once
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let server = MockServer::start(vec![mock.clone()]).await.unwrap();
    let ctx = context(
        ApiKind::Moz,
        server.base_url("moz"),
        vec![ApiLevel::Url],
        &["domain_authority"],
        Default::default(),
    );
    let task = tokio::spawn(async move {
        LinkMetrics::with_clock(ApiKind::Moz, clock())
            .fetch(&urls(1), &ctx)
            .await
    });
    while mock.requests.all().is_empty() {
        tokio::task::yield_now().await;
    }
    // Let the transport read the 429 and install its cooldown before advancing time.
    for _ in 0..100 {
        tokio::task::yield_now().await;
    }
    tokio::time::advance(Duration::from_secs(10)).await;
    assert_eq!(
        task.await.unwrap().unwrap()[0].v_text.as_deref(),
        Some("65")
    );
    let calls = mock.requests.all();
    assert_eq!(calls.len(), 2);
    assert!(calls[1].at.duration_since(calls[0].at) >= Duration::from_secs(10));
    io_guard.abort();
    server.stop().await;
}

// spec: INTEG-157
#[test]
fn country_and_volume_are_settable_from_the_default_cli_profile() {
    let mut cfg = CrawlConfig::default();
    cfg.apply_set("api.ahrefs.options.country=de").unwrap();
    cfg.apply_set("api.ahrefs.options.average_volume=true")
        .unwrap();
    assert_eq!(cfg.api.ahrefs.options.country.as_deref(), Some("de"));
    assert!(cfg.api.ahrefs.options.average_volume);
    let loaded = CrawlConfig::load_str(&cfg.to_osconfig_string().unwrap()).unwrap();
    assert_eq!(loaded.config.api.ahrefs, cfg.api.ahrefs);
}

// spec: INTEG-153
#[tokio::test]
async fn later_bundle_failure_preserves_first_bundle_outcomes() {
    let mock = Arc::new(AhrefsMock::failing_from_batch(2));
    let server = MockServer::start(vec![mock.clone()]).await.unwrap();
    let mut ctx = context(
        ApiKind::Ahrefs,
        server.base_url("ahrefs"),
        vec![ApiLevel::Url],
        &["backlinks"],
        Default::default(),
    );
    ctx.cfg.api.ahrefs.limits.retries = 0;
    let source = LinkMetrics::with_clock(ApiKind::Ahrefs, clock());
    let results = source.fetch_outcomes(&urls(101), &ctx).await;
    assert_eq!(results.iter().filter(|(_, r)| r.is_ok()).count(), 100);
    assert_eq!(results.iter().filter(|(_, r)| r.is_err()).count(), 1);
    assert_eq!(mock.requests.all().len(), 2);
    server.stop().await;
}

// spec: INTEG-152
// spec: INTEG-153
// spec: INTEG-154
#[tokio::test]
async fn missing_rows_and_native_site_errors_are_url_failures() {
    let addresses = urls(3);
    let majestic = Arc::new(MajesticMock {
        missing_targets: vec![addresses[1].url.clone()],
        error_targets: vec![addresses[2].url.clone()],
        ..Default::default()
    });
    let moz = Arc::new(MozMock {
        missing_targets: vec![addresses[1].url.clone()],
        error_targets: vec![addresses[2].url.clone()],
        ..Default::default()
    });
    let ahrefs = Arc::new(AhrefsMock::omitting_targets(vec![
        addresses[1].url.clone(),
        addresses[2].url.clone(),
    ]));
    let server = MockServer::start(vec![majestic, moz, ahrefs])
        .await
        .unwrap();
    for (kind, path, metric) in [
        (ApiKind::Majestic, "majestic", "TrustFlow"),
        (ApiKind::Moz, "moz", "domain_authority"),
        (ApiKind::Ahrefs, "ahrefs", "backlinks"),
    ] {
        let ctx = context(
            kind,
            server.base_url(path),
            vec![ApiLevel::Url],
            &[metric],
            LinkMetricsOptions {
                moz_paid: true,
                ..Default::default()
            },
        );
        let source = LinkMetrics::with_clock(kind, clock());
        let results = source.fetch_outcomes(&addresses, &ctx).await;
        assert!(results[0].1.is_ok());
        assert!(results[1].1.is_err());
        assert!(results[2].1.is_err());
    }
    server.stop().await;
}

// spec: INTEG-153
#[tokio::test]
async fn batch_adapter_rechecks_live_provider_quota_between_bundles() {
    let mock = Arc::new(AhrefsMock::with_unit_limit(250));
    let server = MockServer::start(vec![mock.clone()]).await.unwrap();
    let ctx = context(
        ApiKind::Ahrefs,
        server.base_url("ahrefs"),
        vec![ApiLevel::Url],
        &["backlinks"],
        Default::default(),
    );
    let source = Arc::new(LinkMetrics::with_clock(ApiKind::Ahrefs, clock()));
    // A new explicit job uses current quotas, not an old quota snapshot.
    source
        .test_connection(&ctx, &credential(ApiKind::Ahrefs))
        .await
        .unwrap();
    let adapter = linkmetrics::batch_source(source, urls(101));
    let mut succeeded = 0;
    for url in urls(101) {
        succeeded += usize::from(adapter.fetch(&[url], &ctx).await.is_ok());
    }
    assert_eq!(succeeded, 100);
    assert_eq!(mock.requests.all().len(), 1);
    server.stop().await;
}

// spec: INTEG-153
#[tokio::test]
async fn crawl_cap_never_prefetches_urls_not_admitted_by_the_app() {
    let mock = Arc::new(AhrefsMock::default());
    let server = MockServer::start(vec![mock.clone()]).await.unwrap();
    let mut ctx = context(
        ApiKind::Ahrefs,
        server.base_url("ahrefs"),
        vec![ApiLevel::Url],
        &["backlinks"],
        Default::default(),
    );
    ctx.cfg.api.ahrefs.limits.quota_limit = Some(1);
    let source = Arc::new(LinkMetrics::with_clock(ApiKind::Ahrefs, clock()));
    let addresses = urls(101);
    let adapter = linkmetrics::batch_source(source, addresses.clone());
    let values = adapter.fetch(&addresses[..1], &ctx).await.unwrap();
    assert_eq!(values.len(), 1);
    let requests = mock.requests.all();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].body["targets"].as_array().unwrap().len(), 1);
    assert_eq!(requests[0].body["targets"][0]["url"], addresses[0].url);
    server.stop().await;
}

// spec: INTEG-150
#[tokio::test]
async fn crawl_cap_still_deduplicates_shared_domain_targets() {
    let mock = Arc::new(AhrefsMock::default());
    let server = MockServer::start(vec![mock.clone()]).await.unwrap();
    let mut ctx = context(
        ApiKind::Ahrefs,
        server.base_url("ahrefs"),
        vec![ApiLevel::Domain],
        &["backlinks"],
        Default::default(),
    );
    ctx.cfg.api.ahrefs.limits.quota_limit = Some(2);
    let source = Arc::new(LinkMetrics::with_clock(ApiKind::Ahrefs, clock()));
    let addresses = urls(101);
    let adapter = linkmetrics::batch_source(source, addresses.clone());
    for url in &addresses[..2] {
        let values = adapter
            .fetch(std::slice::from_ref(url), &ctx)
            .await
            .unwrap();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].url_id, url.url_id);
        assert_eq!(values[0].url, url.url);
    }
    assert_eq!(mock.requests.all().len(), 1);
    server.stop().await;
}

// spec: INTEG-153
#[tokio::test]
async fn revoked_ahrefs_credentials_are_recognizable_without_a_json_body() {
    let mock = Arc::new(AhrefsMock::default());
    let server = MockServer::start(vec![mock]).await.unwrap();
    let ctx = context(
        ApiKind::Ahrefs,
        server.base_url("ahrefs"),
        vec![ApiLevel::Url],
        &["backlinks"],
        Default::default(),
    );
    let mut revoked = credential(ApiKind::Ahrefs);
    revoked.access_token = Some("bad".into());
    let source = LinkMetrics::with_clock(ApiKind::Ahrefs, clock());
    assert!(matches!(
        source.test_connection(&ctx, &revoked).await,
        Err(ApiError::Unauthorized(_))
    ));
    server.stop().await;
}

// spec: INTEG-153
#[tokio::test]
async fn crawl_cap_cost_estimate_covers_the_actual_per_request_minimum() {
    let mock = Arc::new(AhrefsMock::default());
    let server = MockServer::start(vec![mock.clone()]).await.unwrap();
    let mut ctx = context(
        ApiKind::Ahrefs,
        server.base_url("ahrefs"),
        vec![ApiLevel::Url],
        &["backlinks"],
        Default::default(),
    );
    ctx.cfg.api.ahrefs.limits.quota_limit = Some(101);
    let addresses = urls(101);
    let estimated =
        linkmetrics::estimate_cost(ApiKind::Ahrefs, &addresses, &ctx.cfg.api.ahrefs).unwrap();
    let source = Arc::new(LinkMetrics::with_clock(ApiKind::Ahrefs, clock()));
    source
        .test_connection(&ctx, &credential(ApiKind::Ahrefs))
        .await
        .unwrap();
    let before = source.quotas()[0].used;
    let adapter = linkmetrics::batch_source(source.clone(), addresses.clone());
    for url in addresses {
        assert!(adapter.fetch(&[url], &ctx).await.is_ok());
    }
    let actual = source.quotas()[0].used - before;
    assert_eq!(mock.requests.all().len(), 101);
    assert_eq!(actual, 5050);
    assert!(estimated >= actual, "estimate={estimated}, actual={actual}");
    server.stop().await;
}
