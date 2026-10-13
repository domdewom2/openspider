//! Fixture-Tests gegen `os-testsite`, die das Request-Log der Testsite als Gegenstelle nutzen:
//! TLS-SNI-Name je Verbindung (CRAWL-123) und gesendete Bytes je Antwort (CRAWL-022).
//!
//! Fixtures: `tests/fixtures/{crawl-status-codes,crawl-http-basic}`.

#[path = "http_support.rs"]
mod support;

use std::sync::Arc;
use std::time::Duration;

use os_model::SystemClock;
use os_testsite::{RequestRecord, TestSite};
use support::*;
use url::Url;

/// Wartet, bis der Server die gesendeten Bytes von `n` Requests verbucht hat (er tut das nach dem
/// Schreiben der Antwort; der Client kann schneller sein).
async fn settled(site: &TestSite, n: usize) -> Vec<RequestRecord> {
    for _ in 0..500 {
        let log = site.requests();
        if log.len() == n && log.iter().all(|r| r.sent_bytes > 0) {
            return log;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("Log nicht vollständig verbucht: {:?}", site.requests());
}

fn tls_fetcher(site: &TestSite) -> os_fetch::HttpFetcher {
    let port = site.port();
    let pem = site.ca_cert_pem().into_bytes();
    fetcher_with(Arc::new(SystemClock), |c| {
        c.resolve = vec![format!("*.site.test=127.0.0.1:{port}")];
        c.extra_root_certs = vec![pem];
    })
}

// spec: CRAWL-123
#[tokio::test]
async fn tls_sni_is_the_host_of_the_url_not_the_resolved_address() {
    let site = TestSite::start("crawl-status-codes").await.unwrap();
    let f = tls_fetcher(&site);
    for host in ["www.site.test", "blog.site.test", "deep.sub.site.test"] {
        let resp = expect_response(
            get(&f, Url::parse(&format!("https://{host}/code/200")).unwrap()).await,
        );
        assert_eq!(resp.status, 200, "{host}");
    }
    let log = settled(&site, 3).await;
    for (rec, host) in log
        .iter()
        .zip(["www.site.test", "blog.site.test", "deep.sub.site.test"])
    {
        assert!(rec.tls, "{host}");
        assert_eq!(rec.sni.as_deref(), Some(host), "SNI = Host der URL");
        assert_eq!(rec.host, host, "Host-Header = Host der URL");
    }
}

// spec: CRAWL-123
#[tokio::test]
async fn plain_http_and_ip_urls_carry_no_sni() {
    let site = TestSite::start("crawl-status-codes").await.unwrap();
    let pem = site.ca_cert_pem().into_bytes();
    let f = fetcher_with(Arc::new(SystemClock), |c| c.extra_root_certs = vec![pem]);
    let http = expect_response(get(&f, Url::parse(&site.url("/code/200")).unwrap()).await);
    assert_eq!(http.status, 200);
    let https = format!("{}/code/200", site.https_base_url());
    let ip = expect_response(get(&f, Url::parse(&https).unwrap()).await);
    assert_eq!(ip.status, 200);
    let log = settled(&site, 2).await;
    assert_eq!((log[0].tls, log[0].sni.clone()), (false, None));
    assert_eq!(
        (log[1].tls, log[1].sni.clone()),
        (true, None),
        "Zieladresse ist eine IP: der Client sendet keinen SNI"
    );
}

// spec: CRAWL-022
#[tokio::test]
async fn transferred_equals_the_bytes_the_server_sent_for_content_length_responses() {
    let site = TestSite::start("crawl-status-codes").await.unwrap();
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    let resp = expect_response(get(&f, Url::parse(&site.url("/code/200")).unwrap()).await);
    let log = settled(&site, 1).await;
    let sent = &log[0];
    assert_eq!(
        sent.sent_framing_bytes, 0,
        "Content-Length: kein Chunk-Rahmen"
    );
    assert_eq!(sent.sent_interim_bytes, 0);
    assert_eq!(
        resp.transferred_bytes, sent.sent_bytes,
        "Übertragen = Kopf + Body, Abweichung zum Byte-Zähler des Servers: 0"
    );
    assert!(resp.transferred_bytes > resp.size_bytes, "Kopf kommt hinzu");

    // Auch eine Antwort ohne Body (204) zählt nur den Kopf
    let no_body = expect_response(get(&f, Url::parse(&site.url("/code/204")).unwrap()).await);
    let log = settled(&site, 2).await;
    assert_eq!(no_body.size_bytes, 0);
    assert_eq!(no_body.transferred_bytes, log[1].sent_bytes);
}

// spec: CRAWL-022
#[tokio::test]
async fn chunked_responses_are_counted_without_chunk_framing() {
    // Dokumentierte Abweichung: `Übertragen` zählt bei Transfer-Encoding: chunked Kopf + Body ohne
    // Chunk-Rahmen (der HTTP-Stack liefert nur die Nutzdaten); der Server schreibt die Rahmenbytes
    // zusätzlich. Die Differenz zum Byte-Zähler des Servers ist genau `sent_framing_bytes`.
    let site = TestSite::start("crawl-http-basic").await.unwrap();
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    let resp = expect_response(get(&f, Url::parse(&site.url("/chunked")).unwrap()).await);
    assert_eq!(resp.status, 200);
    let log = settled(&site, 1).await;
    let sent = &log[0];
    assert!(sent.sent_framing_bytes > 0, "{sent:?}");
    assert_eq!(
        resp.transferred_bytes,
        sent.sent_bytes - sent.sent_framing_bytes,
        "ohne Rahmenbytes: {sent:?}"
    );
    assert_eq!(resp.transferred_bytes, sent.sent_payload_bytes());
    assert!(resp.transferred_bytes < sent.sent_bytes);
}
