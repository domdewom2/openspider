//! Search Console – Properties, Zeitraum, Suchtyp, Dimensionsfilter, Paginierung, Aggregation, URL-Abgleich und
//! Read-Timeout gegen den lokalen Mock (`os_testsite::mocks::gsc`). Keine echten Netzwerkaufrufe; Verzögerungen laufen
//! mit pausierter tokio-Uhr (Testuhr).

use std::sync::Arc;
use std::time::Duration;

use os_fetch::{MemorySecretStore, SecretStoreMut};
use os_integrations::credentials::{Credential, Credentials};
use os_integrations::gsc::analytics::{self, Notice};
use os_integrations::gsc::client::{Failure, GscHttp};
use os_integrations::gsc::options::{self, DateSpan, GscOptions, OptionsError};
use os_integrations::gsc::{dates, sites, GscSource};
use os_integrations::{http, quota::QuotaLedger, ApiCtx, ApiSource, UrlRef};
use os_model::config::UrlMatchConfig;
use os_model::{ApiKind, Clock, CrawlConfig, ManualClock, SearchType, Timestamp, UrlId};
use os_testsite::mocks::gsc::GscMock;
use os_testsite::mocks::MockServer;
use serde_json::json;

/// Testuhr: 19.10.2026 12:00 UTC = 05:00 in Los Angeles (PDT).
fn now() -> Timestamp {
    Timestamp::from_db("2026-10-19T12:00:00Z").unwrap()
}

fn clock() -> Arc<dyn Clock> {
    Arc::new(ManualClock::new(now()))
}

fn options() -> GscOptions {
    GscOptions {
        site: Some("sc-domain:integ.test".into()),
        ..GscOptions::default()
    }
}

async fn start(mock: &GscMock) -> MockServer {
    MockServer::start(vec![Arc::new(mock.clone())])
        .await
        .unwrap()
}

/// Treibt die pausierte Testuhr in Schritten von 10 ms voran und lässt dazwischen alle Aufgaben (auch die
/// Netzwerk-Ein-/Ausgabe über Loopback) laufen. Ohne diesen Treiber springt die automatisch vorrückende Uhr beim
/// Warten auf einen Socket sofort zum nächsten Timer (z. B. dem Lese-Timeout) – das Ergebnis hinge vom Zufall ab.
async fn simulated<F: std::future::Future>(f: F) -> F::Output {
    let drive = async {
        loop {
            for _ in 0..8 {
                tokio::task::yield_now().await;
            }
            tokio::time::advance(Duration::from_millis(10)).await;
        }
    };
    tokio::select! {
        out = f => out,
        _ = drive => unreachable!(),
    }
}

fn client(server: &MockServer, options: &GscOptions) -> GscHttp {
    let base = server.base_url("gsc");
    GscHttp::with_client(
        http::client(&Default::default()).unwrap(),
        base.clone(),
        base,
        Some("good".into()),
        options,
        clock(),
    )
}

fn crawled(urls: &[&str]) -> Vec<UrlRef> {
    urls.iter()
        .enumerate()
        .map(|(i, u)| UrlRef {
            url_id: Some(UrlId(i as i64 + 1)),
            url: u.to_string(),
        })
        .collect()
}

/// Wert einer Metrik für eine URL aus den geschriebenen Werten.
fn value(values: &[os_model::ApiValueWrite], url: &str, metric: &str) -> Option<f64> {
    values
        .iter()
        .find(|v| v.url == url && v.metric == metric && v.url_id.is_some())
        .and_then(|v| v.v_real)
}

// spec: INTEG-110
#[tokio::test]
async fn properties_are_listed_labelled_greyed_and_checked_against_the_crawl_domain() {
    let mock = GscMock::new();
    let server = start(&mock).await;
    let http = client(&server, &options());
    let list = os_integrations::gsc::list_properties(&http).await.unwrap();
    assert_eq!(list.len(), 4);
    let domain = list
        .iter()
        .find(|p| p.site_url == "sc-domain:integ.test")
        .unwrap();
    let prefix = list
        .iter()
        .find(|p| p.site_url == "https://integ.test/")
        .unwrap();
    assert_eq!(domain.kind, sites::PropertyKind::Domain);
    assert_eq!(prefix.kind, sites::PropertyKind::UrlPrefix);
    assert_eq!(domain.label(), "integ.test (Domain)");
    assert_eq!(prefix.label(), "https://integ.test/ (URL-Präfix)");
    assert_ne!(domain.label(), prefix.label());
    // siteUnverifiedUser wird ausgegraut.
    let unverified = list
        .iter()
        .find(|p| p.permission == sites::UNVERIFIED)
        .unwrap();
    assert!(!unverified.selectable());
    assert!(list.iter().filter(|p| p.selectable()).count() == 3);
    // Suchfeld des Dropdowns.
    assert_eq!(sites::search(&list, "BLOG").len(), 1);
    // Crawl von http://other.test/ mit sc-domain:integ.test → Warnhinweis; passende Crawl-Domain ohne Hinweis.
    assert_eq!(
        sites::crawl_warning(domain, "http://other.test/"),
        Some("Property passt nicht zur Crawl-Domain")
    );
    assert_eq!(sites::crawl_warning(domain, "http://www.integ.test/"), None);
    assert_eq!(sites::crawl_warning(prefix, "https://integ.test/a"), None);
    assert!(sites::crawl_warning(prefix, "http://integ.test/").is_some());
    assert_eq!(
        sites::connected_label("Google-Konto", &domain.site_url),
        "Verbunden (Google-Konto / sc-domain:integ.test)"
    );
    server.stop().await;
}

// spec: INTEG-110
#[tokio::test]
async fn connecting_sends_exactly_one_site_list_request() {
    let mock = GscMock::new();
    let server = start(&mock).await;
    let mut cfg = CrawlConfig::default();
    cfg.api.gsc.base_url = Some(server.base_url("gsc"));
    let ctx = ApiCtx {
        cfg,
        secrets: Arc::new(MemorySecretStore::new()),
        network: Default::default(),
        allow_api_override: true,
    };
    let dir = tempfile::tempdir().unwrap();
    let source = GscSource::new(
        clock(),
        Arc::new(QuotaLedger::new(dir.path().join("q.json"))),
    );
    let credential = Credential {
        account_id: "a".into(),
        label: "Google-Konto".into(),
        api_key: None,
        access_token: Some("good".into()),
        refresh_token: None,
        expires_at: None,
        client_secret: None,
        scopes: vec![],
    };
    source.test_connection(&ctx, &credential).await.unwrap();
    assert_eq!(mock.requests().len(), 1);
    assert_eq!(mock.requests()[0].endpoint, "sites");
    let bad = Credential {
        access_token: Some("bad".into()),
        ..credential
    };
    assert!(source.test_connection(&ctx, &bad).await.is_err());
    assert_eq!(mock.requests().len(), 2);
    server.stop().await;
}

// spec: INTEG-111
#[tokio::test]
async fn default_request_covers_thirty_days_up_to_today_minus_two_in_los_angeles() {
    let mock = GscMock::new();
    let server = start(&mock).await;
    let opts = options();
    analytics::fetch_report(&client(&server, &opts), &opts, now()).await;
    let body = &mock.requests_to("query")[0].body;
    assert_eq!(body["endDate"], "2026-10-17");
    assert_eq!(body["startDate"], "2026-09-18");
    assert_eq!(body["dataState"], "final");
    // Kurz nach Mitternacht UTC ist in Los Angeles noch der Vortag.
    let early = Timestamp::from_db("2026-10-19T05:00:00Z").unwrap();
    let range = dates::resolve(DateSpan::default(), early);
    assert_eq!(range.end.to_string(), "2026-10-16");
    assert_eq!(range.start.to_string(), "2026-09-17");
    // Vorgaben.
    assert_eq!(DateSpan::PRESETS.len(), 8);
    let r7 = dates::resolve(DateSpan::LastDays(7), now());
    assert_eq!(
        (r7.start.to_string(), r7.end.to_string()),
        ("2026-10-11".into(), "2026-10-17".into())
    );
    let r16 = dates::resolve(DateSpan::LastMonths(16), now());
    assert_eq!(r16.start.to_string(), "2025-06-19");
    assert!(!r16.clamped);
    server.stop().await;
}

// spec: INTEG-111
#[tokio::test]
async fn too_early_start_is_clamped_with_notice_and_fresh_data_sends_data_state_all() {
    let mock = GscMock::new();
    let server = start(&mock).await;
    let mut cfg = os_model::config::GscConfig {
        site: Some("sc-domain:integ.test".into()),
        ..Default::default()
    };
    cfg.date_range.preset = os_model::DateRangePreset::Custom;
    cfg.date_range.from = Some("2024-01-01".into());
    cfg.date_range.to = Some("2026-10-01".into());
    let mut opts = GscOptions::from_config(&cfg).unwrap();
    opts.fresh_data = true;
    let report = analytics::fetch_report(&client(&server, &opts), &opts, now()).await;
    let body = &mock.requests_to("query")[0].body;
    assert_eq!(body["startDate"], "2025-06-19");
    assert_eq!(body["endDate"], "2026-10-01");
    assert_eq!(body["dataState"], "all");
    assert!(report.range.clamped);
    assert!(report.notices.contains(&Notice::StartClamped {
        earliest: options::parse_date("2025-06-19").unwrap()
    }));
    assert!(report
        .hint(&opts)
        .contains("Zeitraum 2025-06-19 – 2026-10-01"));
    server.stop().await;
}

// spec: INTEG-112
#[tokio::test]
async fn search_type_defaults_to_web_and_discover_has_no_position_and_no_query_or_device_filters() {
    let mock = GscMock::new();
    mock.set_rows(&[("https://integ.test/a", 3.0, 30.0, 0.1, 4.2)]);
    let server = start(&mock).await;
    let opts = options();
    let report = analytics::fetch_report(&client(&server, &opts), &opts, now()).await;
    assert_eq!(mock.requests_to("query")[0].body["type"], "web");
    assert_eq!(report.rows[0].position, Some(4.2));
    assert!(report.hint(&opts).contains("Suchtyp web"));

    for (kind, api) in [
        (SearchType::Discover, "discover"),
        (SearchType::GoogleNews, "googleNews"),
    ] {
        let mut opts = options();
        opts.search_type = kind;
        opts.filters =
            options::parse_filters("query contains shoe\ndevice equals MOBILE\ncountry equals deu")
                .unwrap();
        assert!(!opts.dimension_enabled(options::FilterDimension::Query));
        assert!(!opts.dimension_enabled(options::FilterDimension::Device));
        let report = analytics::fetch_report(&client(&server, &opts), &opts, now()).await;
        let body = mock.requests_to("query").last().unwrap().body.clone();
        assert_eq!(body["type"], api);
        let filters = body["dimensionFilterGroups"][0]["filters"]
            .as_array()
            .unwrap();
        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0]["dimension"], "country");
        assert_eq!(report.rows[0].position, None);
        assert!(report.notices.contains(&Notice::NoPosition));
        assert!(report.notices.contains(&Notice::FilterDisabled("query")));
        let values = analytics::values(
            &report.rows,
            &crawled(&["https://integ.test/a"]),
            &UrlMatchConfig::default(),
            now(),
            false,
        );
        // Spalte Position bleibt leer, die übrigen Werte sind da.
        assert_eq!(value(&values, "https://integ.test/a", "position"), None);
        assert_eq!(value(&values, "https://integ.test/a", "clicks"), Some(3.0));
    }
    server.stop().await;
}

// spec: INTEG-113
#[tokio::test]
async fn dimension_filters_are_sent_as_one_and_group_with_hint() {
    let mock = GscMock::new();
    let server = start(&mock).await;
    let cfg = os_model::config::GscConfig {
        site: Some("sc-domain:integ.test".into()),
        filter: Some(
            "query excludingRegex (?i)openspider|open spider\ncountry equals DEU\ndevice equals mobile".into(),
        ),
        ..Default::default()
    };
    let opts = GscOptions::from_config(&cfg).unwrap();
    let report = analytics::fetch_report(&client(&server, &opts), &opts, now()).await;
    let body = &mock.requests_to("query")[0].body;
    assert_eq!(
        body["dimensionFilterGroups"],
        json!([{ "groupType": "and", "filters": [
            { "dimension": "query", "operator": "excludingRegex", "expression": "(?i)openspider|open spider" },
            { "dimension": "country", "operator": "equals", "expression": "deu" },
            { "dimension": "device", "operator": "equals", "expression": "MOBILE" }
        ] }])
    );
    assert!(report.notices.contains(&Notice::AnonymizedQueries));
    assert!(Notice::AnonymizedQueries
        .text()
        .contains("anonymisierten Suchanfragen"));
    assert!(report.hint(&opts).contains(
        "Gefiltert: query excludingRegex (?i)openspider|open spider, country equals deu"
    ));
    // Ohne Suchanfrage-Filter kein Hinweis auf anonymisierte Suchanfragen.
    let mut country_only = opts.clone();
    country_only
        .filters
        .retain(|f| f.dimension == options::FilterDimension::Country);
    assert!(!analytics::notices(&country_only, &report.range).contains(&Notice::AnonymizedQueries));
    server.stop().await;
}

// spec: INTEG-113
#[tokio::test]
async fn invalid_re2_regex_is_rejected_before_sending() {
    let mock = GscMock::new();
    let server = start(&mock).await;
    for bad in [
        "query includingRegex (?<=open)spider",
        "query excludingRegex spider(?!s)",
        "query includingRegex (a)\\1",
        "query includingRegex (?>a+)b",
        "query includingRegex a++",
        "query includingRegex (open",
        "query includingRegex [a-z",
        "query includingRegex *spider",
    ] {
        let cfg = os_model::config::GscConfig {
            site: Some("sc-domain:integ.test".into()),
            filter: Some(bad.into()),
            ..Default::default()
        };
        assert!(
            matches!(
                GscOptions::from_config(&cfg),
                Err(OptionsError::Regex { .. })
            ),
            "{bad}"
        );
    }
    for good in [
        "(?i)openspider|open spider",
        "^shoe[s]?$",
        "a{2,3}b",
        "(?P<x>a)|[]a]",
        "[[:alpha:]]+",
        "a{,",
        "x*?",
    ] {
        assert_eq!(options::check_re2(good), Ok(()), "{good}");
    }
    for (bad, message) in [
        ("country equals germany", "ISO-3166"),
        ("device equals phone", "DESKTOP"),
        ("page equals x", "Dimension"),
        ("country includingRegex deu", "nur für query"),
    ] {
        let err = options::parse_filters(bad).unwrap_err().to_string();
        assert!(err.contains(message), "{bad}: {err}");
    }
    // Nichts wurde gesendet.
    assert!(mock.requests().is_empty());
    server.stop().await;
}

// spec: INTEG-114
#[tokio::test]
async fn sixty_thousand_rows_take_three_pages_and_max_rows_limits_the_total() {
    let mock = GscMock::new();
    mock.generate_rows(60_000);
    let server = start(&mock).await;
    let opts = options();
    let report = analytics::fetch_report(&client(&server, &opts), &opts, now()).await;
    let pages: Vec<(u64, u64)> = mock
        .requests_to("query")
        .iter()
        .map(|r| {
            (
                r.body["startRow"].as_u64().unwrap(),
                r.body["rowLimit"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(pages, [(0, 25_000), (25_000, 25_000), (50_000, 25_000)]);
    assert_eq!(report.rows.len(), 60_000);
    assert_eq!(report.pages, 3);
    assert!(report.error.is_none());

    let mut limited = opts.clone();
    limited.max_rows = Some(30_000);
    let before = mock.requests_to("query").len();
    let report = analytics::fetch_report(&client(&server, &limited), &limited, now()).await;
    let pages: Vec<(u64, u64)> = mock.requests_to("query")[before..]
        .iter()
        .map(|r| {
            (
                r.body["startRow"].as_u64().unwrap(),
                r.body["rowLimit"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(pages, [(0, 25_000), (25_000, 5_000)]);
    assert_eq!(report.rows.len(), 30_000);
    server.stop().await;
}

// spec: INTEG-114
#[tokio::test]
async fn empty_first_answer_is_no_error_and_assigns_nothing() {
    let mock = GscMock::new();
    mock.set_scenario(Some("empty"));
    let server = start(&mock).await;
    let opts = options();
    let report = analytics::fetch_report(&client(&server, &opts), &opts, now()).await;
    assert_eq!(mock.requests_to("query").len(), 1);
    assert!(report.rows.is_empty());
    assert!(report.error.is_none());
    assert_eq!(report.status_message(), None);
    let values = analytics::values(
        &report.rows,
        &crawled(&["https://integ.test/", "https://integ.test/a"]),
        &UrlMatchConfig::default(),
        now(),
        true,
    );
    assert!(values.is_empty());
    server.stop().await;
}

// spec: INTEG-117
#[tokio::test]
async fn by_page_aggregation_and_fragment_rows_are_assigned_without_summing() {
    let mock = GscMock::new();
    mock.set_rows(&[
        ("https://integ.test/x", 10.0, 100.0, 0.1, 3.0),
        ("https://integ.test/x#abschnitt", 4.0, 50.0, 0.08, 2.0),
        ("https://integ.test/y#abschnitt", 4.0, 40.0, 0.1, 6.0),
        ("https://integ.test/z#oben", 1.0, 10.0, 0.1, 6.0),
        ("https://integ.test/z#unten", 7.0, 70.0, 0.1, 6.0),
    ]);
    let server = start(&mock).await;
    let opts = options();
    let report = analytics::fetch_report(&client(&server, &opts), &opts, now()).await;
    let body = &mock.requests_to("query")[0].body;
    assert_eq!(body["aggregationType"], "byPage");
    assert_eq!(body["dimensions"], json!(["page"]));
    let urls = crawled(&[
        "https://integ.test/x",
        "https://integ.test/y",
        "https://integ.test/z",
    ]);
    let values = analytics::values(&report.rows, &urls, &opts.url_match, now(), true);
    assert_eq!(value(&values, "https://integ.test/x", "clicks"), Some(10.0));
    assert_eq!(
        value(&values, "https://integ.test/x", "impressions"),
        Some(100.0)
    );
    assert_eq!(value(&values, "https://integ.test/y", "clicks"), Some(4.0));
    // Mehrere Fragment-Zeilen ohne fragmentlose Zeile: die mit den meisten Clicks, nicht die Summe.
    assert_eq!(value(&values, "https://integ.test/z", "clicks"), Some(7.0));
    // Alle Zeilen haben eine Crawl-Entsprechung: keine Einträge ohne url_id.
    assert!(values.iter().all(|v| v.url_id.is_some()));
    server.stop().await;
}

// spec: INTEG-014
#[test]
fn url_matching_follows_protocol_slash_case_query_and_percent_rules() {
    let row = |page: &str, clicks: f64, impressions: f64| analytics::SaRow {
        page: page.into(),
        clicks,
        impressions,
        ctr: 0.0,
        position: Some(1.0),
    };
    let rows = vec![
        row("http://integ.test:8080/a/", 5.0, 50.0),
        row("https://integ.test:8080/b", 10.0, 100.0),
        row("https://integ.test:8080/b/", 30.0, 10.0),
        row("https://integ.test:8080/c/", 8.0, 80.0),
        row("https://integ.test:8080/Produkt", 5.0, 9.0),
        row("https://integ.test:8080/d?x=1", 2.0, 20.0),
        row("https://integ.test:8080/%C3%BC", 6.0, 60.0),
        row("https://integ.test:8080/t", 3.0, 30.0),
        row("https://integ.test:8080/t/", 3.0, 31.0),
    ];
    let urls = crawled(&[
        "https://integ.test:8080/a/",
        "https://integ.test:8080/b/",
        "https://integ.test:8080/c",
        "https://integ.test:8080/produkt",
        "https://integ.test:8080/d",
        "https://integ.test:8080/ü",
        "https://integ.test:8080/t",
    ]);
    let on = UrlMatchConfig::default();
    let values = analytics::values(&rows, &urls, &on, now(), true);
    let clicks = |url: &str, values: &[os_model::ApiValueWrite]| value(values, url, "clicks");
    assert_eq!(clicks("https://integ.test:8080/a/", &values), Some(5.0));
    // Varianten werden nicht summiert: höchste Clicks (30, nicht 40).
    assert_eq!(clicks("https://integ.test:8080/b/", &values), Some(30.0));
    assert_eq!(clicks("https://integ.test:8080/c", &values), Some(8.0));
    assert_eq!(
        clicks("https://integ.test:8080/produkt", &values),
        Some(5.0)
    );
    // Query-Strings bleiben Teil der Identität.
    assert_eq!(clicks("https://integ.test:8080/d", &values), None);
    assert_eq!(clicks("https://integ.test:8080/ü", &values), Some(6.0));
    // Gleichstand bei Clicks: Impressions entscheiden.
    assert_eq!(
        value(&values, "https://integ.test:8080/t", "impressions"),
        Some(31.0)
    );
    // Die nicht zugeordnete Zeile /d?x=1 landet ohne url_id (api_urls).
    assert!(values
        .iter()
        .any(|v| v.url_id.is_none() && v.url == "https://integ.test:8080/d?x=1"));

    let off = UrlMatchConfig {
        ignore_trailing_slash: false,
        ignore_case: false,
    };
    let values = analytics::values(&rows, &urls, &off, now(), false);
    assert_eq!(clicks("https://integ.test:8080/c", &values), None);
    assert_eq!(clicks("https://integ.test:8080/produkt", &values), None);
    assert_eq!(clicks("https://integ.test:8080/b/", &values), Some(30.0));
}

// spec: INTEG-128
#[tokio::test(start_paused = true)]
async fn hundred_second_answer_is_processed_with_the_default_read_timeout() {
    simulated(async {
        let mock = GscMock::new();
        mock.set_rows(&[("https://integ.test/a", 3.0, 30.0, 0.1, 4.2)]);
        mock.set_scenario(Some("delay-100s"));
        let server = start(&mock).await;
        let opts = options();
        assert_eq!(opts.read_timeout_s, 120);
        let report = analytics::fetch_report(&client(&server, &opts), &opts, now()).await;
        assert!(report.error.is_none(), "{:?}", report.error);
        assert_eq!(report.rows.len(), 1);
        assert_eq!(mock.requests_to("query").len(), 1);
        let values = analytics::values(
            &report.rows,
            &crawled(&["https://integ.test/a"]),
            &opts.url_match,
            now(),
            false,
        );
        assert_eq!(value(&values, "https://integ.test/a", "clicks"), Some(3.0));
        server.stop().await;
    })
    .await;
}

// spec: INTEG-128
#[tokio::test(start_paused = true)]
async fn thirty_second_timeout_retries_and_keeps_loaded_pages() {
    simulated(async {
        let mock = GscMock::new();
        mock.generate_rows(30_000);
        mock.set_scenario(Some("page2-delay-100s"));
        let server = start(&mock).await;
        let mut opts = options();
        opts.read_timeout_s = 30;
        opts.limits.retries = 2;
        let started = tokio::time::Instant::now();
        let report = analytics::fetch_report(&client(&server, &opts), &opts, now()).await;
        let requests = mock.requests_to("query");
        // Seite 1, dann Seite 2 mit 1 + 2 Wiederholungen.
        assert_eq!(requests.len(), 4);
        assert_eq!(report.error, Some(Failure::Timeout { after_s: 30 }));
        assert_eq!(
            report.status_message().as_deref(),
            Some("Fehler: Read timeout nach 30 s")
        );
        assert_eq!(report.rows.len(), 25_000);
        // Erster Timeout nach 30 s (±2 s) ab Senden der zweiten Seite.
        let first = requests[1].at;
        let second = requests[2].at;
        let gap = second.duration_since(first);
        assert!(
            gap >= Duration::from_secs(30)
                && gap <= Duration::from_secs(32) + Duration::from_secs(2),
            "{gap:?}"
        );
        // Backoff nach INTEG-005: Wartezeit zwischen Versuch 2 und 3 mindestens 0,8 · 2 s.
        let third = requests[3].at;
        assert!(
            third.duration_since(second) >= Duration::from_secs(30) + Duration::from_millis(1600)
        );
        assert!(started.elapsed() >= Duration::from_secs(90));
        server.stop().await;
    })
    .await;
}

// spec: INTEG-128
#[tokio::test(start_paused = true)]
async fn read_timeout_applies_per_request_and_only_to_search_console() {
    simulated(async {
        let mock = GscMock::new();
        mock.generate_rows(60_000);
        mock.set_scenario(Some("paged-delay-100s"));
        let server = start(&mock).await;
        let opts = options();
        let started = tokio::time::Instant::now();
        let report = analytics::fetch_report(&client(&server, &opts), &opts, now()).await;
        // 3 Seiten à 100 s mit Timeout 120 s → Erfolg.
        assert!(report.error.is_none(), "{:?}", report.error);
        assert_eq!(report.rows.len(), 60_000);
        assert_eq!(mock.requests_to("query").len(), 3);
        assert!(started.elapsed() >= Duration::from_secs(300));
        // Der allgemeine API-Client der übrigen Quellen behält seinen eigenen Timeout (30 s) – die GSC-Einstellung
        // (hier 300 s) ändert ihn nicht.
        let mut long = options();
        long.read_timeout_s = 300;
        assert_eq!(
            client(&server, &long).read_timeout(),
            Duration::from_secs(300)
        );
        let other = http::client(&Default::default()).unwrap();
        let before = tokio::time::Instant::now();
        let result = other
            .post(format!(
                "{}/webmasters/v3/sites/x/searchAnalytics/query",
                server.base_url("gsc")
            ))
            .header("Content-Type", "application/json")
            .body("{}")
            .send()
            .await;
        assert!(result.is_err_and(|e| e.is_timeout()));
        assert_eq!(before.elapsed(), Duration::from_secs(30));
        server.stop().await;
    })
    .await;
}

// spec: INTEG-128
#[test]
fn read_timeout_outside_ten_to_six_hundred_is_rejected() {
    for bad in [0, 9, 601, 3600] {
        assert_eq!(
            options::validate_read_timeout(bad),
            Err(OptionsError::ReadTimeout(bad))
        );
        let opts = GscOptions {
            read_timeout_s: bad,
            ..GscOptions::default()
        };
        assert!(opts.validate().is_err());
    }
    for good in [10, 120, 600] {
        assert_eq!(options::validate_read_timeout(good), Ok(()));
    }
    assert_eq!(GscOptions::default().read_timeout_s, 120);
    assert_eq!(options::CONNECT_TIMEOUT, Duration::from_secs(30));
}

// spec: INTEG-117
#[tokio::test]
async fn source_fetch_loads_the_report_once_and_assigns_values_per_url() {
    let mock = GscMock::new();
    mock.set_rows(&[
        ("https://integ.test/", 12.0, 340.0, 0.0353, 2.4),
        ("https://integ.test/a#top", 2.0, 20.0, 0.1, 7.0),
    ]);
    let server = start(&mock).await;
    let store = Arc::new(MemorySecretStore::new());
    Credentials::new(store.clone() as Arc<dyn SecretStoreMut>)
        .put(
            ApiKind::Gsc,
            Credential {
                account_id: "acct".into(),
                label: "Google-Konto".into(),
                api_key: None,
                access_token: Some("good".into()),
                refresh_token: None,
                expires_at: None,
                client_secret: None,
                scopes: vec![],
            },
            true,
        )
        .unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.api.gsc.enabled = true;
    cfg.api.gsc.site = Some("sc-domain:integ.test".into());
    cfg.api.gsc.base_url = Some(server.base_url("gsc"));
    let ctx = ApiCtx {
        cfg,
        secrets: store,
        network: Default::default(),
        allow_api_override: true,
    };
    let dir = tempfile::tempdir().unwrap();
    let source = GscSource::new(
        clock(),
        Arc::new(QuotaLedger::new(dir.path().join("q.json"))),
    );
    let mut all = Vec::new();
    for url in [
        "https://integ.test/",
        "https://integ.test/a",
        "https://integ.test/b",
    ] {
        all.extend(source.fetch(&crawled(&[url]), &ctx).await.unwrap());
    }
    // Ein Gesamtreport, keine Abfrage je URL.
    assert_eq!(mock.requests_to("query").len(), 1);
    assert_eq!(value(&all, "https://integ.test/", "clicks"), Some(12.0));
    assert_eq!(value(&all, "https://integ.test/", "ctr"), Some(0.0353));
    assert_eq!(value(&all, "https://integ.test/a", "clicks"), Some(2.0));
    assert_eq!(value(&all, "https://integ.test/b", "clicks"), None);
    assert!(all.iter().all(|v| v.source == "gsc" && v.level == "url"));
    source.reset().await;
    source
        .fetch(&crawled(&["https://integ.test/"]), &ctx)
        .await
        .unwrap();
    assert_eq!(mock.requests_to("query").len(), 2);
    server.stop().await;
}
