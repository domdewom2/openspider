//! Search Console – URL Inspection gegen den lokalen Mock: Tageskontingent und Reihenfolge, Drosselung, Auslassen
//! nicht indexierbarer URLs, mehrere Properties, die 17 Spalten, `Summary`, Fehler und Read-Timeout. Keine echten
//! Netzwerkaufrufe; Kontingentzähler in einer temporären Datei, Verzögerungen mit pausierter tokio-Uhr.

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use os_integrations::gsc::client::GscHttp;
use os_integrations::gsc::inspection::{self, summary, Inspector, Outcome, Target, QUOTA_SOURCE};
use os_integrations::gsc::options::GscOptions;
use os_integrations::gsc::sites::{self, Property};
use os_integrations::{http, quota::QuotaLedger};
use os_model::{Clock, CrawlConfig, CrawlMode, ManualClock, Timestamp, UrlId};
use os_testsite::mocks::gsc::{fixture, GscMock};
use os_testsite::mocks::MockServer;

fn now() -> Timestamp {
    Timestamp::from_db("2026-10-19T12:00:00Z").unwrap()
}

fn clock() -> Arc<dyn Clock> {
    Arc::new(ManualClock::new(now()))
}

fn options(site: &str) -> GscOptions {
    GscOptions {
        site: Some(site.into()),
        url_inspection: true,
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

fn html(url: &str) -> Target {
    Target {
        url_id: None,
        url: url.into(),
        is_html: true,
        indexable: true,
    }
}

fn ledger(dir: &tempfile::TempDir) -> QuotaLedger {
    QuotaLedger::new(dir.path().join("api-quotas.json"))
}

fn used(ledger: &QuotaLedger, property: &str) -> u64 {
    2_000
        - ledger
            .remaining(QUOTA_SOURCE, property, now(), 2_000)
            .unwrap()
}

/// Prüft eine Folge von Zeitpunkten: in keinem gleitenden 60-s-Fenster mehr als 600.
fn max_per_minute(mut at: Vec<tokio::time::Instant>) -> usize {
    at.sort();
    let mut window = VecDeque::new();
    let mut max = 0;
    for t in at {
        window.push_back(t);
        while window
            .front()
            .is_some_and(|f| t.duration_since(*f) >= Duration::from_secs(60))
        {
            window.pop_front();
        }
        max = max.max(window.len());
    }
    max
}

/// Crawl-Datei mit den gegebenen URLs `(url, crawl_depth, content_class, indexability)` in Einfügereihenfolge.
fn crawl_file(
    dir: &tempfile::TempDir,
    mode: CrawlMode,
    rows: &[(String, i64, &str, &str)],
) -> os_store::CrawlFile {
    let init = os_store::CrawlInit {
        crawl_id: os_model::IdSource::new_crawl_id(&os_model::SeededIds::new(7), now()),
        mode,
        start_url: Some("https://integ.test/".into()),
        list_source: None,
        project: "test".into(),
        name: "gsc".into(),
        config: CrawlConfig::default(),
        created_at: now(),
    };
    let file = os_store::CrawlFile::create(
        &dir.path().join("crawl.ospider"),
        &init,
        os_store::StorageProfile::Balanced,
    )
    .unwrap();
    let conn = file.conn();
    conn.execute_batch("BEGIN").unwrap();
    for (url, depth, class, indexability) in rows {
        conn.execute(
            "INSERT INTO urls (url, address, origin, is_internal, state, discovered_via, crawl_depth, \
             folder_depth, url_length, content_class, indexability) \
             VALUES (?1, ?1, 'link', 1, 'done', 'link', ?2, 1, 10, ?3, ?4)",
            rusqlite_params(url, *depth, class, indexability),
        )
        .unwrap();
    }
    conn.execute_batch("COMMIT").unwrap();
    file
}

fn rusqlite_params<'a>(
    url: &'a str,
    depth: i64,
    class: &'a str,
    indexability: &'a str,
) -> (&'a str, i64, &'a str, &'a str) {
    (url, depth, class, indexability)
}

// spec: INTEG-120
#[tokio::test(start_paused = true)]
async fn of_2500_html_urls_the_2000_shallowest_are_inspected_and_the_rest_get_quota_exceeded() {
    simulated(async {
        let dir = tempfile::tempdir().unwrap();
        // 2 500 HTML-Seiten auf 5 Ebenen, Einfügereihenfolge absichtlich nicht nach Tiefe; dazu Ressourcen.
        let mut rows = Vec::new();
        for i in 0..2_500i64 {
            let depth = 4 - (i % 5);
            rows.push((
                format!("https://integ.test/d{depth}/p{i}"),
                depth,
                "HTML",
                "Indexable",
            ));
        }
        for (path, class) in [
            ("/img.png", "Images"),
            ("/doc.pdf", "PDF"),
            ("/app.css", "CSS"),
        ] {
            rows.push((format!("https://integ.test{path}"), 1, class, "Indexable"));
        }
        let file = crawl_file(&dir, CrawlMode::Spider, &rows);
        let targets = inspection::targets_from_file(&file).unwrap();
        assert_eq!(targets.len(), 2_503);
        // Reihenfolge: kleinste Crawl-Tiefe zuerst, darin früheste Entdeckung.
        assert!(targets[0].url.starts_with("https://integ.test/d0/"));
        assert!(targets
            .last()
            .unwrap()
            .url
            .starts_with("https://integ.test/d4/"));

        let mock = GscMock::new();
        let server = start(&mock).await;
        let opts = options("sc-domain:integ.test");
        let http = client(&server, &opts);
        let ledger = ledger(&dir);
        let outcomes = Inspector::new(&http, &ledger, clock(), &opts, &[])
            .run(&targets)
            .await
            .unwrap();
        let requests = mock.requests_to("inspect");
        assert_eq!(requests.len(), 2_000);
        assert_eq!(used(&ledger, "sc-domain:integ.test"), 2_000);
        let inspected: Vec<&str> = targets
            .iter()
            .zip(&outcomes)
            .filter(|(_, o)| matches!(o, Outcome::Inspected { .. }))
            .map(|(t, _)| t.url.as_str())
            .collect();
        assert_eq!(inspected.len(), 2_000);
        // Die ersten 2 000 in Entdeckungsreihenfolge (Ebenen 0 – 3 komplett, 0 aus Ebene 4).
        assert!(inspected.iter().all(|u| !u.contains("/d4/")));
        let quota: Vec<&Target> = targets
            .iter()
            .zip(&outcomes)
            .filter(|(_, o)| o.summary().as_deref() == Some(summary::QUOTA_EXCEEDED))
            .map(|(t, _)| t)
            .collect();
        assert_eq!(quota.len(), 500);
        assert!(quota.iter().all(|t| t.url.contains("/d4/")));
        // Nicht-HTML-URLs werden nie abgefragt und haben kein Summary.
        for (t, o) in targets.iter().zip(&outcomes) {
            if !t.is_html {
                assert_eq!(o, &Outcome::Skipped, "{}", t.url);
                assert!(o.values(t, now()).is_empty());
                assert!(requests
                    .iter()
                    .all(|r| r.body["inspectionUrl"] != t.url.as_str()));
            }
        }
        // Nie mehr als 600 Anfragen je Minute und Property (Messung am Mock).
        assert!(max_per_minute(requests.iter().map(|r| r.at).collect()) <= 600);
        // Ein zweiter Lauf am selben Pazifiktag findet kein Kontingent mehr.
        let again = Inspector::new(&http, &ledger, clock(), &opts, &[])
            .run(&targets[..3])
            .await
            .unwrap();
        assert!(again
            .iter()
            .all(|o| o.summary().as_deref() == Some(summary::QUOTA_EXCEEDED)));
        assert_eq!(mock.requests_to("inspect").len(), 2_000);
        server.stop().await;
    })
    .await;
}

// spec: INTEG-120
#[tokio::test]
async fn list_mode_inspects_in_list_order() {
    let dir = tempfile::tempdir().unwrap();
    let rows: Vec<_> = ["a", "b", "c"]
        .iter()
        .map(|p| (format!("https://integ.test/{p}"), 0, "HTML", "Indexable"))
        .collect();
    let file = crawl_file(&dir, CrawlMode::List, &rows);
    // Hochgeladene Liste: c, a, b.
    for (position, id) in [(1, 3), (2, 1), (3, 2)] {
        file.conn()
            .execute(
                "INSERT INTO list_input (position, original, url_id, outcome) VALUES (?1, 'x', ?2, 'ok')",
                (position, id),
            )
            .unwrap();
    }
    let targets = inspection::targets_from_file(&file).unwrap();
    let urls: Vec<&str> = targets.iter().map(|t| t.url.as_str()).collect();
    assert_eq!(
        urls,
        [
            "https://integ.test/c",
            "https://integ.test/a",
            "https://integ.test/b"
        ]
    );
    assert_eq!(targets[0].url_id, Some(UrlId(3)));
    let mock = GscMock::new();
    let server = start(&mock).await;
    let mut opts = options("sc-domain:integ.test");
    opts.limits.concurrency = 1;
    let http = client(&server, &opts);
    let ledger = ledger(&dir);
    Inspector::new(&http, &ledger, clock(), &opts, &[])
        .run(&targets)
        .await
        .unwrap();
    let sent: Vec<String> = mock
        .requests_to("inspect")
        .iter()
        .map(|r| r.body["inspectionUrl"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(sent, urls);
    server.stop().await;
}

// spec: INTEG-121
#[tokio::test]
async fn skipping_non_indexable_urls_saves_quota() {
    let dir = tempfile::tempdir().unwrap();
    let targets = vec![
        html("https://integ.test/"),
        html("https://integ.test/ok"),
        Target {
            indexable: false,
            ..html("https://integ.test/noindex")
        },
        Target {
            indexable: false,
            ..html("https://integ.test/canonicalised")
        },
    ];
    let mock = GscMock::new();
    let server = start(&mock).await;
    let mut opts = options("sc-domain:integ.test");
    opts.skip_non_indexable = true;
    let http = client(&server, &opts);
    let ledger = ledger(&dir);
    let outcomes = Inspector::new(&http, &ledger, clock(), &opts, &[])
        .run(&targets)
        .await
        .unwrap();
    let sent: Vec<String> = mock
        .requests_to("inspect")
        .iter()
        .map(|r| r.body["inspectionUrl"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(sent.len(), 2);
    assert!(sent
        .iter()
        .all(|u| !u.contains("noindex") && !u.contains("canonicalised")));
    assert_eq!(used(&ledger, "sc-domain:integ.test"), 2);
    // Ausgelassene URLs: Summary leer.
    assert_eq!(outcomes[2], Outcome::Skipped);
    assert_eq!(outcomes[3].summary(), None);
    // Ohne Option werden auch sie abgefragt.
    opts.skip_non_indexable = false;
    Inspector::new(&http, &ledger, clock(), &opts, &[])
        .run(&targets[2..])
        .await
        .unwrap();
    assert_eq!(mock.requests_to("inspect").len(), 4);
    assert_eq!(used(&ledger, "sc-domain:integ.test"), 4);
    server.stop().await;
}

fn three_properties() -> Vec<Property> {
    vec![
        Property::new("sc-domain:integ.test", "siteOwner"),
        Property::new("https://integ.test/", "siteOwner"),
        Property::new("https://integ.test/blog/", "siteFullUser"),
    ]
}

// spec: INTEG-122
#[test]
fn each_url_uses_the_most_specific_property() {
    let properties = three_properties();
    let pick = |url: &str| sites::most_specific(url, &properties).map(|p| p.site_url.clone());
    assert_eq!(
        pick("https://integ.test/blog/a").as_deref(),
        Some("https://integ.test/blog/")
    );
    assert_eq!(
        pick("https://integ.test/shop/b").as_deref(),
        Some("https://integ.test/")
    );
    assert_eq!(
        pick("https://sub.integ.test/c").as_deref(),
        Some("sc-domain:integ.test")
    );
    assert_eq!(
        pick("http://integ.test/blog/a").as_deref(),
        Some("sc-domain:integ.test")
    );
    assert_eq!(pick("https://other.test/"), None);
    // Unverifizierte Properties werden nie verwendet.
    let with_unverified = vec![
        Property::new("sc-domain:integ.test", "siteOwner"),
        Property::new("https://integ.test/blog/", sites::UNVERIFIED),
    ];
    assert_eq!(
        sites::most_specific("https://integ.test/blog/a", &with_unverified)
            .map(|p| p.site_url.as_str()),
        Some("sc-domain:integ.test")
    );
}

// spec: INTEG-122
#[tokio::test(start_paused = true)]
async fn multiple_properties_raise_the_daily_limit_without_fallback_between_properties() {
    simulated(async {
        let dir = tempfile::tempdir().unwrap();
        let mock = GscMock::new();
        let server = start(&mock).await;
        let mut opts = options("sc-domain:integ.test");
        opts.multiple_properties = true;
        let http = client(&server, &opts);
        let ledger = ledger(&dir);
        let properties = vec![
            Property::new("https://integ.test/", "siteOwner"),
            Property::new("https://integ.test/blog/", "siteFullUser"),
        ];
        // 3 000 URLs, je 1 500 je Property.
        let mut targets = Vec::new();
        for i in 0..1_500 {
            targets.push(html(&format!("https://integ.test/blog/p{i}")));
            targets.push(html(&format!("https://integ.test/shop/p{i}")));
        }
        let outcomes = Inspector::new(&http, &ledger, clock(), &opts, &properties)
            .run(&targets)
            .await
            .unwrap();
        assert!(outcomes.iter().all(|o| matches!(o, Outcome::Inspected { .. })));
        assert_eq!(mock.requests_to("inspect").len(), 3_000);
        let mut per_property: BTreeMap<String, usize> = BTreeMap::new();
        for r in mock.requests_to("inspect") {
            *per_property.entry(r.property.clone().unwrap()).or_default() += 1;
        }
        assert_eq!(per_property["https://integ.test/"], 1_500);
        assert_eq!(per_property["https://integ.test/blog/"], 1_500);
        // Spalte Inspection Property.
        let values = outcomes[0].values(&targets[0], now());
        assert!(values
            .iter()
            .any(|v| v.metric == "inspection_property" && v.v_text.as_deref() == Some("https://integ.test/blog/")));
        // Das Kontingent von /blog/ ist erschöpft: keine Ausweichproperty, sondern Quota exceeded; der Rest von
        // https://integ.test/ (500) bleibt nutzbar.
        assert_eq!(
            ledger
                .reserve_up_to(QUOTA_SOURCE, "https://integ.test/blog/", now(), 2_000, 2_000)
                .unwrap(),
            500
        );
        let more = vec![html("https://integ.test/blog/extra"), html("https://integ.test/shop/extra")];
        let before = mock.requests_to("inspect").len();
        let outcomes = Inspector::new(&http, &ledger, clock(), &opts, &properties)
            .run(&more)
            .await
            .unwrap();
        assert_eq!(
            outcomes[0],
            Outcome::QuotaExceeded {
                property: "https://integ.test/blog/".into()
            }
        );
        assert!(matches!(&outcomes[1], Outcome::Inspected { property, .. } if property == "https://integ.test/"));
        assert_eq!(mock.requests_to("inspect").len(), before + 1);
        // Mit erschöpfter /blog/-Property: kein Rückfall auf die Domain- oder die Stamm-Property.
        ledger
            .reserve_up_to(QUOTA_SOURCE, "https://integ.test/", now(), 2_000, 2_000)
            .unwrap();
        let outcomes = Inspector::new(&http, &ledger, clock(), &opts, &three_properties())
            .run(&[html("https://integ.test/blog/late")])
            .await
            .unwrap();
        assert_eq!(outcomes[0].summary().as_deref(), Some(summary::QUOTA_EXCEEDED));
        assert_eq!(mock.requests_to("inspect").len(), before + 1);
        server.stop().await;
    })
    .await;
}

/// `expected-inspection.csv` (INTEG-123): eine Zeile je Mock-Antwort, 17 Spalten.
const EXPECTED_INSPECTION_CSV: &str = "\
fixture\tSummary\tCoverage\tLast Crawl\tCrawled As\tCrawl Allowed\tPage Fetch\tIndexing Allowed\tUser-Declared Canonical\tGoogle-Selected Canonical\tMobile Usability\tMobile Usability Issues\tAMP Results\tAMP Issues\tRich Results\tRich Results Types\tRich Results Types Errors\tRich Results Warnings
inspect-indexed\tURL is on Google\tSubmitted and indexed\t2026-10-10T08:15:30Z\tMobile\tYes\tSuccessful\tYes\t{url}\t{url}\tPass\t\t\t\tPass\tBreadcrumbs\t\t
inspect-not-indexed\tURL is not on Google\tCrawled - currently not indexed\t2026-09-30T23:01:02Z\tDesktop\tYes\tSuccessful\tYes\t\t\t\t\t\t\t\t\t\t
inspect-canonical-mismatch\tURL is not on Google\tDuplicate, Google chose different canonical than user\t2026-10-01T10:00:00Z\tMobile\tYes\tSuccessful\tYes\t{url}\thttps://integ.test/original\t\t\t\t\t\t\t\t
inspect-rich-error\tURL is on Google, but has issues\tSubmitted and indexed\t2026-10-10T08:15:30Z\tMobile\tYes\tSuccessful\tYes\t{url}\t{url}\t\t\t\t\tFail\tProduct snippets; Breadcrumbs; FAQ\tProduct snippets\tBreadcrumbs
inspect-amp-fail\tURL is on Google, but has issues\tSubmitted and indexed\t2026-10-10T08:15:30Z\tMobile\tYes\tSuccessful\tYes\t{url}\t{url}\t\t\tFail\tThe attribute 'onclick' may not appear in tag 'div'.; Custom JavaScript is not allowed.\t\t\t\t
inspect-mobile-fail\tURL is on Google, but has issues\tSubmitted and indexed\t2026-10-10T08:15:30Z\tMobile\tYes\tSuccessful\tYes\t{url}\t{url}\tFail\tUSES_INCOMPATIBLE_PLUGINS; TEXT_TOO_SMALL\t\t\t\t\t\t
inspect-minimal\tURL is not on Google\tURL is unknown to Google\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t
";

// spec: INTEG-123
#[tokio::test]
async fn all_seventeen_columns_are_filled_from_the_mock_answers() {
    let dir = tempfile::tempdir().unwrap();
    let mock = GscMock::new();
    let mut lines = EXPECTED_INSPECTION_CSV.lines();
    let header: Vec<&str> = lines.next().unwrap().split('\t').collect();
    assert_eq!(header.len(), 18);
    let expected: Vec<Vec<&str>> = lines.map(|l| l.split('\t').collect()).collect();
    let mut targets = Vec::new();
    for row in &expected {
        let url = format!("https://integ.test/{}", row[0]);
        mock.set_inspection(&url, fixture(row[0]).unwrap());
        targets.push(html(&url));
    }
    let server = start(&mock).await;
    let opts = options("sc-domain:integ.test");
    let http = client(&server, &opts);
    let ledger = ledger(&dir);
    let outcomes = Inspector::new(&http, &ledger, clock(), &opts, &[])
        .run(&targets)
        .await
        .unwrap();
    for ((row, target), outcome) in expected.iter().zip(&targets).zip(&outcomes) {
        let Outcome::Inspected { fields, .. } = outcome else {
            panic!("{}: {outcome:?}", row[0]);
        };
        let columns = fields.columns();
        assert_eq!(columns.len(), 17);
        for (i, (_, name, value)) in columns.iter().enumerate() {
            assert_eq!(*name, header[i + 1]);
            let want = row[i + 1].replace("{url}", &target.url);
            assert_eq!(value.unwrap_or(""), want, "{} / {name}", row[0]);
        }
        // Fehlende Teilergebnisse: Spalten leer, als Werte gar nicht geschrieben (dazu Inspection Property und, wenn
        // beide Canonicals gesetzt sind, der Vergleich für INTEG-125).
        let values = outcome.values(target, now());
        let canonicals = usize::from(fields.canonical_not_selected().is_some());
        assert_eq!(
            values.len(),
            columns.iter().filter(|(_, _, v)| v.is_some()).count() + 1 + canonicals
        );
        let mismatch = values
            .iter()
            .find(|v| v.metric == "canonical_not_selected")
            .and_then(|v| v.v_real);
        let expected = match row[0] {
            "inspect-canonical-mismatch" => Some(1.0),
            "inspect-not-indexed" | "inspect-minimal" => None,
            _ => Some(0.0),
        };
        assert_eq!(mismatch, expected, "{}", row[0]);
    }
    // Verdikte lesbar.
    for (raw, label) in [
        ("PASS", "Pass"),
        ("PARTIAL", "Partial"),
        ("FAIL", "Fail"),
        ("NEUTRAL", "Neutral"),
    ] {
        assert_eq!(inspection::verdict_label(raw), Some(label));
    }
    assert_eq!(inspection::verdict_label("VERDICT_UNSPECIFIED"), None);
    server.stop().await;
}

// spec: INTEG-124
#[test]
fn summary_is_derived_from_the_index_verdict_and_the_partial_results() {
    let result = |index: &str, mobile: Option<&str>, amp: Option<&str>, rich: Option<&str>| {
        let mut r = serde_json::json!({ "indexStatusResult": { "verdict": index } });
        for (key, v) in [
            ("mobileUsabilityResult", mobile),
            ("ampResult", amp),
            ("richResultsResult", rich),
        ] {
            if let Some(v) = v {
                r[key] = serde_json::json!({ "verdict": v });
            }
        }
        inspection::derive_summary(&r)
    };
    assert_eq!(
        result("PASS", Some("PASS"), None, Some("PASS")),
        summary::ON_GOOGLE
    );
    assert_eq!(result("PASS", None, None, None), summary::ON_GOOGLE);
    assert_eq!(
        result("PASS", Some("NEUTRAL"), None, None),
        summary::ON_GOOGLE
    );
    for issues in [
        (Some("FAIL"), None, None),
        (None, Some("PARTIAL"), None),
        (None, None, Some("FAIL")),
        (Some("PASS"), Some("PASS"), Some("PARTIAL")),
    ] {
        assert_eq!(
            result("PASS", issues.0, issues.1, issues.2),
            summary::ON_GOOGLE_WITH_ISSUES
        );
    }
    for verdict in ["FAIL", "NEUTRAL", "PARTIAL"] {
        assert_eq!(
            result(verdict, Some("PASS"), None, None),
            summary::NOT_ON_GOOGLE
        );
    }
    assert_eq!(
        inspection::derive_summary(&serde_json::json!({})),
        summary::NOT_ON_GOOGLE
    );
    // Fehler der API für eine URL → Error: …
    let error = Outcome::Error {
        property: Some("sc-domain:integ.test".into()),
        message: "URL not in property".into(),
    };
    assert_eq!(
        error.summary().as_deref(),
        Some("Error: URL not in property")
    );
}

// spec: INTEG-127
#[tokio::test]
async fn foreign_host_and_403_become_errors_without_local_quota_use() {
    let dir = tempfile::tempdir().unwrap();
    let mock = GscMock::new();
    let server = start(&mock).await;
    let opts = options("sc-domain:integ.test");
    let http = client(&server, &opts);
    let ledger = ledger(&dir);
    let outcomes = Inspector::new(&http, &ledger, clock(), &opts, &[])
        .run(&[html("https://fremd.test/x")])
        .await
        .unwrap();
    assert_eq!(
        outcomes[0].summary().as_deref(),
        Some("Error: URL not in property")
    );
    assert!(mock.requests_to("inspect").is_empty());
    assert_eq!(used(&ledger, "sc-domain:integ.test"), 0);
    // Antwortet die API selbst mit 403 (URL nicht in der Property), gilt dasselbe Summary; keine Wiederholung.
    mock.set_scenario(Some("403-not-in-property"));
    let outcomes = Inspector::new(&http, &ledger, clock(), &opts, &[])
        .run(&[html("https://integ.test/x")])
        .await
        .unwrap();
    assert_eq!(
        outcomes[0].summary().as_deref(),
        Some("Error: URL not in property")
    );
    assert_eq!(mock.requests_to("inspect").len(), 1);
    server.stop().await;
}

// spec: INTEG-127
#[tokio::test]
async fn daily_quota_answer_marks_all_remaining_urls_without_further_requests() {
    let dir = tempfile::tempdir().unwrap();
    let mock = GscMock::new();
    mock.set_remaining("sc-domain:integ.test", 3);
    let server = start(&mock).await;
    let mut opts = options("sc-domain:integ.test");
    opts.limits.concurrency = 1;
    let http = client(&server, &opts);
    let ledger = ledger(&dir);
    let targets: Vec<Target> = (0..10)
        .map(|i| html(&format!("https://integ.test/p{i}")))
        .collect();
    let outcomes = Inspector::new(&http, &ledger, clock(), &opts, &[])
        .run(&targets)
        .await
        .unwrap();
    assert!(outcomes[..3]
        .iter()
        .all(|o| matches!(o, Outcome::Inspected { .. })));
    assert!(outcomes[3..]
        .iter()
        .all(|o| o.summary().as_deref() == Some(summary::QUOTA_EXCEEDED)));
    // Drei Erfolge, eine 429-Antwort, danach keine Anfrage mehr (keine Wiederholung bei Tageskontingent).
    assert_eq!(mock.requests_to("inspect").len(), 4);
    // Der lokale Zähler steht danach auf dem Limit.
    assert_eq!(used(&ledger, "sc-domain:integ.test"), 2_000);
    let later = Inspector::new(&http, &ledger, clock(), &opts, &[])
        .run(&targets[..1])
        .await
        .unwrap();
    assert_eq!(later[0].summary().as_deref(), Some(summary::QUOTA_EXCEEDED));
    assert_eq!(mock.requests_to("inspect").len(), 4);
    server.stop().await;
}

// spec: INTEG-127
#[tokio::test(start_paused = true)]
async fn slow_inspection_answers_are_processed_or_end_in_read_timeout_after_retries() {
    simulated(async {
        let dir = tempfile::tempdir().unwrap();
        let mock = GscMock::new();
        mock.set_scenario(Some("delay-90s"));
        let server = start(&mock).await;
        let opts = options("sc-domain:integ.test");
        let http = client(&server, &opts);
        let ledger = ledger(&dir);
        let outcomes = Inspector::new(&http, &ledger, clock(), &opts, &[])
            .run(&[html("https://integ.test/slow")])
            .await
            .unwrap();
        assert_eq!(outcomes[0].summary().as_deref(), Some(summary::ON_GOOGLE));
        assert_eq!(mock.requests_to("inspect").len(), 1);

        mock.set_scenario(Some("delay-180s"));
        let started = tokio::time::Instant::now();
        let outcomes = Inspector::new(&http, &ledger, clock(), &opts, &[])
            .run(&[html("https://integ.test/slower")])
            .await
            .unwrap();
        assert_eq!(
            outcomes[0].summary().as_deref(),
            Some("Error: Read timeout")
        );
        // 1 Versuch + 5 Wiederholungen (INTEG-005), jeder nach 120 s abgebrochen.
        assert_eq!(mock.requests_to("inspect").len(), 1 + 6);
        assert!(started.elapsed() >= Duration::from_secs(6 * 120));
        server.stop().await;
    })
    .await;
}

// spec: INTEG-128
#[tokio::test(start_paused = true)]
async fn inspection_uses_the_configured_read_timeout() {
    simulated(async {
        let dir = tempfile::tempdir().unwrap();
        let mock = GscMock::new();
        mock.set_scenario(Some("delay-90s"));
        let server = start(&mock).await;
        let mut opts = options("sc-domain:integ.test");
        opts.read_timeout_s = 30;
        opts.limits.retries = 1;
        let http = client(&server, &opts);
        let ledger = ledger(&dir);
        let outcomes = Inspector::new(&http, &ledger, clock(), &opts, &[])
            .run(&[html("https://integ.test/slow")])
            .await
            .unwrap();
        assert_eq!(
            outcomes[0].summary().as_deref(),
            Some("Error: Read timeout")
        );
        let requests = mock.requests_to("inspect");
        assert_eq!(requests.len(), 2);
        let gap = requests[1].at.duration_since(requests[0].at);
        assert!(
            gap >= Duration::from_secs(30) && gap <= Duration::from_secs(32),
            "{gap:?}"
        );
        server.stop().await;
    })
    .await;
}
