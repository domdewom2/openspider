//! Fixture-Tests, die auf Fähigkeiten des Testservers aus M1-P09 warten (Kompression, Chunked,
//! Echo der Request-Header, Cookies, virtuelle Hosts mit Basic-Auth, Verbindungsabbrüche, TLS).
//!
//! Scharfgeschaltet in Welle 1.5 (M1.md 2a, E6): die Szenarien `tests/fixtures/crawl-*` liefern die
//! benötigten Routen im Format von P09. Dieselben Verhalten prüfen `http_fetch.rs` und
//! `http_session.rs` zusätzlich gegen einen eigenen Mini-Server.
//!
//! Nicht über den Testserver abbildbar (siehe Doku von `os-testsite`): 103 Early Hints (Zwischenantworten)
//! und ein Blackhole-Port für Verbindungs-Timeouts. Beides ist in `http_fetch.rs` abgedeckt
//! (`informational_responses_are_skipped`, `connect_timeout_is_the_smaller_of_ten_seconds_and_the_response_timeout`).

#[path = "http_support.rs"]
mod support;

use std::sync::Arc;
use std::time::Duration;

use os_fetch::{content_class, Fetch, FetchRequest};
use os_model::{ContentClass, CookieMode, NoResponseReason, SystemClock};
use os_testsite::TestSite;
use support::*;
use url::Url;

fn url(site: &TestSite, path: &str) -> Url {
    Url::parse(&site.url(path)).unwrap()
}

fn text(resp: &os_fetch::HttpResponse) -> String {
    String::from_utf8_lossy(&resp.body).into_owned()
}

// spec: CRAWL-011, CRAWL-114
#[tokio::test]
async fn echo_headers_shows_the_exact_user_agent() {
    let site = TestSite::start("crawl-http-basic").await.unwrap();
    let ua = "Mozilla/5.0 (compatible; Fixture-UA/1.0)";
    let f = fetcher_with(Arc::new(SystemClock), |c| c.user_agent = ua.to_string());
    let resp = expect_response(get(&f, url(&site, "/echo-headers")).await);
    let echoed = text(&resp);
    assert!(echoed.contains(ua), "{echoed}");
    assert!(echoed.contains("gzip, deflate, br"), "{echoed}");
}

// spec: CRAWL-011
#[tokio::test]
async fn compressed_and_chunked_fixture_routes_decode_to_the_same_html() {
    let site = TestSite::start("crawl-http-basic").await.unwrap();
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    let plain = expect_response(get(&f, url(&site, "/plain")).await);
    for path in ["/gzip", "/br", "/deflate-raw", "/chunked"] {
        let resp = expect_response(get(&f, url(&site, path)).await);
        assert_eq!(resp.status, 200, "{path}");
        assert_eq!(resp.decode_error, None, "{path}");
        assert_eq!(resp.md5_hex, plain.md5_hex, "{path}");
        assert_eq!(resp.size_bytes, plain.size_bytes, "{path}");
    }
    // Header gzip, Body unkomprimiert: Statuscode bleibt, Status „Connection Error“
    let broken = expect_response(get(&f, url(&site, "/broken-gzip")).await);
    assert_eq!(broken.status, 200);
    assert_eq!(broken.status_text, "Connection Error");
    assert!(broken.decode_error.is_some());
}

// spec: CRAWL-019, ONPAGE-020
#[tokio::test]
async fn route_without_content_type_is_unknown_or_html_when_assumed() {
    let site = TestSite::start("crawl-content-types").await.unwrap();
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    let resp = expect_response(get(&f, url(&site, "/kein-ct")).await);
    let ct = header(&resp, "content-type");
    assert_eq!(ct, None);
    assert_eq!(content_class(ct, false), ContentClass::Unknown);
    assert_eq!(content_class(ct, true), ContentClass::Html);
}

// spec: CRAWL-013, ONPAGE-065
#[tokio::test]
async fn fixture_no_response_routes_have_the_expected_causes() {
    let site = TestSite::start("crawl-no-response").await.unwrap();
    let f = fetcher_with(Arc::new(SystemClock), |c| {
        c.response_timeout = Duration::from_millis(500)
    });
    // Verbindung wird nach halbem Body bzw. nach den Headern abgebrochen
    for path in ["/reset-mid-body", "/drop"] {
        let (reason, _, _) = expect_no_response(get(&f, url(&site, path)).await);
        assert_eq!(reason, NoResponseReason::ConnectionError, "{path}");
    }
}

// spec: CRAWL-013, ONPAGE-065
#[tokio::test]
async fn expired_certificate_is_a_tls_error() {
    let site = TestSite::start("crawl-no-response").await.unwrap();
    let port = site.addr().port();
    let f = fetcher_with(Arc::new(SystemClock), |c| {
        c.resolve = vec![format!("*.site.test=127.0.0.1:{port}")];
    });
    let (reason, detail, _) =
        expect_no_response(get(&f, Url::parse("https://expired.site.test/").unwrap()).await);
    assert_eq!(reason, NoResponseReason::TlsError, "{detail}");
}

// spec: CRAWL-088
#[tokio::test]
async fn cookie_fixture_set_check_flow_follows_the_three_modes() {
    let site = TestSite::start("crawl-cookies").await.unwrap();
    let make = |mode: CookieMode| fetcher_with(Arc::new(SystemClock), move |c| c.cookies = mode);
    let with = |jar| FetchRequest {
        cookies: jar,
        ..page_request(url(&site, "/check"))
    };
    // Nur Sitzung: /check sieht das Cookie, ein separater Abruf von /check2 nicht
    let f = make(CookieMode::SessionOnly);
    let jar = f.cookie_jar_for_new_seed();
    let _ = expect_response(
        f.fetch(FetchRequest {
            cookies: jar.clone(),
            ..page_request(url(&site, "/set"))
        })
        .await,
    );
    assert!(text(&expect_response(f.fetch(with(jar)).await)).contains("sid=abc123"));
    let separate = FetchRequest {
        cookies: f.cookie_jar_for_new_seed(),
        ..page_request(url(&site, "/check2"))
    };
    assert!(!text(&expect_response(f.fetch(separate).await)).contains("sid="));
    // Nicht speichern: /check bleibt leer
    let f = make(CookieMode::None);
    let _ = expect_response(f.fetch(page_request(url(&site, "/set"))).await);
    assert!(!text(&expect_response(
        f.fetch(page_request(url(&site, "/check"))).await
    ))
    .contains("sid="));
}

// spec: CRAWL-117
#[tokio::test]
async fn basic_auth_fixture_host_accepts_the_stored_credentials_only_for_itself() {
    let site = TestSite::start("crawl-auth").await.unwrap();
    let port = site.addr().port();
    let f = fetcher_with(Arc::new(SystemClock), |c| {
        c.resolve = vec![
            format!("*.site.test=127.0.0.1:{port}"),
            format!("other.test=127.0.0.1:{port}"),
        ];
        c.basic_auth = vec![os_fetch::BasicAuthEntry {
            prefix: "http://basic.site.test/".to_string(),
            username: "prüfer".to_string(),
            password: "geheim".to_string(),
        }];
    });
    let own = expect_response(get(&f, Url::parse("http://basic.site.test/").unwrap()).await);
    assert_eq!(own.status, 200);
    // Der Abruf kommt mit Basic-Authorization (UTF-8, prüfer:geheim) am Host an
    let sent = text(&own);
    assert!(sent.contains("Basic cHLDvGZlcjpnZWhlaW0="), "{sent}");
    // Weiterleitung von dort auf other.test/echo-headers: dort kommt kein Authorization-Header an
    let redirect =
        expect_response(get(&f, Url::parse("http://basic.site.test/to-other").unwrap()).await);
    let target = Url::parse(header(&redirect, "location").unwrap()).unwrap();
    let echoed = text(&expect_response(get(&f, target).await));
    assert!(
        !echoed.to_ascii_lowercase().contains("authorization"),
        "{echoed}"
    );
}
