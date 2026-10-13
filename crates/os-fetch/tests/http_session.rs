//! Cookies (CRAWL-088), Basic-Auth (CRAWL-117), Geheimnisse und Protokoll-Redaktion
//! (OUT-225, OUT-243) gegen den Mini-Server.

#[path = "http_support.rs"]
mod support;

use std::fmt::Write as _;
use std::sync::{Arc, Mutex};

use os_fetch::{
    BasicAuthEntry, CookieJar, Fetch, FetchConfig, FetchRequest, HttpFetcher, MemorySecretStore,
};
use os_model::{BasicAuth, CookieMode, CrawlConfig, SystemClock};
use support::*;
use url::Url;

fn body_text(resp: &os_fetch::HttpResponse) -> String {
    String::from_utf8_lossy(&resp.body).into_owned()
}

fn with_jar(url: Url, jar: Option<Arc<CookieJar>>) -> FetchRequest {
    FetchRequest {
        cookies: jar,
        ..page_request(url)
    }
}

/// `/set` setzt ein Cookie und leitet auf `/check` weiter; `/check*` zeigen die empfangenen Cookies.
fn cookie_server_handler(req: &Request) -> Reply {
    match req.path() {
        "/set" => Reply::Raw(response(
            302,
            "Found",
            &[("location", "/check"), ("set-cookie", "sid=abc123; Path=/")],
            b"",
        )),
        "/set-secure" => Reply::Raw(response(
            200,
            "OK",
            &[("set-cookie", "tls=geheim; Secure; Path=/")],
            b"gesetzt",
        )),
        _ => ok_text(&format!("cookie={}", req.header("cookie").unwrap_or("-"))),
    }
}

// spec: CRAWL-088
#[tokio::test]
async fn session_only_cookie_survives_the_redirect_but_not_a_separate_fetch() {
    let server = MiniServer::start(cookie_server_handler).await;
    let f = fetcher_with(Arc::new(SystemClock), |c| {
        c.cookies = CookieMode::SessionOnly
    });
    // Die Engine legt den Speicher je Seed an und reicht ihn an das Weiterleitungsziel weiter.
    let jar = f
        .cookie_jar_for_new_seed()
        .expect("Speicher im Modus SessionOnly");
    let set = expect_response(
        f.fetch(with_jar(server.url("/set"), Some(jar.clone())))
            .await,
    );
    assert_eq!(set.status, 302);
    assert_eq!(header(&set, "location"), Some("/check"));
    let check = expect_response(f.fetch(with_jar(server.url("/check"), Some(jar))).await);
    assert_eq!(
        body_text(&check),
        "cookie=sid=abc123",
        "/check erhält das Cookie"
    );

    // späterer, separater Abruf (neuer Seed, neuer Speicher): kein Cookie
    let other_seed = f.cookie_jar_for_new_seed();
    let check2 = expect_response(f.fetch(with_jar(server.url("/check2"), other_seed)).await);
    assert_eq!(body_text(&check2), "cookie=-");
    // ohne Speicher in der Anfrage ebenfalls nicht
    let bare = expect_response(f.fetch(with_jar(server.url("/check2"), None)).await);
    assert_eq!(body_text(&bare), "cookie=-");
}

// spec: CRAWL-088
#[tokio::test]
async fn persistent_cookies_are_shared_across_fetches_and_threads() {
    let server = MiniServer::start(cookie_server_handler).await;
    let f = Arc::new(fetcher_with(Arc::new(SystemClock), |c| {
        c.cookies = CookieMode::Persistent
    }));
    let set = expect_response(
        f.fetch(with_jar(server.url("/set"), f.cookie_jar_for_new_seed()))
            .await,
    );
    assert_eq!(set.status, 302);
    // /check2 erhält das Cookie, auch ohne mitgegebenen Speicher und aus einem anderen Thread
    let f2 = f.clone();
    let url = server.url("/check2");
    let other_thread = tokio::spawn(async move { f2.fetch(with_jar(url, None)).await });
    let resp = expect_response(other_thread.await.unwrap());
    assert_eq!(body_text(&resp), "cookie=sid=abc123");
    // und ein zweiter „Seed“ teilt denselben Speicher
    let shared = f.cookie_jar_for_new_seed().unwrap();
    assert_eq!(shared.len(), 1);
}

// spec: CRAWL-088
#[tokio::test]
async fn cookie_mode_none_never_sends_or_stores_cookies() {
    let server = MiniServer::start(cookie_server_handler).await;
    let f = fetcher_with(Arc::new(SystemClock), |c| c.cookies = CookieMode::None);
    assert!(f.cookie_jar_for_new_seed().is_none());
    // selbst ein vorbefüllter Speicher in der Anfrage wird ignoriert
    let jar = Arc::new(CookieJar::new());
    assert!(jar.insert_raw(&server.url("/"), "alt=1; Path=/"));
    let _ = expect_response(
        f.fetch(with_jar(server.url("/set"), Some(jar.clone())))
            .await,
    );
    let check = expect_response(
        f.fetch(with_jar(server.url("/check"), Some(jar.clone())))
            .await,
    );
    assert_eq!(body_text(&check), "cookie=-", "/check erhält kein Cookie");
    assert_eq!(jar.len(), 1, "es wurde auch nichts hineingeschrieben");
}

// spec: CRAWL-088
#[tokio::test]
async fn secure_cookie_is_not_sent_over_plain_http() {
    let server = MiniServer::start(cookie_server_handler).await;
    let port = server.addr.port();
    // Loopback-Adressen gelten als „sicherer Kontext“; ein Fixture-Hostname nicht.
    let f = fetcher_with(Arc::new(SystemClock), |c| {
        c.cookies = CookieMode::SessionOnly;
        c.resolve = vec![format!("*.site.test=127.0.0.1:{port}")];
    });
    let jar = f.cookie_jar_for_new_seed().unwrap();
    let base = |path: &str| Url::parse(&format!("http://www.site.test{path}")).unwrap();
    let _ = expect_response(
        f.fetch(with_jar(base("/set-secure"), Some(jar.clone())))
            .await,
    );
    let resp = expect_response(f.fetch(with_jar(base("/secure-only"), Some(jar))).await);
    assert_eq!(body_text(&resp), "cookie=-");
}

// spec: CRAWL-088
#[tokio::test]
async fn a_new_crawl_starts_with_an_empty_cookie_store() {
    let f = fetcher_with(Arc::new(SystemClock), |c| {
        c.cookies = CookieMode::SessionOnly
    });
    assert!(f.cookie_jar_for_new_seed().unwrap().is_empty());
    let f = fetcher_with(Arc::new(SystemClock), |c| {
        c.cookies = CookieMode::Persistent
    });
    assert!(f.cookie_jar_for_new_seed().unwrap().is_empty());
}

fn auth_server_handler(expected: &'static str) -> impl Fn(&Request) -> Reply + Send + Sync {
    move |req: &Request| match req.path() {
        "/protected" => match req.header("authorization") {
            Some(a) if a == expected => ok_text("geschützter Inhalt"),
            _ => Reply::Raw(response(
                401,
                "Unauthorized",
                &[("www-authenticate", "Basic realm=\"Test\"")],
                b"",
            )),
        },
        "/to-other" => Reply::Raw(response(
            302,
            "Found",
            &[("location", "http://other.test/echo-auth")],
            b"",
        )),
        _ => ok_text(&format!(
            "auth={}",
            req.header("authorization").unwrap_or("-")
        )),
    }
}

/// `Basic base64("prüfer:geheim")` – UTF-8, nicht Latin-1.
const PRUEFER_GEHEIM: &str = "Basic cHLDvGZlcjpnZWhlaW0=";

fn auth_entry(prefix: String) -> BasicAuthEntry {
    BasicAuthEntry {
        prefix,
        username: "prüfer".to_string(),
        password: "geheim".to_string(),
    }
}

// spec: CRAWL-117
#[tokio::test]
async fn stored_credentials_unlock_the_protected_page_with_utf8_encoding() {
    let server = MiniServer::start(auth_server_handler(PRUEFER_GEHEIM)).await;
    let port = server.addr.port();
    let f = fetcher_with(Arc::new(SystemClock), |c| {
        c.basic_auth = vec![auth_entry(format!("http://127.0.0.1:{port}/"))];
    });
    let resp = expect_response(get(&f, server.url("/protected")).await);
    assert_eq!(resp.status, 200);
    assert_eq!(body_text(&resp), "geschützter Inhalt");
}

// spec: CRAWL-117
#[tokio::test]
async fn without_credentials_the_401_is_stored_and_nothing_blocks() {
    let server = MiniServer::start(auth_server_handler(PRUEFER_GEHEIM)).await;
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    let resp = expect_response(get(&f, server.url("/protected")).await);
    assert_eq!(resp.status, 401);
    assert_eq!(resp.status_text, "Unauthorized");
    assert_eq!(
        header(&resp, "www-authenticate"),
        Some("Basic realm=\"Test\"")
    );
}

// spec: CRAWL-117
#[tokio::test]
async fn credentials_never_go_to_other_hosts_or_after_a_redirect_to_a_foreign_host() {
    let server = MiniServer::start(auth_server_handler(PRUEFER_GEHEIM)).await;
    let port = server.addr.port();
    let f = fetcher_with(Arc::new(SystemClock), |c| {
        c.basic_auth = vec![auth_entry(format!("http://127.0.0.1:{port}/"))];
        c.resolve = vec![format!("other.test=127.0.0.1:{port}")];
    });
    // Der passende Host erhält den Header ...
    let own = expect_response(get(&f, server.url("/echo-auth")).await);
    assert_eq!(body_text(&own), format!("auth={PRUEFER_GEHEIM}"));
    // ... die Weiterleitung führt auf einen fremden Host (dieselbe IP, anderer Name) ...
    let redirect = expect_response(get(&f, server.url("/to-other")).await);
    assert_eq!(redirect.status, 302);
    let target = Url::parse(header(&redirect, "location").unwrap()).unwrap();
    // ... und dort kommt nichts an.
    let foreign = expect_response(get(&f, target).await);
    assert_eq!(body_text(&foreign), "auth=-");
}

// spec: CRAWL-117
#[tokio::test]
async fn plain_http_needs_the_explicit_http_prefix_as_confirmation() {
    let server = MiniServer::start(auth_server_handler(PRUEFER_GEHEIM)).await;
    let port = server.addr.port();
    // Präfix ohne Schema bzw. mit https gilt nur für https: über http geht nichts raus.
    for prefix in [
        "127.0.0.1",
        &format!("127.0.0.1:{port}"),
        &format!("https://127.0.0.1:{port}/"),
    ] {
        let f = fetcher_with(Arc::new(SystemClock), |c| {
            c.basic_auth = vec![auth_entry(prefix.to_string())]
        });
        let resp = expect_response(get(&f, server.url("/echo-auth")).await);
        assert_eq!(body_text(&resp), "auth=-", "Präfix {prefix}");
    }
}

// spec: CRAWL-117
#[tokio::test]
async fn credentials_in_the_url_are_not_used() {
    let server = MiniServer::start(auth_server_handler(PRUEFER_GEHEIM)).await;
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    let url = Url::parse(&format!(
        "http://wer:wasauchimmer@127.0.0.1:{}/echo-auth",
        server.addr.port()
    ))
    .unwrap();
    let resp = expect_response(get(&f, url).await);
    assert_eq!(
        body_text(&resp),
        "auth=-",
        "Zugangsdaten stammen nur aus der Konfiguration"
    );
}

// spec: CRAWL-117, OUT-225
#[tokio::test]
async fn password_comes_from_the_secret_store_and_a_missing_entry_is_reported_clearly() {
    let server = MiniServer::start(auth_server_handler(PRUEFER_GEHEIM)).await;
    let port = server.addr.port();
    let mut cfg = CrawlConfig::default();
    cfg.auth.basic = vec![BasicAuth {
        prefix: format!("http://127.0.0.1:{port}/"),
        username: "prüfer".to_string(),
        secret_ref: "basic-test".to_string(),
    }];
    let secrets = MemorySecretStore::new();
    secrets.set("basic-test", "geheim");
    let fc = FetchConfig::from_crawl_config(&cfg, &secrets, Arc::new(SystemClock)).unwrap();
    let f = HttpFetcher::new(fc).unwrap();
    let resp = expect_response(get(&f, server.url("/protected")).await);
    assert_eq!(resp.status, 200);

    // Eintrag fehlt (anderer Rechner): verständlicher Konfigurationsfehler, kein Absturz
    let err =
        FetchConfig::from_crawl_config(&cfg, &MemorySecretStore::new(), Arc::new(SystemClock))
            .unwrap_err()
            .to_string();
    assert!(
        err.contains("nicht verbunden") && err.contains("basic-test"),
        "{err}"
    );
}

// spec: OUT-225
#[test]
fn config_file_and_debug_output_contain_no_password() {
    let mut cfg = CrawlConfig::default();
    cfg.auth.basic = vec![BasicAuth {
        prefix: "https://intern.example.org/".to_string(),
        username: "admin".to_string(),
        secret_ref: "basic-intern".to_string(),
    }];
    let secrets = MemorySecretStore::new();
    secrets.set("basic-intern", "KlartextPasswort-4711");
    let fc = FetchConfig::from_crawl_config(&cfg, &secrets, Arc::new(SystemClock)).unwrap();
    // Konfigurationsdatei: nur die Kennung
    let toml = cfg.to_toml_string().unwrap();
    assert!(toml.contains("basic-intern"));
    assert!(!toml.contains("KlartextPasswort-4711"));
    // Debug-Ausgaben von Konfiguration, Abrufer und Secret-Store
    let f = HttpFetcher::new(fc.clone()).unwrap();
    for dbg in [
        format!("{fc:?}"),
        format!("{f:?}"),
        format!("{secrets:?}"),
        format!("{:?}", fc.basic_auth),
    ] {
        assert!(!dbg.contains("KlartextPasswort-4711"), "{dbg}");
    }
}

/// Sammelt alle Ereignisse (jede Stufe) als Text.
struct Capture(Arc<Mutex<String>>);

impl tracing::Subscriber for Capture {
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        struct V<'a>(&'a mut String);
        impl tracing::field::Visit for V<'_> {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                let _ = write!(self.0, "{}={:?} ", field.name(), value);
            }
        }
        let mut line = format!("{} ", event.metadata().target());
        event.record(&mut V(&mut line));
        let mut log = self.0.lock().unwrap_or_else(|e| e.into_inner());
        log.push_str(&line);
        log.push('\n');
    }
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
}

// spec: OUT-243, OUT-225
#[tokio::test]
async fn no_secret_reaches_the_logs_or_the_stored_headers() {
    const PASSWORD: &str = "BasicPW-S3cret-4711";
    const COOKIE_OUT: &str = "SessionCookie-9f8e7d";
    const COOKIE_IN: &str = "ServerCookie-1a2b3c";
    const TOKEN: &str = "QueryToken-55aa66";
    const API_KEY: &str = "ApiKey-77bb88";
    const USERINFO_PW: &str = "UserinfoPW-99cc00";

    let log = Arc::new(Mutex::new(String::new()));
    let _guard = tracing::subscriber::set_default(Capture(log.clone()));

    let server = MiniServer::start(|_| {
        Reply::Raw(response(
            200,
            "OK",
            &[
                ("content-type", "text/html"),
                ("set-cookie", &format!("srv={COOKIE_IN}; Path=/")),
            ],
            b"ok",
        ))
    })
    .await;
    let port = server.addr.port();

    let mut cfg = CrawlConfig::default();
    cfg.auth.basic = vec![BasicAuth {
        prefix: format!("http://127.0.0.1:{port}/"),
        username: "admin".to_string(),
        secret_ref: "pw".to_string(),
    }];
    cfg.advanced.cookies = CookieMode::SessionOnly;
    let secrets = MemorySecretStore::new();
    secrets.set("pw", PASSWORD);
    let fc = FetchConfig::from_crawl_config(&cfg, &secrets, Arc::new(SystemClock)).unwrap();
    let f = HttpFetcher::new(fc).unwrap();

    let jar = f.cookie_jar_for_new_seed().unwrap();
    assert!(jar.insert_raw(&server.url("/"), &format!("sid={COOKIE_OUT}; Path=/")));
    let url = Url::parse(&format!(
        "http://wer:{USERINFO_PW}@127.0.0.1:{port}/seite?token={TOKEN}&api_key={API_KEY}&a=1"
    ))
    .unwrap();
    let resp = expect_response(f.fetch(with_jar(url, Some(jar.clone()))).await);
    // die Anfrage trug wirklich Zugangsdaten und Cookie (sonst wäre der Test wertlos)
    let sent = server.last_request("/seite").unwrap();
    assert!(sent.header("authorization").is_some());
    assert!(sent.header("cookie").unwrap().contains(COOKIE_OUT));

    // ein Fehlschlag mit denselben Geheimnissen in der Adresse (geschlossener Port)
    let closed = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let closed_port = closed.local_addr().unwrap().port();
    drop(closed);
    let bad = Url::parse(&format!(
        "http://wer:{USERINFO_PW}@127.0.0.1:{closed_port}/x?token={TOKEN}&api_key={API_KEY}"
    ))
    .unwrap();
    let (_, detail, _) = expect_no_response(f.fetch(with_jar(bad, Some(jar))).await);

    // gespeicherte Antwort-Header: Set-Cookie ist geschwärzt
    assert_eq!(header(&resp, "set-cookie"), Some("<redacted>"));
    assert_eq!(header(&resp, "content-type"), Some("text/html"));

    let logs = log.lock().unwrap_or_else(|e| e.into_inner()).clone();
    assert!(
        logs.contains("Abruf"),
        "Der Test braucht Protokollzeilen: {logs:?}"
    );
    assert!(logs.contains("<redacted>"), "{logs}");
    let everything = format!("{logs}\n{detail}\n{resp:?}\n{f:?}");
    for secret in [PASSWORD, COOKIE_OUT, COOKIE_IN, TOKEN, API_KEY, USERINFO_PW] {
        assert!(
            !everything.contains(secret),
            "Geheimnis {secret:?} gefunden in:\n{everything}"
        );
    }
    // auch die Base64-Form von „admin:Passwort“ darf nirgends stehen
    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD.encode(format!("admin:{PASSWORD}"));
    assert!(
        !everything.contains(&b64),
        "kodiertes Passwort im Protokoll"
    );
}
