//! Fixture-Tests gegen `os-testsite`: Statuscodes und Klassen (CRAWL-012), Inhaltstypen
//! (CRAWL-019, ONPAGE-020), Zeit- und Fehlerfälle (CRAWL-013/022/095).
//!
//! Fixtures: `tests/fixtures/crawl-{status-codes,content-types,timing,no-response,http-basic}`.

#[path = "http_support.rs"]
mod support;

use std::sync::Arc;
use std::time::Duration;

use os_fetch::http::StatusClass;
use os_fetch::{content_class, Fetch};
use os_model::{ContentClass, NoResponseReason, SystemClock};
use os_testsite::TestSite;
use support::*;
use url::Url;

fn url(site: &TestSite, path: &str) -> Url {
    Url::parse(&site.url(path)).unwrap()
}

// spec: CRAWL-012
#[tokio::test]
async fn every_status_code_of_the_fixture_is_kept_unchanged() {
    let site = TestSite::start("crawl-status-codes").await.unwrap();
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    let expected: &[(u16, &str, StatusClass)] = &[
        (200, "OK", StatusClass::Success),
        (201, "Created", StatusClass::Success),
        (204, "No Content", StatusClass::Success),
        (206, "Partial Content", StatusClass::Success),
        (218, "", StatusClass::Success),
        (299, "", StatusClass::Success),
        (301, "Moved Permanently", StatusClass::Redirection),
        (302, "Found", StatusClass::Redirection),
        (303, "See Other", StatusClass::Redirection),
        (304, "Not Modified", StatusClass::Redirection),
        (307, "Temporary Redirect", StatusClass::Redirection),
        (308, "Permanent Redirect", StatusClass::Redirection),
        (400, "Bad Request", StatusClass::ClientError),
        (401, "Unauthorized", StatusClass::ClientError),
        (403, "Forbidden", StatusClass::ClientError),
        (404, "Not Found", StatusClass::ClientError),
        (410, "Gone", StatusClass::ClientError),
        (418, "", StatusClass::ClientError),
        (429, "Too Many Requests", StatusClass::ClientError),
        (
            451,
            "Unavailable For Legal Reasons",
            StatusClass::ClientError,
        ),
        (499, "", StatusClass::ClientError),
        (500, "Internal Server Error", StatusClass::ServerError),
        (502, "Bad Gateway", StatusClass::ServerError),
        (503, "Service Unavailable", StatusClass::ServerError),
        (504, "Gateway Timeout", StatusClass::ServerError),
        (520, "", StatusClass::ServerError),
        (600, "", StatusClass::Other),
        (999, "", StatusClass::Other),
    ];
    let mut by_class = std::collections::BTreeMap::<String, usize>::new();
    for (code, text, class) in expected {
        let resp = expect_response(get(&f, url(&site, &format!("/code/{code}"))).await);
        assert_eq!(resp.status, *code, "Statuscode {code} bleibt unverändert");
        assert_eq!(resp.status_text, *text, "Status zu {code}");
        assert_eq!(StatusClass::of(resp.status), *class, "Klasse zu {code}");
        *by_class
            .entry(format!("{:?}", StatusClass::of(resp.status)))
            .or_default() += 1;
        if matches!(code, 301 | 302 | 303 | 307 | 308) {
            assert_eq!(header(&resp, "location"), Some("/code/200"), "{code}");
        }
        if *code == 304 {
            assert_eq!(
                header(&resp, "location"),
                None,
                "304 erzeugt keine Weiterleitung"
            );
        }
    }
    // „Sonstige“ enthält genau die Codes außerhalb von 100–599 und nichts anderes
    assert_eq!(by_class.get("Other"), Some(&2));
    // jede Antwort gehört zu genau einer Klasse: Summe der Klassen = Zahl der Abrufe
    assert_eq!(by_class.values().sum::<usize>(), expected.len());
}

// spec: CRAWL-012
#[tokio::test]
async fn early_hints_from_the_fixture_are_skipped_and_the_final_200_is_stored() {
    let site = TestSite::start("crawl-status-codes").await.unwrap();
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    let resp = expect_response(get(&f, url(&site, "/early-hints")).await);
    assert_eq!(resp.status, 200, "gespeichert wird die finale Antwort");
    assert_eq!(resp.status_text, "OK");
    assert_eq!(StatusClass::of(resp.status), StatusClass::Success);
    assert!(
        String::from_utf8_lossy(&resp.body).contains("<title>Early Hints</title>"),
        "Body der 200-Antwort"
    );
    assert_eq!(
        header(&resp, "link"),
        None,
        "Header der 103-Antwort gehören nicht zur gespeicherten Antwort"
    );
    // Die 103-Antwort wurde wirklich gesendet (Zwischenantwort vor der finalen Antwort) …
    let log = site.requests();
    assert_eq!(log.len(), 1, "ein Abruf, keine Wiederholung wegen 1xx");
    assert!(log[0].sent_interim_bytes > 0, "{:?}", log[0]);
    // … und zählt nicht zu `Übertragen` der gespeicherten Antwort: nur die Body-Bytes der
    // 200-Antwort (K-02), weder Zwischenantwort noch Kopf
    for _ in 0..500 {
        if site.requests()[0].sent_bytes > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let sent = &site.requests()[0];
    assert_eq!(resp.transferred_bytes, sent.sent_body_bytes);
    assert!(
        resp.transferred_bytes < sent.sent_payload_bytes(),
        "{sent:?}"
    );
}

// spec: ONPAGE-020, CRAWL-019
#[tokio::test]
async fn content_type_fixture_routes_map_to_the_class_table() {
    use ContentClass::*;
    let site = TestSite::start("crawl-content-types").await.unwrap();
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    let expected: &[(&str, ContentClass)] = &[
        ("/t-html", Html),
        ("/t-xhtml", Html),
        ("/HTML-upper", Html),
        ("/t-js", JavaScript),
        ("/t-js-text", JavaScript),
        ("/t-js-x", JavaScript),
        ("/t-js-ecma", JavaScript),
        ("/t-js-text-ecma", JavaScript),
        ("/t-js-x-ecma", JavaScript),
        ("/t-js-text-x", JavaScript),
        ("/t-css", Css),
        ("/t-png", Images),
        ("/t-svg", Images),
        ("/t-webp", Images),
        ("/t-avif", Images),
        ("/t-pdf", Pdf),
        ("/t-swf", Plugins),
        ("/t-spl", Plugins),
        ("/t-mp4", Media),
        ("/t-mp3", Media),
        ("/t-woff2", Fonts),
        ("/t-woff", Fonts),
        ("/t-ttf", Fonts),
        ("/t-eot", Fonts),
        ("/t-xml", Xml),
        ("/t-xml-text", Xml),
        ("/t-rss", Xml),
        ("/t-atom", Xml),
        ("/t-json", Other),
        ("/t-text", Other),
        ("/t-zip", Other),
        ("/t-doc", Other),
        ("/falscher-ct", Other),
        ("/t-invalid", Unknown),
    ];
    for (path, class) in expected {
        let resp = expect_response(get(&f, url(&site, path)).await);
        assert_eq!(resp.status, 200, "{path}");
        let ct = header(&resp, "content-type");
        assert_eq!(
            content_class(ct, false),
            *class,
            "{path}: Content-Type {ct:?}"
        );
        // „Assume pages are HTML“ ändert nichts, solange ein Header da ist
        assert_eq!(content_class(ct, true), *class, "{path} mit assume_html");
    }
    // ein Wert mit Großbuchstaben und Parametern bleibt unverändert im Header (CRAWL-019)
    let upper = expect_response(get(&f, url(&site, "/HTML-upper")).await);
    assert_eq!(
        header(&upper, "content-type"),
        Some("Text/HTML; Charset=UTF-8")
    );
}

// spec: ONPAGE-020, CRAWL-019
#[tokio::test]
async fn response_without_content_type_is_unknown_or_html_when_assumed() {
    let server = MiniServer::start(|_| {
        Reply::Raw(response(
            200,
            "OK",
            &[],
            b"<html><body>ohne Typ</body></html>",
        ))
    })
    .await;
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    let resp = expect_response(get(&f, server.url("/kein-ct")).await);
    assert_eq!(header(&resp, "content-type"), None);
    assert_eq!(content_class(None, false), ContentClass::Unknown);
    assert_eq!(content_class(None, true), ContentClass::Html);
}

// spec: CRAWL-022
#[tokio::test]
async fn last_modified_header_is_passed_through_raw() {
    let site = TestSite::start("crawl-timing").await.unwrap();
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    let valid = expect_response(get(&f, url(&site, "/lm-valid")).await);
    assert_eq!(
        header(&valid, "last-modified"),
        Some("Wed, 21 Oct 2015 07:28:00 GMT")
    );
    // Ungültiges Datum: kein Fehler beim Abruf, der Rohwert bleibt (Auswertung macht die Engine)
    let invalid = expect_response(get(&f, url(&site, "/lm-invalid")).await);
    assert_eq!(invalid.status, 200);
    assert_eq!(header(&invalid, "last-modified"), Some("gestern"));
}

// spec: CRAWL-022
#[tokio::test]
async fn redirect_response_time_covers_only_its_own_request() {
    let site = TestSite::start("crawl-timing").await.unwrap();
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    // /r301 → /slow-800: die Zeile /r301 misst nur den eigenen Request (< 100 ms), nicht die Kette
    let resp = expect_response(get(&f, url(&site, "/r301")).await);
    assert_eq!(resp.status, 301);
    assert_eq!(header(&resp, "location"), Some("/slow-800"));
    assert!(resp.response_time_ms < 100, "{} ms", resp.response_time_ms);
}

// spec: CRAWL-022
// Wanduhr-Messung (Regel 2.8): läuft als Benchmark über P28.
#[ignore = "Benchmark: Wanduhr-Messung (Regel 2.8), P28"]
#[tokio::test]
async fn slow_route_response_time_is_between_800_and_1000_ms() {
    let site = TestSite::start("crawl-timing").await.unwrap();
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    let resp = expect_response(get(&f, url(&site, "/slow-800")).await);
    assert!(
        (800..=1000).contains(&resp.response_time_ms),
        "{} ms",
        resp.response_time_ms
    );
}

// spec: CRAWL-095
// Wanduhr-Messung (Regel 2.8): läuft als Benchmark über P28.
#[ignore = "Benchmark: Wanduhr-Messung (Regel 2.8), P28"]
#[tokio::test]
async fn timeout_two_seconds_against_three_second_route() {
    let site = TestSite::start("crawl-timing").await.unwrap();
    let f = fetcher_with(Arc::new(SystemClock), |c| {
        c.response_timeout = Duration::from_secs(2)
    });
    let (reason, _, elapsed_ms) = expect_no_response(get(&f, url(&site, "/slow-3000")).await);
    assert_eq!(reason, NoResponseReason::ResponseTimeout);
    assert!((2000..=2500).contains(&elapsed_ms), "nach {elapsed_ms} ms");
    let ok = expect_response(get(&f, url(&site, "/slow-1000")).await);
    assert_eq!(ok.status, 200);
}

// spec: CRAWL-095, CRAWL-013
#[tokio::test]
async fn hanging_fixture_route_ends_in_a_response_timeout() {
    let site = TestSite::start("crawl-no-response").await.unwrap();
    let f = fetcher_with(Arc::new(SystemClock), |c| {
        c.response_timeout = Duration::from_millis(400)
    });
    for path in ["/hang-response", "/timeout"] {
        let (reason, _, _) = expect_no_response(get(&f, url(&site, path)).await);
        assert_eq!(reason, NoResponseReason::ResponseTimeout, "{path}");
        assert_eq!(reason.status_text(), "Connection Timeout");
    }
}

// spec: CRAWL-011
#[tokio::test]
async fn plain_fixture_page_is_fetched_with_body_hash_and_counted_bytes() {
    let site = TestSite::start("crawl-http-basic").await.unwrap();
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    let resp = expect_response(f.fetch(page_request(url(&site, "/plain"))).await);
    assert_eq!(resp.status, 200);
    let body = String::from_utf8(resp.body.to_vec()).unwrap();
    assert!(body.contains("<title>Einfache Seite</title>"));
    assert_eq!(resp.size_bytes, body.len() as u64);
    assert_eq!(
        resp.transferred_bytes, resp.size_bytes,
        "unkomprimiert: Transferred = Size, der Kopf kommt nicht hinzu (K-02)"
    );
    assert_eq!(resp.md5_hex.len(), 32);
    assert!(!resp.truncated);
}
