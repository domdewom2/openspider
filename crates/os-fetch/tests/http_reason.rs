//! Reason-Phrase der Statuszeile (CRAWL-012): die vom Server gesendete Phrase wird übernommen,
//! sonst gilt der IANA-Text, für Codes ohne Eintrag und ohne Phrase bleibt der Anzeigetext leer.
//!
//! Eigenes Szenario in einem `tempfile`-Verzeichnis (Route-Option `reason` des Testservers).

#[path = "http_support.rs"]
mod support;

use std::sync::Arc;

use os_model::SystemClock;
use os_testsite::{SiteOptions, TestSite};
use support::*;
use url::Url;

const SCENARIO: &str = r#"
[[route]]
path = "/fine"
status = 218
reason = "This Is Fine"
body = "<html><body>fine</body></html>"

[[route]]
path = "/plain-ok"
status = 200
body = "<html><body>ok</body></html>"

[[route]]
path = "/plain-299"
status = 299
body = "<html><body>299</body></html>"
"#;

async fn start(dir: &tempfile::TempDir) -> TestSite {
    std::fs::write(dir.path().join("scenario.toml"), SCENARIO).expect("scenario.toml");
    TestSite::start_dir_with(
        dir.path(),
        SiteOptions {
            port: 0,
            log_requests: false,
        },
    )
    .await
    .expect("Testserver")
}

fn url(site: &TestSite, path: &str) -> Url {
    Url::parse(&site.url(path)).unwrap()
}

// spec: CRAWL-012
#[tokio::test]
async fn server_reason_phrase_is_shown_iana_text_is_the_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let site = start(&dir).await;
    let f = fetcher_with(Arc::new(SystemClock), |_| {});

    // Eigene Phrase zu einem Code, den die IANA-Registry nicht kennt.
    let r = expect_response(get(&f, url(&site, "/fine")).await);
    assert_eq!(r.status, 218);
    assert_eq!(r.reason_phrase.as_deref(), Some("This Is Fine"));
    assert_eq!(r.status_text, "This Is Fine");

    // 200 ohne eigene Phrase: IANA-Text.
    let r = expect_response(get(&f, url(&site, "/plain-ok")).await);
    assert_eq!(r.status, 200);
    assert_eq!(r.reason_phrase, None);
    assert_eq!(r.status_text, "OK");

    // 299 ohne Phrase und ohne IANA-Eintrag: leer.
    let r = expect_response(get(&f, url(&site, "/plain-299")).await);
    assert_eq!(r.status, 299);
    assert_eq!(r.reason_phrase, None);
    assert_eq!(r.status_text, "");
}

// spec: CRAWL-012
#[tokio::test]
async fn non_ascii_reason_phrase_falls_back_to_the_iana_text() {
    // Der Testserver sendet nur ASCII-Phrasen; für Nicht-ASCII-Bytes steuert der Mini-Server die Statuszeile.
    // httparse (unter hyper) liefert für Phrasen mit obs-text bewusst eine leere Phrase; damit gilt der IANA-Text.
    let server = MiniServer::start(|_| {
        Reply::Raw(response(
            200,
            "Alles in Ordnung ü",
            &[("content-type", "text/plain")],
            b"x",
        ))
    })
    .await;
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    let r = expect_response(get(&f, server.url("/")).await);
    assert_eq!(r.status, 200);
    assert_eq!(r.reason_phrase, None);
    assert_eq!(r.status_text, "OK");
}
