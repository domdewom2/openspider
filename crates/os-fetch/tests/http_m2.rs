#[path = "http_support.rs"]
mod support;
use os_fetch::{Fetch, FetchConfig, FetchOutcome, HttpFetcher, MemorySecretStore};
use os_model::{Clock, CrawlConfig, ManualClock, ProxyMode, SystemClock, Timestamp};
use os_testsite::TestSite;
use std::{sync::Arc, time::Duration};
use support::*;
use url::Url;

fn setup(site: &TestSite, clock: Arc<dyn Clock>) -> FetchConfig {
    let mut c = config(clock);
    c.network.proxy.mode = ProxyMode::None;
    c.resolve = vec![format!("*.test={}", site.addr())];
    c.extra_root_certs = vec![site.ca_cert_pem().into_bytes()];
    c
}
fn url(site: &TestSite, scheme: &str, host: &str, path: &str) -> Url {
    Url::parse(&format!("{scheme}://{host}:{}{path}", site.addr().port())).unwrap()
}

// spec: CRAWL-018
#[tokio::test]
async fn hsts_is_crawl_local_secure_expiring_and_revocable() {
    let site = TestSite::start("crawl-m2-http").await.unwrap();
    let clock = Arc::new(ManualClock::new(Timestamp::UNIX_EPOCH));
    let cfg = setup(&site, clock.clone());
    let f = HttpFetcher::new(cfg.clone()).unwrap();
    get(&f, url(&site, "http", "site.test", "/hsts")).await;
    assert_eq!(
        expect_response(get(&f, url(&site, "http", "site.test", "/")).await).status,
        200
    );
    get(&f, url(&site, "https", "site.test", "/hsts")).await;
    for host in ["site.test", "blog.site.test"] {
        let r = expect_response(get(&f, url(&site, "http", host, "/")).await);
        assert_eq!(r.status, 307);
        assert_eq!(r.synthetic, Some(os_fetch::SyntheticResponse::Hsts));
        assert_eq!(
            r.header("location"),
            Some(url(&site, "https", host, "/").as_str())
        );
        assert_eq!(r.transferred_bytes, 0);
    }
    assert_eq!(
        expect_response(get(&f, url(&site, "http", "evilsite.test", "/")).await).status,
        200
    );
    let fresh = HttpFetcher::new(cfg.clone()).unwrap();
    assert_eq!(
        expect_response(get(&fresh, url(&site, "http", "site.test", "/")).await).status,
        200
    );
    clock.advance(Duration::from_secs(61));
    assert_eq!(
        expect_response(get(&f, url(&site, "http", "site.test", "/")).await).status,
        200
    );
    get(&f, url(&site, "https", "site.test", "/hsts")).await;
    get(&f, url(&site, "https", "site.test", "/remove-hsts")).await;
    assert_eq!(
        expect_response(get(&f, url(&site, "http", "site.test", "/")).await).status,
        200
    );
    let mut off = cfg;
    off.http.respect_hsts = false;
    let off = HttpFetcher::new(off).unwrap();
    get(&off, url(&site, "https", "site.test", "/hsts")).await;
    assert_eq!(
        expect_response(get(&off, url(&site, "http", "site.test", "/")).await).status,
        200
    );
}

// spec: CRAWL-023
// spec: OUT-270
// spec: CRAWL-123 [M2]
#[tokio::test]
async fn tls_import_errors_sni_and_probe_chain() {
    let site = TestSite::start("crawl-m2-http").await.unwrap();
    let mut cfg = setup(&site, Arc::new(SystemClock));
    cfg.extra_root_certs.clear();
    let untrusted = HttpFetcher::new(cfg.clone()).unwrap();
    assert!(matches!(
        get(&untrusted, url(&site, "https", "site.test", "/"))
            .await
            .outcome,
        FetchOutcome::NoResponse {
            reason: os_model::NoResponseReason::TlsError,
            ..
        }
    ));
    let dir = tempfile::tempdir().unwrap();
    let ca = dir.path().join("ca.pem");
    std::fs::write(&ca, site.ca_cert_pem()).unwrap();
    cfg.network
        .trusted_certificates
        .push(ca.to_string_lossy().into_owned());
    let f = HttpFetcher::new(cfg.clone()).unwrap();
    assert_eq!(
        expect_response(get(&f, url(&site, "https", "site.test", "/")).await).status,
        200
    );
    for host in ["expired.site.test", "wronghost.site.test"] {
        assert!(matches!(
            get(&f, url(&site, "https", host, "/")).await.outcome,
            FetchOutcome::NoResponse {
                reason: os_model::NoResponseReason::TlsError,
                ..
            }
        ));
    }
    let probe = os_fetch::net::certificates::probe(
        url(&site, "https", "site.test", "/").as_str(),
        &cfg.network,
        None,
        &cfg.resolve,
    )
    .await
    .unwrap();
    assert!(probe.trusted);
    assert!(probe.certificates[0].starts_with("-----BEGIN CERTIFICATE-----"));
    let probe = os_fetch::net::certificates::probe(
        url(&site, "https", "expired.site.test", "/").as_str(),
        &cfg.network,
        None,
        &cfg.resolve,
    )
    .await
    .unwrap();
    assert!(!probe.trusted);
    std::fs::write(&ca, site.ca_cert_der()).unwrap();
    let der = HttpFetcher::new(cfg.clone()).unwrap();
    assert_eq!(
        expect_response(get(&der, url(&site, "https", "site.test", "/")).await).status,
        200
    );
    cfg.http.ignore_certificate_errors = true;
    let f = HttpFetcher::new(cfg.clone()).unwrap();
    for host in ["expired.site.test", "wronghost.site.test"] {
        assert_eq!(
            expect_response(get(&f, url(&site, "https", host, "/")).await).status,
            200
        );
    }
    std::fs::write(ca, b"not a certificate").unwrap();
    assert!(HttpFetcher::new(cfg).is_err());
}

// spec: CRAWL-024
#[tokio::test]
async fn alpn_h2_and_http1_have_identical_fixture_results_without_h2c() {
    let site = TestSite::start("crawl-m2-http").await.unwrap();
    let cfg = setup(&site, Arc::new(SystemClock));
    let f = HttpFetcher::new(cfg.clone()).unwrap();
    let h2 = expect_response(get(&f, url(&site, "https", "site.test", "/")).await);
    assert_eq!(h2.http_version, "HTTP/2");
    let mut cfg = cfg;
    cfg.http.version = os_model::HttpVersion::Http1;
    let h1 = expect_response(
        get(
            &HttpFetcher::new(cfg).unwrap(),
            url(&site, "https", "site.test", "/"),
        )
        .await,
    );
    assert_eq!(h1.http_version, "HTTP/1.1");
    assert_eq!(h1.body, h2.body);
    assert_eq!(h1.status, h2.status);
    assert_eq!(h1.header("content-type"), h2.header("content-type"));
    assert_eq!(
        expect_response(get(&f, url(&site, "http", "site.test", "/")).await).http_version,
        "HTTP/1.1"
    );
}

// spec: CRAWL-115
// spec: CRAWL-116
#[tokio::test]
async fn headers_are_validated_merged_scoped_and_language_is_opt_in() {
    let site = TestSite::start("crawl-m2-http").await.unwrap();
    let mut cfg = setup(&site, Arc::new(SystemClock));
    let plain = HttpFetcher::new(cfg.clone()).unwrap();
    assert!(!String::from_utf8_lossy(
        &expect_response(get(&plain, url(&site, "http", "site.test", "/echo")).await).body
    )
    .contains("accept-language"));
    cfg.http.accept_language = Some("de-DE".into());
    cfg.http.headers = vec![
        os_model::HeaderEntry {
            name: "X-Fixture".into(),
            value: "secret".into(),
            host: None,
        },
        os_model::HeaderEntry {
            name: "Cookie".into(),
            value: "custom=1".into(),
            host: None,
        },
    ];
    let f = HttpFetcher::new(cfg.clone()).unwrap();
    let mut request = page_request(url(&site, "http", "site.test", "/echo"));
    let jar = Arc::new(os_fetch::CookieJar::new());
    jar.store_set_cookie(&request.url, ["session=2; Path=/"]);
    request.cookies = Some(jar);
    let r = expect_response(f.fetch(request).await);
    let body = String::from_utf8_lossy(&r.body);
    assert!(body.contains("de-DE"));
    assert!(body.contains("secret"));
    assert!(body.contains("custom=1"));
    assert!(body.contains("session=2"));
    let external = expect_response(get(&f, url(&site, "http", "other.test", "/echo")).await);
    assert!(!String::from_utf8_lossy(&external.body).contains("secret"));
    for (name, value) in [("User-Agent", "x"), ("X-Foo", "a\r\nb")] {
        cfg.http.headers = vec![os_model::HeaderEntry {
            name: name.into(),
            value: value.into(),
            host: None,
        }];
        assert!(HttpFetcher::new(cfg.clone()).is_err());
    }
}

// spec: CRAWL-075 [M2]
// spec: CRAWL-140 [P1]
// spec: CRAWL-042 [M2]
#[tokio::test]
async fn limits_apply_during_receipt_to_decoded_body_and_media() {
    use std::io::Write;
    let big = vec![b'a'; 2_000_000];
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gzip.write_all(&big).unwrap();
    let compressed = gzip.finish().unwrap();
    let server = MiniServer::start(move |req| {
        let (ct, bytes, enc) = if req.path() == "/gzip" {
            ("text/html", compressed.clone(), "gzip")
        } else {
            ("video/mp4", big.clone(), "identity")
        };
        Reply::Raw(response(
            200,
            "OK",
            &[("Content-Type", ct), ("Content-Encoding", enc)],
            &bytes,
        ))
    })
    .await;
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    let mut req = page_request(server.url("/gzip"));
    req.body_limit.max_decoded_bytes = 1024;
    req.attempt = 2;
    let r = expect_response(f.fetch(req).await);
    assert_eq!(r.body.len(), 1024);
    assert!(r.truncated);
    assert_eq!(r.decode_error, None);
    assert_eq!(r.attempts, 3);
    assert!(r.untruncated_bytes.is_some());
    let r = expect_response(get(&f, server.url("/media")).await);
    assert_eq!(r.body.len(), 65536);
    assert!(r.truncated);
    assert_eq!(r.content_length, Some(2_000_000));
    assert_eq!(r.untruncated_bytes, Some(2_000_000));
    assert!(r.transferred_bytes < 2_000_000);
}

// spec: CRAWL-123
#[tokio::test]
async fn localhost_reaches_ipv4_only_listener() {
    let server = MiniServer::start(|_| ok_text("ipv4")).await;
    let mut url = server.url("/");
    url.set_host(Some("localhost")).unwrap();
    let f = HttpFetcher::new(
        FetchConfig::from_crawl_config(
            &CrawlConfig::default(),
            &MemorySecretStore::new(),
            Arc::new(SystemClock),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(expect_response(get(&f, url).await).body, "ipv4");
}

// spec: CRAWL-118
#[tokio::test]
async fn digest_md5_sha256_nonce_counter_and_stale_challenge() {
    use md5::{Digest, Md5};
    use sha2::Sha256;
    for algorithm in ["MD5", "SHA-256"] {
        let server=MiniServer::start(move |req| {
            let authorization=req.header("authorization").unwrap_or("");
            let challenge=|nonce:&str,stale:bool| Reply::Raw(response(401,"Unauthorized",&[("WWW-Authenticate",&format!("Digest realm=\"fixture\", nonce=\"{nonce}\", algorithm={algorithm}, qop=\"auth\", stale={stale}"))],b""));
            if !authorization.starts_with("Digest ") { return challenge("old",false); }
            if authorization.contains("nonce=\"old\"") { return challenge("new",true); }
            let field=|key:&str| authorization.split_once(&format!("{key}=\"")).unwrap().1.split('"').next().unwrap().to_string();
            let nc=authorization.split_once("nc=").unwrap().1.split(',').next().unwrap();
            let hash=|s:&str| if algorithm=="MD5" { hex::encode(Md5::digest(s.as_bytes())) } else { hex::encode(Sha256::digest(s.as_bytes())) };
            let a1=hash("user:fixture:password"); let a2=hash(&format!("GET:{}",req.target)); let digest=hash(&format!("{a1}:new:{nc}:{}:auth:{a2}",field("cnonce")));
            assert_eq!(field("response"),digest); assert_eq!(field("uri"),req.target);
            ok_text(nc)
        }).await;
        let mut cfg = config(Arc::new(SystemClock));
        cfg.basic_auth = vec![os_fetch::BasicAuthEntry {
            prefix: server.url("/").to_string(),
            username: "user".into(),
            password: "password".into(),
        }];
        let f = HttpFetcher::new(cfg).unwrap();
        let first = expect_response(get(&f, server.url("/one?x=1")).await);
        assert_eq!(first.status, 200);
        assert_eq!(first.body, "00000001");
        let second = expect_response(get(&f, server.url("/two")).await);
        assert_eq!(second.body, "00000002");
        assert!(!format!("{first:?}").contains("password"));
    }
}
