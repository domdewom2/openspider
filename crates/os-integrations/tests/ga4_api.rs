//! GA4-Quelle gegen den lokalen GA4-Mock (`os_testsite::mocks::ga4`, INTEG-019): Verbindung und Auswahl, Zeitraum,
//! Metriken, Seitendimension, Filter, Ergebnislimit, Paginierung, Kontingente, Kompatibilität und URL-Abgleich.
//! Zugangsdaten nur im `MemorySecretStore` (C13), keine echten Netzwerkziele.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use os_fetch::MemorySecretStore;
use os_integrations::credentials::{Credential, Credentials};
use os_integrations::ga4::catalog::{self, MetricTree};
use os_integrations::ga4::report::{OTHER_ROW_HINT, THRESHOLD_HINT};
use os_integrations::ga4::settings::{
    self, extra_preset, resolve_dates, DimensionFilter, ExtraPreset, FilterOp, Ga4Settings,
    MetricSel, PageDimension, SettingsError,
};
use os_integrations::ga4::{status_label, Ga4Source, NO_PROPERTIES};
use os_integrations::{ApiCtx, ApiError, ApiSource, ApiState, UrlRef};
use os_model::config::{DateRangeConfig, UrlMatchConfig};
use os_model::{ApiKind, Clock, CrawlConfig, DateRangePreset, ManualClock, Timestamp, UrlId};
use os_testsite::mocks::ga4::{
    Ga4Mock, MockRow, QuotaStatus, Synthetic, CUSTOM_METRIC, SITE_ROWS, STREAM,
};
use os_testsite::mocks::MockServer;
use serde_json::{json, Value};

const NOW: &str = "2026-10-19T10:30:00Z";

struct Harness {
    mock: Ga4Mock,
    server: MockServer,
    clock: Arc<ManualClock>,
    source: Arc<Ga4Source>,
    store: Arc<MemorySecretStore>,
}

impl Harness {
    async fn new() -> Harness {
        let mock = Ga4Mock::default();
        let server = MockServer::start(vec![Arc::new(mock.clone())])
            .await
            .unwrap();
        let clock = Arc::new(ManualClock::new(Timestamp::from_db(NOW).unwrap()));
        let store = Arc::new(MemorySecretStore::new());
        Credentials::new(store.clone())
            .put(ApiKind::Ga4, credential("good"), true)
            .unwrap();
        Harness {
            source: Arc::new(Ga4Source::new(clock.clone())),
            mock,
            server,
            clock,
            store,
        }
    }

    /// Kontext mit Property 2001 und Stream 3001, sonst Standard; `f` passt die Konfiguration an.
    fn ctx(&self, f: impl FnOnce(&mut CrawlConfig)) -> ApiCtx {
        let mut cfg = CrawlConfig::default();
        cfg.api.ga4.enabled = true;
        cfg.api.ga4.base_url = Some(self.server.base_url("ga4"));
        cfg.api.ga4.property = Some(STREAM.to_string());
        cfg.api.ga4.account = Some("fixture-google".into());
        f(&mut cfg);
        ApiCtx {
            cfg,
            secrets: self.store.clone(),
            network: Default::default(),
            allow_api_override: false,
            quota_ledger: None,
        }
    }

    async fn fetch_one(&self, ctx: &ApiCtx, url: &str) -> Result<Vec<(String, f64)>, ApiError> {
        let values = self.source.fetch(&[url_ref(1, url)], ctx).await?;
        Ok(values
            .into_iter()
            .map(|v| (v.metric, v.v_real.unwrap()))
            .collect())
    }

    fn reports(&self) -> Vec<Value> {
        self.mock.bodies("runReport")
    }
}

fn credential(token: &str) -> Credential {
    Credential {
        account_id: "fixture-google".into(),
        label: "Fixture Google".into(),
        api_key: None,
        access_token: Some(token.into()),
        refresh_token: Some("fixture-refresh".into()),
        expires_at: None,
        client_secret: None,
        scopes: vec!["https://www.googleapis.com/auth/analytics.readonly".into()],
    }
}

fn url_ref(id: i64, url: &str) -> UrlRef {
    UrlRef {
        url_id: Some(UrlId(id)),
        url: url.into(),
    }
}

fn value(values: &[(String, f64)], metric: &str) -> Option<f64> {
    values.iter().find(|(m, _)| m == metric).map(|(_, v)| *v)
}

fn metric_names(body: &Value) -> Vec<String> {
    body["metrics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["name"].as_str().unwrap().to_string())
        .collect()
}

fn num(v: &Value) -> u64 {
    v.as_str()
        .map(|s| s.parse().unwrap())
        .or_else(|| v.as_u64())
        .unwrap()
}

fn filter_json(filters: &[DimensionFilter]) -> String {
    serde_json::to_string(filters).unwrap()
}

// spec: INTEG-090
#[tokio::test]
async fn accounts_properties_and_streams_are_dependent_and_selection_is_persisted() {
    let h = Harness::new().await;
    let ctx = h.ctx(|_| {});
    let tree = h.source.accounts(&ctx).await.unwrap();
    assert_eq!(tree.accounts.len(), 2);
    let a = tree.properties("accounts/1001").unwrap();
    assert_eq!(
        a.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
        ["properties/2001", "properties/2002"]
    );
    let b = tree.properties("accounts/1002").unwrap();
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].name, "properties/2003");
    assert_eq!(tree.search("accounts/1001", "stag").len(), 1);
    let streams = h.source.streams(&ctx, "properties/2001").await.unwrap();
    assert_eq!(streams.len(), 2);
    assert_eq!(streams[0].host.as_deref(), Some("integ.test"));
    assert_eq!(streams[1].host.as_deref(), Some("blog.integ.test"));

    // Auswahl im Profil: Property- und Stream-Kennung; bei erneuter Verbindung vorausgewählt.
    let settings = Ga4Settings {
        property: "2001".into(),
        stream: Some("3001".into()),
        ..Ga4Settings::default()
    };
    let mut cfg = CrawlConfig::default();
    settings.to_config(&mut cfg.api.ga4);
    assert_eq!(cfg.api.ga4.property.as_deref(), Some(STREAM));
    let again = Ga4Settings::from_config(&cfg.api.ga4).unwrap();
    assert_eq!(
        (again.property.as_str(), again.stream.as_deref()),
        ("2001", Some("3001"))
    );
    assert_eq!(
        tree.account_of(cfg.api.ga4.property.as_deref().unwrap())
            .unwrap()
            .name,
        "accounts/1001"
    );
    assert_eq!(
        status_label("Integ Account A", "Integ Shop", Some("Integ Web")),
        "Verbunden (Integ Account A / Integ Shop / Integ Web)"
    );
    assert_eq!(
        status_label("Integ Account A", "Integ Shop", None),
        "Verbunden (Integ Account A / Integ Shop / Alle Datenstreams)"
    );
    h.server.stop().await;
}

// spec: INTEG-090
#[tokio::test]
async fn account_without_property_shows_hint_and_connection_test_names_the_account() {
    let h = Harness::new().await;
    let ctx = h.ctx(|_| {});
    h.mock.set_scenario("no-properties");
    let tree = h.source.accounts(&ctx).await.unwrap();
    assert_eq!(tree.properties("accounts/1003"), Err(NO_PROPERTIES));
    assert_eq!(NO_PROPERTIES, "Keine GA4-Properties gefunden");
    h.mock.set_scenario("ok");
    h.mock.clear_requests();
    // Genau ein Prüfabruf; Kennung und Label stammen aus der Kontoliste (I01-Erweiterungspunkt).
    let checked = h
        .source
        .test_connection(&ctx, &credential("good"))
        .await
        .unwrap();
    assert_eq!(h.mock.requests().len(), 1);
    assert_eq!(checked.account_id, "ga4:accounts/1001");
    assert_eq!(checked.label, "Integ Account A (+1)");
    let denied = h.source.test_connection(&ctx, &credential("bad")).await;
    assert!(matches!(denied, Err(ApiError::Unauthorized(m)) if m.starts_with("HTTP 401")));
    h.server.stop().await;
}

// spec: INTEG-090
#[tokio::test]
async fn selected_stream_adds_a_hostname_filter_and_foreign_hosts_are_not_assigned() {
    let h = Harness::new().await;
    let ctx = h.ctx(|_| {});
    let values = h.fetch_one(&ctx, "https://integ.test/five").await.unwrap();
    assert_eq!(value(&values, "sessions"), Some(5.0));
    let body = &h.reports()[0];
    let filter = &body["dimensionFilter"];
    assert_eq!(filter["filter"]["fieldName"], "hostName");
    assert_eq!(filter["filter"]["stringFilter"]["value"], "integ.test");
    assert!(h
        .fetch_one(&ctx, "https://fremd.test/x")
        .await
        .unwrap()
        .is_empty());
    // Ohne Stream („Alle Datenstreams“) gibt es keinen Hostfilter.
    let all = h.ctx(|c| c.api.ga4.property = Some("properties/2001".into()));
    h.mock.clear_requests();
    h.fetch_one(&all, "https://integ.test/five").await.unwrap();
    assert!(h.reports()[0].get("dimensionFilter").is_none());
    h.server.stop().await;
}

// spec: INTEG-091
#[tokio::test]
async fn default_range_is_last_30_days_until_yesterday_and_custom_ranges_are_checked() {
    let h = Harness::new().await;
    let ctx = h.ctx(|_| {});
    h.fetch_one(&ctx, "https://integ.test/").await.unwrap();
    assert_eq!(
        h.reports()[0]["dateRanges"],
        json!([{"startDate": "30daysAgo", "endDate": "yesterday"}])
    );
    let today = h.clock.now().0.date();
    let default = resolve_dates(&DateRangeConfig::default(), today).unwrap();
    assert_eq!(default.label(), "2026-09-19 – 2026-10-18");
    let custom = |from: &str, to: &str| DateRangeConfig {
        preset: DateRangePreset::Custom,
        from: Some(from.into()),
        to: Some(to.into()),
    };
    assert_eq!(
        resolve_dates(&custom("2026-10-10", "2026-10-01"), today),
        Err(SettingsError::EndBeforeStart)
    );
    let future = resolve_dates(&custom("2026-10-01", "2026-12-31"), today).unwrap();
    assert_eq!(
        (future.start.as_str(), future.end.as_str()),
        ("2026-10-01", "today")
    );
    assert_eq!(future.to, today);
    // Keine clientseitige Untergrenze für das Startdatum.
    assert!(resolve_dates(&custom("2005-01-01", "2005-01-31"), today).is_ok());
    let fourteen = resolve_dates(&extra_preset(ExtraPreset::Last14Days, today), today).unwrap();
    assert_eq!(fourteen.label(), "2026-10-05 – 2026-10-18");
    let year = resolve_dates(&extra_preset(ExtraPreset::Last12Months, today), today).unwrap();
    assert_eq!(year.start, "2025-10-19");
    // Ein Enddatum vor dem Start wird vor dem Senden abgelehnt.
    h.mock.clear_requests();
    let bad = h.ctx(|c| {
        c.api.ga4.date_range = custom("2026-10-10", "2026-10-01");
        c.api.ga4.metrics = vec!["sessions".into()];
    });
    assert!(h.fetch_one(&bad, "https://integ.test/").await.is_err());
    assert!(h.reports().is_empty());
    h.server.stop().await;
}

// spec: INTEG-091
#[tokio::test]
async fn rejected_date_range_shows_the_api_message_and_sets_no_values() {
    let h = Harness::new().await;
    h.mock.set_scenario("400-invalid-date-range");
    let ctx = h.ctx(|c| {
        c.api.ga4.date_range = DateRangeConfig {
            preset: DateRangePreset::Custom,
            from: Some("2001-01-01".into()),
            to: Some("2001-12-31".into()),
        };
    });
    let err = h
        .source
        .fetch(&[url_ref(1, "https://integ.test/")], &ctx)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, ApiError::Provider { status: 400, message } if message.contains("Invalid date range"))
    );
    let status = h.source.status();
    assert_eq!(status.state, ApiState::Failed);
    assert!(status.detail.unwrap().contains("Invalid date range"));
    h.server.stop().await;
}

// spec: INTEG-092
#[tokio::test]
async fn standard_metrics_columns_and_api_names() {
    let h = Harness::new().await;
    let ctx = h.ctx(|_| {});
    let settings = Ga4Settings::from_config(&ctx.cfg.api.ga4).unwrap();
    assert_eq!(
        settings
            .metrics
            .iter()
            .map(MetricSel::column_name)
            .collect::<Vec<_>>(),
        [
            "GA4 Sessions",
            "GA4 Engaged Sessions",
            "GA4 Engagement Rate",
            "GA4 Views",
            "GA4 Conversions",
            "GA4 Event Count",
            "GA4 Total Revenue"
        ]
    );
    let values = h.fetch_one(&ctx, "https://integ.test/").await.unwrap();
    let bodies = h.reports();
    assert_eq!(bodies.len(), 1);
    assert_eq!(
        metric_names(&bodies[0]),
        [
            "sessions",
            "engagedSessions",
            "engagementRate",
            "screenPageViews",
            "keyEvents",
            "eventCount",
            "totalRevenue",
            "bounceRate"
        ]
    );
    // Quotienten werden als Dezimalzahl gespeichert (Export-Standard), Währung in der Property-Währung.
    assert_eq!(value(&values, "engagementRate"), Some(0.6));
    assert_eq!(value(&values, "totalRevenue"), Some(180.0));
    let report = h.source.report(&ctx).await.unwrap();
    assert_eq!(report.currency.as_deref(), Some("EUR"));
    assert!(catalog::metrics().len() >= 65);
    assert_eq!(
        catalog::metric("engagementRate").unwrap().kind,
        catalog::MetricKind::Percent
    );
    assert_eq!(
        catalog::metric("totalRevenue").unwrap().kind,
        catalog::MetricKind::Currency
    );
    h.server.stop().await;
}

// spec: INTEG-092
#[tokio::test]
async fn fifteen_metrics_need_two_requests_and_merge_without_inventing_values() {
    let h = Harness::new().await;
    let fifteen = [
        "sessions",
        "engagedSessions",
        "engagementRate",
        "bounceRate",
        "screenPageViews",
        "keyEvents",
        "eventCount",
        "totalRevenue",
        "activeUsers",
        "newUsers",
        "totalUsers",
        "averageSessionDuration",
        "itemRevenue",
        "transactions",
        "addToCarts",
    ];
    h.mock.configure(|c| {
        let mut only_second = MockRow::page(
            "https://integ.test/only-second",
            [
                ("itemRevenue".to_string(), 7.0),
                ("transactions".to_string(), 2.0),
            ]
            .into(),
        );
        only_second.metrics.insert("addToCarts".to_string(), 3.0);
        c.rows.push(only_second);
    });
    let ctx = h.ctx(|c| c.api.ga4.metrics = fifteen.iter().map(|m| m.to_string()).collect());
    let report = h.source.report(&ctx).await.unwrap();
    let bodies = h.reports();
    assert_eq!(
        bodies
            .iter()
            .map(|b| metric_names(b).len())
            .collect::<Vec<_>>(),
        [10, 5]
    );
    assert_eq!(report.requests, 2);
    let first = h.fetch_one(&ctx, "https://integ.test/five").await.unwrap();
    // /five kommt nur in Antwort 1 vor (in Antwort 2 sind alle Metriken 0): nur deren zehn Werte, der Rest bleibt leer.
    assert_eq!(first.len(), 10, "{first:?}");
    assert_eq!(value(&first, "itemRevenue"), None);
    let home = h.fetch_one(&ctx, "https://integ.test/").await.unwrap();
    assert_eq!(value(&home, "sessions"), Some(120.0));
    let second = h
        .fetch_one(&ctx, "https://integ.test/only-second")
        .await
        .unwrap();
    let mut metrics: Vec<&str> = second.iter().map(|(m, _)| m.as_str()).collect();
    metrics.sort_unstable();
    // Nur Antwort 2 (deren fünf Metriken; GA4 liefert dort 0 für nicht belegte Metriken).
    assert_eq!(
        metrics,
        [
            "addToCarts",
            "averageSessionDuration",
            "itemRevenue",
            "totalUsers",
            "transactions"
        ]
    );
    assert_eq!(value(&second, "itemRevenue"), Some(7.0));
    assert_eq!(value(&second, "sessions"), None, "leer, nicht 0");
    h.server.stop().await;
}

// spec: INTEG-092
#[tokio::test]
async fn unknown_metric_is_reported_removed_and_the_rest_is_loaded() {
    let h = Harness::new().await;
    let ctx = h.ctx(|c| c.api.ga4.metrics = vec!["sessions".into(), "conversions".into()]);
    let values = h.fetch_one(&ctx, "https://integ.test/five").await.unwrap();
    assert_eq!(value(&values, "sessions"), Some(5.0));
    let bodies = h.reports();
    assert_eq!(bodies.len(), 2);
    assert!(metric_names(&bodies[0]).contains(&"conversions".to_string()));
    assert!(!metric_names(&bodies[1]).contains(&"conversions".to_string()));
    assert!(h
        .source
        .notices()
        .iter()
        .any(|n| n.contains("conversions") && n.contains("entfernt")));
    h.server.stop().await;
}

// spec: INTEG-092
#[tokio::test]
async fn metric_tree_comes_from_metadata_with_a_custom_category() {
    let h = Harness::new().await;
    let ctx = h.ctx(|_| {});
    let tree = h.source.metric_tree(&ctx, "properties/2001").await;
    let custom = tree.category(catalog::CUSTOM_CATEGORY);
    assert_eq!(custom.len(), 1);
    assert_eq!(custom[0].api, CUSTOM_METRIC.0);
    assert_eq!(custom[0].name, "Fixture Score");
    assert!(tree.all().count() >= 66);
    assert_eq!(tree.search("engaged")[0].api, "engagedSessions");
    for category in ["User", "Session", "Engagement", "E-commerce", "Advertising"] {
        assert!(!tree.category(category).is_empty(), "{category}");
    }
    // Ohne erreichbaren Endpunkt: statischer Katalog.
    let offline = h.ctx(|c| c.api.ga4.base_url = Some("http://127.0.0.1:9/ga4".into()));
    assert_eq!(
        h.source.metric_tree(&offline, "properties/2001").await,
        MetricTree::from_catalog()
    );
    h.server.stop().await;
}

// spec: INTEG-093
#[tokio::test]
async fn landing_page_dimension_uses_stream_host_and_skips_not_set() {
    let h = Harness::new().await;
    let ctx = h.ctx(|c| {
        c.api.ga4.metrics = vec!["sessions@landing".into(), "screenPageViews".into()];
    });
    let settings = Ga4Settings::from_config(&ctx.cfg.api.ga4).unwrap();
    assert_eq!(
        settings
            .metrics
            .iter()
            .map(MetricSel::column_name)
            .collect::<Vec<_>>(),
        ["GA4 Sessions (Landing Page)", "GA4 Views"]
    );
    let values = h.fetch_one(&ctx, "http://integ.test/five").await.unwrap();
    assert_eq!(value(&values, "sessions@landing"), Some(5.0));
    assert_eq!(value(&values, "screenPageViews"), Some(10.0));
    let dims: BTreeSet<String> = h
        .reports()
        .iter()
        .map(|b| b["dimensions"][0]["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        dims,
        BTreeSet::from(["landingPagePlusQueryString".into(), "pageLocation".into()])
    );
    let report = h.source.report(&ctx).await.unwrap();
    for rows in report.rows.values() {
        assert!(rows.keys().all(|u| !u.contains("(not set)")));
        assert!(rows.keys().all(|u| u.contains("://integ.test/")));
    }
    h.server.stop().await;
}

// spec: INTEG-093
#[tokio::test]
async fn landing_pages_without_stream_take_the_host_from_ga4_and_foreign_hosts_stay_unassigned() {
    let h = Harness::new().await;
    let ctx = h.ctx(|c| {
        c.api.ga4.property = Some("properties/2001".into());
        c.api.ga4.page_dimension = Some("landingPagePlusQueryString".into());
        c.api.ga4.metrics = vec!["sessions".into()];
    });
    let values = h.fetch_one(&ctx, "https://integ.test/three").await.unwrap();
    assert_eq!(value(&values, "sessions@landing"), Some(3.0));
    assert_eq!(
        h.reports()[0]["dimensions"],
        json!([{"name": "hostName"}, {"name": "landingPagePlusQueryString"}])
    );
    assert!(h
        .fetch_one(&ctx, "https://integ.test/x")
        .await
        .unwrap()
        .is_empty());
    let report = h.source.report(&ctx).await.unwrap();
    let assignment = report.assign(
        &[url_ref(1, "https://integ.test/three")],
        &UrlMatchConfig::default(),
        &BTreeSet::from(["integ.test".to_string()]),
        h.clock.now(),
    );
    assert!(assignment
        .orphans
        .contains(&"https://integ.test/orphan".to_string()));
    assert!(!assignment.orphans.iter().any(|u| u.contains("fremd.test")));
    assert!(!assignment.orphans.iter().any(|u| u.ends_with("/three")));
    h.server.stop().await;
}

// spec: INTEG-094
#[tokio::test]
async fn dimension_filters_become_filter_expressions_and_bad_regex_is_rejected() {
    let h = Harness::new().await;
    let organic = DimensionFilter {
        dimension: "sessionDefaultChannelGroup".into(),
        op: FilterOp::Exact,
        value: "Organic Search".into(),
        negate: false,
    };
    let one = h.ctx(|c| {
        c.api.ga4.property = Some("properties/2001".into());
        c.api.ga4.filter = Some(filter_json(std::slice::from_ref(&organic)));
    });
    h.fetch_one(&one, "https://integ.test/").await.unwrap();
    assert_eq!(
        h.reports()[0]["dimensionFilter"],
        json!({"filter": {"fieldName": "sessionDefaultChannelGroup",
            "stringFilter": {"matchType": "EXACT", "value": "Organic Search", "caseSensitive": false}}})
    );
    assert!(h
        .source
        .notices()
        .contains(&"Gefiltert: sessionDefaultChannelGroup = Organic Search".to_string()));
    let google = DimensionFilter {
        dimension: "firstUserSource".into(),
        op: FilterOp::Contains,
        value: "google".into(),
        negate: true,
    };
    let two = h.ctx(|c| {
        c.api.ga4.property = Some("properties/2001".into());
        c.api.ga4.filter = Some(filter_json(&[organic.clone(), google]));
    });
    h.mock.clear_requests();
    h.fetch_one(&two, "https://integ.test/").await.unwrap();
    let group = &h.reports()[0]["dimensionFilter"]["andGroup"]["expressions"];
    assert_eq!(group.as_array().unwrap().len(), 2);
    assert_eq!(
        group[1]["notExpression"]["filter"]["fieldName"],
        "firstUserSource"
    );
    // Ungültiger Regex wird vor dem Senden abgelehnt (auch RE2-fremde Konstrukte).
    for pattern in ["(unclosed", "^(?=x)"] {
        let bad = h.ctx(|c| {
            c.api.ga4.filter = Some(filter_json(&[DimensionFilter {
                dimension: "pagePath".into(),
                op: FilterOp::Regex,
                value: pattern.into(),
                negate: false,
            }]))
        });
        assert!(matches!(
            Ga4Settings::from_config(&bad.cfg.api.ga4),
            Err(SettingsError::BadRegex { index: 0, .. })
        ));
        h.mock.clear_requests();
        h.source.invalidate().await;
        assert!(h.fetch_one(&bad, "https://integ.test/").await.is_err());
        assert!(h.reports().is_empty(), "{pattern}");
    }
    assert!(settings::check_regex("^/blog/.*").is_ok());
    h.server.stop().await;
}

// spec: INTEG-095
#[tokio::test]
async fn result_limit_loads_top_sessions_only() {
    let h = Harness::new().await;
    h.mock.configure(|c| {
        c.synthetic = Some(Synthetic {
            rows: 2_500,
            page_cap: None,
        })
    });
    let ctx = h.ctx(|c| {
        c.api.ga4.metrics = vec!["sessions".into()];
        c.api.ga4.filter = Some(r#"{"max_results": 1000}"#.into());
    });
    let report = h.source.report(&ctx).await.unwrap();
    let bodies = h.reports();
    assert_eq!(bodies.len(), 1);
    assert_eq!(num(&bodies[0]["limit"]), 1000);
    assert_eq!(
        bodies[0]["orderBys"],
        json!([{"metric": {"metricName": "sessions"}, "desc": true}])
    );
    assert_eq!(report.rows[&PageDimension::PageLocation].len(), 1000);
    assert_eq!(
        value(
            &h.fetch_one(&ctx, "https://integ.test/p/0").await.unwrap(),
            "sessions"
        ),
        Some(2500.0)
    );
    assert!(h
        .fetch_one(&ctx, "https://integ.test/p/1500")
        .await
        .unwrap()
        .is_empty());
    // Standard und Bereich.
    let defaults = Ga4Settings::default();
    assert_eq!(
        (defaults.max_results, defaults.page_size),
        (100_000, 10_000)
    );
    let clamp = h.ctx(|c| c.api.ga4.filter = Some(r#"{"max_results": 99000000}"#.into()));
    assert_eq!(
        Ga4Settings::from_config(&clamp.cfg.api.ga4)
            .unwrap()
            .max_results,
        10_000_000
    );
    h.server.stop().await;
}

// spec: INTEG-096
#[tokio::test]
async fn pagination_uses_limit_and_offset_until_row_count() {
    let h = Harness::new().await;
    h.mock.configure(|c| {
        c.synthetic = Some(Synthetic {
            rows: 25_000,
            page_cap: None,
        })
    });
    let ctx = h.ctx(|c| c.api.ga4.metrics = vec!["sessions".into()]);
    let report = h.source.report(&ctx).await.unwrap();
    let mut pages: Vec<(u64, u64)> = h
        .reports()
        .iter()
        .map(|b| (num(&b["offset"]), num(&b["limit"])))
        .collect();
    pages.sort_unstable();
    assert_eq!(pages, [(0, 10_000), (10_000, 10_000), (20_000, 10_000)]);
    assert_eq!(report.rows[&PageDimension::PageLocation].len(), 25_000);
    h.server.stop().await;
}

// spec: INTEG-096
#[tokio::test]
async fn page_size_is_capped_at_250000_and_result_limit_shortens_the_last_page() {
    let h = Harness::new().await;
    h.mock.configure(|c| {
        c.synthetic = Some(Synthetic {
            rows: 300_000,
            page_cap: Some(2),
        })
    });
    let big = h.ctx(|c| {
        c.api.ga4.metrics = vec!["sessions".into()];
        c.api.ga4.filter = Some(r#"{"max_results": 10000000, "page_size": 400000}"#.into());
    });
    h.source.report(&big).await.unwrap();
    let mut pages: Vec<(u64, u64)> = h
        .reports()
        .iter()
        .map(|b| (num(&b["offset"]), num(&b["limit"])))
        .collect();
    pages.sort_unstable();
    assert_eq!(pages, [(0, 250_000), (250_000, 250_000)]);

    h.mock.clear_requests();
    h.mock.configure(|c| {
        c.synthetic = Some(Synthetic {
            rows: 25_000,
            page_cap: None,
        })
    });
    let limited = h.ctx(|c| {
        c.api.ga4.metrics = vec!["sessions".into()];
        c.api.ga4.filter = Some(r#"{"max_results": 15000}"#.into());
    });
    let report = h.source.report(&limited).await.unwrap();
    let mut pages: Vec<(u64, u64)> = h
        .reports()
        .iter()
        .map(|b| (num(&b["offset"]), num(&b["limit"])))
        .collect();
    pages.sort_unstable();
    assert_eq!(pages, [(0, 10_000), (10_000, 5_000)]);
    assert_eq!(report.rows[&PageDimension::PageLocation].len(), 15_000);
    h.server.stop().await;
}

// spec: INTEG-096
#[tokio::test]
async fn other_row_and_thresholding_flags_produce_hints() {
    let h = Harness::new().await;
    h.mock.configure(|c| {
        c.data_loss_from_other_row = true;
        c.thresholding = true;
    });
    let ctx = h.ctx(|_| {});
    h.fetch_one(&ctx, "https://integ.test/").await.unwrap();
    let notices = h.source.notices();
    assert!(notices.contains(&OTHER_ROW_HINT.to_string()));
    assert_eq!(
        OTHER_ROW_HINT,
        "GA4 hat Zeilen zu (other) zusammengefasst – Daten unvollständig"
    );
    assert!(notices.contains(&THRESHOLD_HINT.to_string()));
    h.server.stop().await;
}

// spec: INTEG-099
#[tokio::test]
async fn low_hourly_quota_holds_requests_until_the_next_full_hour() {
    let h = Harness::new().await;
    h.mock.configure(|c| {
        c.synthetic = Some(Synthetic {
            rows: 20_000,
            page_cap: Some(1),
        });
        c.tokens_per_hour = QuotaStatus {
            consumed: 38_000,
            remaining: 2_010,
        };
    });
    let ctx = h.ctx(|c| c.api.ga4.metrics = vec!["sessions".into()]);
    let source = h.source.clone();
    let task = tokio::spawn(async move { source.report(&ctx).await.map(|r| r.requests) });
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(
        h.reports().len(),
        1,
        "zweite Seite wartet auf den Stundenwechsel"
    );
    assert_eq!(
        h.source
            .quotas()
            .iter()
            .find(|q| q.dimension == "tokensPerHour")
            .unwrap()
            .resets_at,
        Some(Timestamp::from_db("2026-10-19T11:00:00Z").unwrap())
    );
    h.clock.advance(Duration::from_secs(30 * 60));
    let requests = tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(requests, 2);
    assert!(h
        .reports()
        .iter()
        .all(|b| b["returnPropertyQuota"] == json!(true)));
    let quotas = h.source.quotas();
    let day = quotas
        .iter()
        .find(|q| q.dimension == "tokensPerDay")
        .unwrap();
    assert_eq!((day.used, day.limit), (20, Some(200_000)));
    h.server.stop().await;
}

// spec: INTEG-099
#[tokio::test]
async fn at_most_ten_concurrent_requests_per_property() {
    let h = Harness::new().await;
    h.mock.configure(|c| {
        c.synthetic = Some(Synthetic {
            rows: 30_000,
            page_cap: Some(1),
        });
        c.delay = Duration::from_millis(60);
    });
    let ctx = h.ctx(|c| {
        c.api.ga4.metrics = vec!["sessions".into()];
        c.api.ga4.limits.concurrency = 50;
        c.api.ga4.filter = Some(r#"{"page_size": 1000}"#.into());
    });
    h.source.report(&ctx).await.unwrap();
    assert_eq!(h.reports().len(), 30);
    let max = h.mock.max_inflight();
    assert!((2..=10).contains(&max), "gleichzeitig offen: {max}");
    h.server.stop().await;
}

// spec: INTEG-099
#[tokio::test]
async fn resource_exhausted_pauses_the_integration() {
    let h = Harness::new().await;
    h.mock.set_scenario("resource-exhausted");
    let ctx = h.ctx(|_| {});
    let err = h
        .source
        .fetch(&[url_ref(1, "https://integ.test/")], &ctx)
        .await
        .unwrap_err();
    assert!(matches!(err, ApiError::QuotaExceeded(_)));
    let status = h.source.status();
    assert_eq!(status.state, ApiState::Paused);
    assert!(status.detail.unwrap().contains("pausiert"));
    // Weitere URLs desselben Laufs fragen nicht erneut an.
    h.source
        .fetch(&[url_ref(2, "https://integ.test/five")], &ctx)
        .await
        .unwrap_err();
    assert_eq!(h.reports().len(), 1);
    h.server.stop().await;
}

fn compat_settings(metrics: &[&str], filters: Vec<DimensionFilter>) -> Ga4Settings {
    Ga4Settings {
        property: "2001".into(),
        metrics: metrics
            .iter()
            .map(|m| MetricSel::new(m, PageDimension::PageLocation))
            .collect(),
        filters,
        ..Ga4Settings::default()
    }
}

// spec: INTEG-101
#[tokio::test]
async fn incompatible_metrics_are_greyed_out_and_marked_without_removal() {
    let h = Harness::new().await;
    let ctx = h.ctx(|_| {});
    let plain = compat_settings(&["sessions", "itemRevenue"], vec![]);
    let result = h.source.check_compatibility(&ctx, &plain).await;
    let bodies = h.mock.bodies("checkCompatibility");
    assert_eq!(bodies.len(), 1);
    assert_eq!(metric_names(&bodies[0]), ["sessions", "itemRevenue"]);
    assert_eq!(bodies[0]["dimensions"], json!([{"name": "pageLocation"}]));
    assert!(!result.is_selectable("advertiserAdCost"));
    assert_eq!(
        result.tooltip("advertiserAdCost").as_deref(),
        Some("Nicht kombinierbar mit: pageLocation")
    );
    assert!(result.is_selectable("sessions") && result.is_selectable("itemRevenue"));
    assert!(result.ok_enabled(&plain));
    // Ein Filter auf firstUserSource macht die bereits gewählte Metrik itemRevenue inkompatibel: markiert, nicht entfernt.
    let filtered = compat_settings(
        &["sessions", "itemRevenue"],
        vec![DimensionFilter {
            dimension: "firstUserSource".into(),
            op: FilterOp::Exact,
            value: "google".into(),
            negate: false,
        }],
    );
    let result = h.source.check_compatibility(&ctx, &filtered).await;
    assert_eq!(result.conflicts(&filtered), ["itemRevenue"]);
    assert!(!result.ok_enabled(&filtered));
    assert_eq!(filtered.metrics.len(), 2);
    h.server.stop().await;
}

// spec: INTEG-101
#[tokio::test]
async fn no_report_request_contains_a_combination_reported_incompatible() {
    let h = Harness::new().await;
    let ctx = h.ctx(|c| {
        c.api.ga4.metrics = vec!["sessions".into(), "advertiserAdCost".into()];
    });
    let settings = Ga4Settings::from_config(&ctx.cfg.api.ga4).unwrap();
    let result = h.source.check_compatibility(&ctx, &settings).await;
    assert_eq!(result.conflicts(&settings), ["advertiserAdCost"]);
    let err = h.fetch_one(&ctx, "https://integ.test/").await.unwrap_err();
    assert!(matches!(err, ApiError::Provider { status: 400, .. }));
    assert!(h.reports().is_empty());
    h.server.stop().await;
}

// spec: INTEG-101
#[tokio::test]
async fn unreachable_check_keeps_selection_free_and_report_400_sets_no_values() {
    let h = Harness::new().await;
    h.mock.configure(|c| c.compatibility_down = true);
    let ctx = h.ctx(|c| {
        c.api.ga4.metrics = vec!["sessions".into(), "advertiserAdCost".into()];
    });
    let settings = Ga4Settings::from_config(&ctx.cfg.api.ga4).unwrap();
    let result = h.source.check_compatibility(&ctx, &settings).await;
    assert!(!result.reachable);
    assert!(result.is_selectable("advertiserAdCost") && result.ok_enabled(&settings));
    let err = h
        .source
        .fetch(&[url_ref(1, "https://integ.test/")], &ctx)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, ApiError::Provider { status: 400, message } if message.contains("advertiserAdCost"))
    );
    assert!(h
        .source
        .status()
        .detail
        .unwrap()
        .contains("advertiserAdCost"));
    h.server.stop().await;
}

// spec: INTEG-101
#[tokio::test]
async fn selection_changes_are_debounced_into_one_check() {
    let h = Harness::new().await;
    let ctx = h.ctx(|_| {});
    let mut checker = h.source.checker(ctx);
    checker.update(compat_settings(&["sessions"], vec![]));
    tokio::time::sleep(Duration::from_millis(100)).await;
    checker.update(compat_settings(&["sessions", "itemRevenue"], vec![]));
    tokio::time::sleep(Duration::from_millis(100)).await;
    let last_change = tokio::time::Instant::now();
    checker.update(compat_settings(
        &["sessions", "itemRevenue", "addToCarts"],
        vec![],
    ));
    let result = tokio::time::timeout(Duration::from_secs(5), checker.changed())
        .await
        .unwrap()
        .unwrap();
    assert!(result.reachable);
    let requests: Vec<_> = h
        .mock
        .requests()
        .into_iter()
        .filter(|r| r.path.ends_with(":checkCompatibility"))
        .collect();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        metric_names(&requests[0].body),
        ["sessions", "itemRevenue", "addToCarts"]
    );
    let delay = requests[0].at.duration_since(last_change);
    assert!(
        delay >= Duration::from_millis(450) && delay <= Duration::from_millis(800),
        "{delay:?}"
    );
    h.server.stop().await;
}

// spec: INTEG-014
#[tokio::test]
async fn ga4_rows_are_matched_by_protocol_slash_case_and_percent_encoding() {
    let h = Harness::new().await;
    let ctx = h.ctx(|c| c.api.ga4.metrics = vec!["sessions".into()]);
    let sessions = |values: Vec<(String, f64)>| value(&values, "sessions");
    // Protokoll wird immer abgeglichen.
    assert_eq!(
        sessions(h.fetch_one(&ctx, "https://integ.test/a/").await.unwrap()),
        Some(5.0)
    );
    // Varianten /b (10) und /b/ (30): die höchste, nicht die Summe.
    assert_eq!(
        sessions(h.fetch_one(&ctx, "https://integ.test/b/").await.unwrap()),
        Some(30.0)
    );
    assert_eq!(
        sessions(h.fetch_one(&ctx, "https://integ.test/c").await.unwrap()),
        Some(7.0)
    );
    assert_eq!(
        sessions(
            h.fetch_one(&ctx, "https://integ.test/produkt")
                .await
                .unwrap()
        ),
        Some(5.0)
    );
    // Query bleibt Teil der Identität, Prozentkodierung wird normalisiert.
    assert_eq!(
        sessions(h.fetch_one(&ctx, "https://integ.test/d?x=1").await.unwrap()),
        Some(2.0)
    );
    assert_eq!(
        sessions(h.fetch_one(&ctx, "https://integ.test/d").await.unwrap()),
        Some(1.0)
    );
    assert_eq!(
        sessions(h.fetch_one(&ctx, "https://integ.test/ü").await.unwrap()),
        Some(3.0)
    );
    // Ohne Slash- bzw. Groß-/Kleinschreibungs-Abgleich keine Zuordnung.
    let strict = h.ctx(|c| {
        c.api.ga4.metrics = vec!["sessions".into()];
        c.api.ga4.url_match = UrlMatchConfig {
            ignore_trailing_slash: false,
            ignore_case: false,
        };
    });
    assert!(h
        .fetch_one(&strict, "https://integ.test/c")
        .await
        .unwrap()
        .is_empty());
    assert!(h
        .fetch_one(&strict, "https://integ.test/produkt")
        .await
        .unwrap()
        .is_empty());
    h.server.stop().await;
}

// spec: INTEG-097
#[tokio::test]
async fn site_fixture_values_are_assigned_and_zero_is_kept() {
    let h = Harness::new().await;
    let ctx = h.ctx(|_| {});
    for row in SITE_ROWS {
        let values = h
            .fetch_one(&ctx, &format!("https://integ.test{}", row.path))
            .await
            .unwrap();
        assert_eq!(
            value(&values, "sessions"),
            Some(row.sessions),
            "{}",
            row.path
        );
        assert_eq!(
            value(&values, "bounceRate"),
            Some(row.bounce_rate),
            "{}",
            row.path
        );
    }
    assert!(h
        .fetch_one(&ctx, "https://integ.test/nodata")
        .await
        .unwrap()
        .is_empty());
    // Ein Report für alle URLs.
    assert_eq!(h.reports().len(), 1);
    h.server.stop().await;
}
