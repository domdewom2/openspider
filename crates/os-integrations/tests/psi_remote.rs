//! M2-I04: Remote-PSI gegen den lokalen Mock (`os_testsite::mocks::psi`): Verbinden, Gerätetyp, Ratenbegrenzung,
//! Tageskontingent, Restzeit und Fehlerbehandlung. Keine echten Schlüssel, nur Loopback.
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::Duration;

use os_fetch::MemorySecretStore;
use os_integrations::credentials::{Credential, Credentials};
use os_integrations::psi::{
    eta, lighthouse::Cell, remote, settings, settings::Source, PsiSource, Sleep,
};
use os_integrations::{ApiCtx, ApiError, ApiSource, ApiState, UrlRef};
use os_model::{ApiKind, CrawlConfig, ManualClock, PsiDevice, Timestamp};
use os_testsite::mocks::{psi::PsiMock, MockServer};

const KEY: &str = "fixture-key";

fn credential(key: &str) -> Credential {
    Credential {
        account_id: "psi-fixture".into(),
        label: "Fixture".into(),
        api_key: Some(key.into()),
        access_token: None,
        refresh_token: None,
        expires_at: None,
        client_secret: None,
        scopes: vec![],
    }
}

async fn setup() -> (MockServer, PsiMock, ApiCtx) {
    let mock = PsiMock::default();
    let server = MockServer::start(vec![Arc::new(mock.clone())])
        .await
        .unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.api.psi.enabled = true;
    cfg.api.psi.base_url = Some(server.base_url("psi"));
    let store = Arc::new(MemorySecretStore::new());
    Credentials::new(store.clone())
        .put(ApiKind::Psi, credential(KEY), true)
        .unwrap();
    let ctx = ApiCtx {
        cfg,
        secrets: store,
        network: Default::default(),
        allow_api_override: false,
    };
    (server, mock, ctx)
}

fn url(path: &str) -> UrlRef {
    UrlRef {
        url_id: Some(os_model::UrlId(1)),
        url: format!("https://site.test/{path}"),
    }
}

fn text(out: &os_integrations::psi::lighthouse::Outcome, name: &str) -> String {
    match out.get(name) {
        Some(Cell::Text(t)) => t.clone(),
        other => panic!("{name}: {other:?}"),
    }
}

/// Testuhr: jede Wartezeit stellt die Uhr vor und wird protokolliert.
fn test_sleep(clock: Arc<ManualClock>, log: Arc<Mutex<Vec<Duration>>>) -> Sleep {
    Arc::new(move |d| {
        clock.advance(d);
        log.lock().unwrap().push(d);
        Box::pin(async {})
    })
}

// spec: INTEG-130
#[tokio::test]
async fn connect_checks_the_key_once_and_reports_denied_access_with_checklist() {
    let (server, mock, ctx) = setup().await;
    assert_eq!(remote::check_key(&ctx, KEY).await, Ok(()));
    for (key, status, reason) in [
        ("key-not-enabled", 403, "SERVICE_DISABLED"),
        ("key-invalid", 400, "API_KEY_INVALID"),
    ] {
        let before = mock.calls().len();
        let err = remote::check_key(&ctx, key).await.unwrap_err();
        let remote::ConnectError::Denied(denied) = err else {
            panic!("{key}: {err:?}")
        };
        assert_eq!((denied.status, denied.reason.as_str()), (status, reason));
        assert_eq!(
            denied.status_line(),
            format!("Fehler: PSI-Zugriff abgelehnt ({reason})")
        );
        assert_eq!(denied.checklist().len(), 3);
        assert!(denied.checklist()[0].contains("PageSpeed Insights API"));
        assert!(denied.checklist()[1].contains("später erneut verbinden"));
        assert!(denied.checklist()[2].contains("beschränkt"));
        assert_eq!(mock.calls().len(), before + 1, "genau eine Testanfrage");
        // Über das Trait: dieselbe Statuszeile als Anbieterfehler.
        let e = PsiSource::new()
            .test_connection(&ctx, &credential(key))
            .await
            .unwrap_err();
        assert_eq!(
            e,
            ApiError::Provider {
                status,
                message: format!("Fehler: PSI-Zugriff abgelehnt ({reason})")
            }
        );
    }
    server.stop().await;
}

// spec: INTEG-130
#[tokio::test]
async fn new_profile_defaults_to_remote_and_stays_disconnected_without_key() {
    let cfg = CrawlConfig::default();
    assert_eq!(Source::of(&cfg.api.psi), Source::Remote);
    let (server, mock, mut ctx) = setup().await;
    ctx.secrets = Arc::new(MemorySecretStore::new());
    let source = PsiSource::new();
    // Ohne Schlüssel: nicht verbunden, keine Anfrage, kein Umschalten auf Lokal.
    let err = source.fetch(&[url("fast")], &ctx).await.unwrap_err();
    assert!(matches!(err, ApiError::NotConnected(_)), "{err:?}");
    assert!(matches!(
        source
            .test_connection(
                &ctx,
                &Credential {
                    api_key: None,
                    ..credential("")
                }
            )
            .await,
        Err(ApiError::NotConnected(_))
    ));
    assert!(mock.calls().is_empty());
    assert_eq!(Source::of(&ctx.cfg.api.psi), Source::Remote);
    server.stop().await;
}

// spec: INTEG-130
#[test]
fn only_internal_html_urls_with_status_200_are_requested() {
    use os_model::{IdSource, SeededIds};
    let dir = tempfile::tempdir().unwrap();
    let at = Timestamp::UNIX_EPOCH;
    let file = os_store::CrawlFile::create(
        &dir.path().join("psi.ospider"),
        &os_store::CrawlInit {
            config: CrawlConfig::default(),
            crawl_id: SeededIds::new(7).new_crawl_id(at),
            mode: os_model::CrawlMode::Spider,
            start_url: Some("https://site.test/".into()),
            list_source: None,
            project: "fixture".into(),
            name: "psi".into(),
            created_at: at,
        },
        os_store::StorageProfile::Balanced,
    )
    .unwrap();
    for (id, path, internal, class, status) in [
        (1, "/", 1, "HTML", 200),
        (2, "/moved", 1, "HTML", 301),
        (3, "/logo.png", 1, "Image", 200),
        (4, "/style.css", 1, "CSS", 200),
        (5, "/gone", 1, "HTML", 404),
        (6, "/page", 1, "HTML", 200),
    ] {
        file.conn()
            .execute(
                "INSERT INTO urls(id,url,address,origin,is_internal,in_tables,state,discovered_via,folder_depth,url_length,content_class,status_code) \
                 VALUES(?1,?2,?2,'https://site.test',?3,1,'done','link',0,20,?4,?5)",
                (id, format!("https://site.test{path}"), internal, class, status),
            )
            .unwrap();
    }
    file.conn()
        .execute(
            "INSERT INTO urls(id,url,address,origin,is_internal,in_tables,state,discovered_via,folder_depth,url_length,content_class,status_code) \
             VALUES(7,'https://other.test/','https://other.test/','https://other.test',0,1,'done','link',0,19,'HTML',200)",
            [],
        )
        .unwrap();
    let urls: Vec<String> = os_integrations::psi::eligible_urls(&file)
        .unwrap()
        .into_iter()
        .map(|u| u.url)
        .collect();
    assert_eq!(urls, ["https://site.test/", "https://site.test/page"]);
}

// spec: INTEG-131
#[tokio::test]
async fn default_request_uses_strategy_mobile_and_desktop_returns_desktop_field_data() {
    let (server, mock, mut ctx) = setup().await;
    let source = PsiSource::new();
    let mobile = source
        .fetch_outcome("https://site.test/crux-full", &ctx)
        .await
        .unwrap();
    assert_eq!(mock.calls()[0].strategy, "mobile");
    assert_eq!(
        mobile.get("CrUX Largest Contentful Paint (sec)"),
        Some(&Cell::Num(2.3))
    );
    ctx.cfg.api.psi.device = PsiDevice::Desktop;
    let desktop = source
        .fetch_outcome("https://site.test/crux-full", &ctx)
        .await
        .unwrap();
    assert_eq!(mock.calls()[1].strategy, "desktop");
    assert_eq!(
        desktop.get("CrUX Largest Contentful Paint (sec)"),
        Some(&Cell::Num(1.5))
    );
    assert_eq!(text(&desktop, "Core Web Vitals Assessment"), "Pass");
    let url = remote::request_url("http://x", "https://a.test/", "k", PsiDevice::Mobile).unwrap();
    assert!(url.query().unwrap().contains("strategy=mobile"));
    server.stop().await;
}

// spec: INTEG-142
#[tokio::test]
async fn requests_never_exceed_the_configured_rate_window() {
    // Standard: 60 Anfragen je 100 s, 5 gleichzeitig.
    let defaults = settings::effective_limits(&CrawlConfig::default().api.psi);
    assert_eq!(
        (
            defaults.requests_per_window,
            defaults.window_s,
            defaults.concurrency
        ),
        (Some(60), 100, 5)
    );
    // Messung im Mock mit verkleinertem Fenster (4 Anfragen je 1 s), damit der Test kurz bleibt.
    let (server, mock, mut ctx) = setup().await;
    ctx.cfg.api.psi.limits.requests_per_window = Some(4);
    ctx.cfg.api.psi.limits.window_s = 1;
    let source = Arc::new(PsiSource::new());
    let urls: Vec<UrlRef> = (0..10).map(|i| url(&format!("fast-{i}"))).collect();
    let queue = os_integrations::queue::Queue::default();
    let result = queue.run(source, &urls, &ctx, 5).await.unwrap();
    assert_eq!(result.succeeded.len(), 10);
    let calls = mock.calls();
    assert_eq!(calls.len(), 10);
    for (i, c) in calls.iter().enumerate() {
        let in_window = calls[i..]
            .iter()
            .filter(|o| o.at.duration_since(c.at) < Duration::from_millis(950))
            .count();
        assert!(in_window <= 4, "{in_window} Anfragen in einem Fenster");
    }
    server.stop().await;
}

// spec: INTEG-142
#[tokio::test]
async fn daily_quota_stops_requests_and_resets_at_the_pacific_day_boundary() {
    let (server, mock, mut ctx) = setup().await;
    assert_eq!(
        settings::daily_quota(&CrawlConfig::default().api.psi),
        25_000
    );
    ctx.cfg.api.psi.limits.quota_limit = Some(3);
    // 2026-10-19 06:30 UTC = 23:30 PDT am 18.10.
    let clock = Arc::new(ManualClock::new(
        Timestamp::from_db("2026-10-19T06:30:00Z").unwrap(),
    ));
    let source = PsiSource::new().with_clock(clock.clone());
    for i in 0..3 {
        let out = source
            .fetch_outcome(&format!("https://site.test/fast-{i}"), &ctx)
            .await
            .unwrap();
        assert!(out.is_success());
    }
    let out = source
        .fetch_outcome("https://site.test/fast-3", &ctx)
        .await
        .unwrap();
    assert_eq!(text(&out, "PSI Status"), "Error");
    assert!(text(&out, "PSI Error").starts_with("Quota exceeded: "));
    assert_eq!(
        mock.calls().len(),
        3,
        "keine Anfrage nach erschöpftem Kontingent"
    );
    assert_eq!(source.daily_used(), 3);
    // Tageswechsel America/Los_Angeles (Mitternacht PDT = 07:00 UTC).
    clock.advance(Duration::from_secs(31 * 60));
    assert_eq!(source.daily_used(), 0);
    assert!(source
        .fetch_outcome("https://site.test/fast-4", &ctx)
        .await
        .unwrap()
        .is_success());
    assert_eq!(mock.calls().len(), 4);
    server.stop().await;
}

// spec: INTEG-142
#[test]
fn remaining_time_estimate_matches_both_worked_examples() {
    let rate_bound = eta::remote(3_000, 60, 100, Duration::from_secs(4), 5);
    assert!((rate_bound.as_secs_f64() - 5_000.0).abs() <= 250.0);
    assert_eq!(eta::format(rate_bound), "ca. 1 h 23 min");
    let response_bound = eta::remote(3_000, 60, 100, Duration::from_secs(20), 5);
    assert!((response_bound.as_secs_f64() - 12_000.0).abs() <= 600.0);
    assert_eq!(eta::format(response_bound), "ca. 3 h 20 min");
    // Lokal: nur Laufzeit je URL und Parallelität.
    assert_eq!(
        eta::local(100, Duration::from_secs(30), 2),
        Duration::from_secs(1_500)
    );
}

// spec: INTEG-143
#[tokio::test]
async fn a_single_500_is_retried_and_yields_success() {
    let (server, mock, ctx) = setup().await;
    let clock = Arc::new(ManualClock::new(Timestamp::UNIX_EPOCH));
    let waits = Arc::new(Mutex::new(Vec::new()));
    let source = PsiSource::new()
        .with_clock(clock.clone())
        .with_sleep(test_sleep(clock, waits.clone()));
    let out = source
        .fetch_outcome("https://site.test/500-once", &ctx)
        .await
        .unwrap();
    assert_eq!(text(&out, "PSI Status"), "Success");
    assert_eq!(out.get("PSI Error"), None);
    assert_eq!(mock.calls().len(), 2);
    assert_eq!(waits.lock().unwrap().len(), 1, "ein Backoff");
    server.stop().await;
}

// spec: INTEG-143
#[tokio::test]
async fn more_than_ten_consecutive_500s_pause_the_integration_for_ten_minutes() {
    let (server, mock, ctx) = setup().await;
    let clock = Arc::new(ManualClock::new(Timestamp::UNIX_EPOCH));
    let waits = Arc::new(Mutex::new(Vec::new()));
    let seen: Arc<OnceLock<Weak<PsiSource>>> = Arc::new(OnceLock::new());
    let hints = Arc::new(Mutex::new(Vec::new()));
    let sleep: Sleep = {
        let (clock, waits, seen, hints) =
            (clock.clone(), waits.clone(), seen.clone(), hints.clone());
        Arc::new(move |d| {
            if let Some(source) = seen.get().and_then(Weak::upgrade) {
                let status = source.status();
                if status.state == ApiState::Paused {
                    hints
                        .lock()
                        .unwrap()
                        .push(status.detail.unwrap_or_default());
                }
            }
            clock.advance(d);
            waits.lock().unwrap().push(d);
            Box::pin(async {})
        })
    };
    let source = Arc::new(PsiSource::new().with_clock(clock.clone()).with_sleep(sleep));
    seen.set(Arc::downgrade(&source)).unwrap();
    // Standard: 5 Wiederholungen → je URL 6 Versuche; die 11. Anfrage in Folge löst die Pause aus.
    let first = source
        .fetch_outcome("https://site.test/500-burst", &ctx)
        .await;
    assert!(
        matches!(first, Err(ref e) if e.http_status() == Some(500)),
        "{first:?}"
    );
    assert!(source.paused_until().is_none());
    let second = source
        .fetch_outcome("https://site.test/b/500-burst", &ctx)
        .await;
    assert!(second.is_err());
    let waits = waits.lock().unwrap().clone();
    assert!(
        waits.contains(&Duration::from_secs(600)),
        "Pause von 10 Minuten: {waits:?}"
    );
    let hints = hints.lock().unwrap().clone();
    assert!(
        hints.iter().any(|h| h.contains("pausiert 10 Minuten")),
        "Hinweis für die UI: {hints:?}"
    );
    // Vor der Pause wurden genau 11 Anfragen gesendet, danach wieder.
    assert!(mock.calls().len() > 11);
    server.stop().await;
}

// spec: INTEG-143
#[tokio::test]
async fn lighthouse_runtime_error_is_not_retried() {
    let (server, mock, ctx) = setup().await;
    let source = PsiSource::new();
    let values = source
        .fetch(&[url("lh-errored-document")], &ctx)
        .await
        .unwrap();
    assert_eq!(mock.calls().len(), 1, "keine Wiederholung");
    let status = values.iter().find(|v| v.metric == "PSI Status").unwrap();
    assert_eq!(status.v_text.as_deref(), Some("Error"));
    let error = values.iter().find(|v| v.metric == "PSI Error").unwrap();
    assert!(error
        .v_text
        .as_deref()
        .unwrap()
        .starts_with("ERRORED_DOCUMENT_REQUEST "));
    assert!(values
        .iter()
        .all(|v| v.source == "psi" && v.url_id == Some(os_model::UrlId(1))));
    server.stop().await;
}

// spec: INTEG-143
#[tokio::test]
async fn exhausted_provider_quota_is_an_error_with_quota_message() {
    let (server, mock, ctx) = setup().await;
    let source = PsiSource::new();
    let out = source
        .fetch_outcome("https://site.test/quota-exhausted", &ctx)
        .await
        .unwrap();
    assert_eq!(text(&out, "PSI Status"), "Error");
    assert!(text(&out, "PSI Error").starts_with("Quota exceeded:"));
    assert_ne!(text(&out, "PSI Status"), "Quota exceeded");
    assert_eq!(mock.calls().len(), 1);
    server.stop().await;
}
