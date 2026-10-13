mod common;

use common::{get, get_at, parse, read_all, request_bytes};
use os_testsite::{SiteOptions, SyntheticConfig, TestSite};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

// spec: TECH-007
#[tokio::test]
async fn records_carry_all_fields_in_order() {
    let site = TestSite::start("testsite-hosts").await.unwrap();
    get_at(
        site.addr(),
        "Site.Test:99",
        "/who?x=1",
        &[("X-One", "a"), ("x-one", "b")],
    )
    .await;
    get(&site, "127.0.0.1", "/").await;

    let log = site.requests();
    assert_eq!(log.len(), 2);
    let first = &log[0];
    assert_eq!((first.seq, log[1].seq), (1, 2));
    assert_eq!(first.method, "GET");
    assert_eq!(first.host, "site.test");
    assert_eq!(first.path_and_query, "/who?x=1");
    assert_eq!(first.path(), "/who");
    assert_eq!(first.header_values("x-one"), vec!["a", "b"]);
    assert_eq!(first.header("host"), Some("Site.Test:99"));
    assert_eq!(first.headers[0].0, "Host");
    assert!(!first.tls);
    // RFC 3339 mit Millisekunden in UTC.
    assert_eq!(
        first.ts.len(),
        "2026-01-01T00:00:00.000Z".len(),
        "{}",
        first.ts
    );
    assert!(first.ts.ends_with('Z') && first.ts.contains('T'));
    assert!(log[1].ts >= first.ts);
    assert_ne!(first.conn_id, log[1].conn_id);
}

// spec: TECH-007
#[tokio::test]
async fn count_matches_host_and_path_with_or_without_query() {
    let site = TestSite::start("testsite-hosts").await.unwrap();
    get(&site, "site.test", "/who").await;
    get(&site, "site.test", "/who?a=1").await;
    get(&site, "other.test", "/who").await;
    assert_eq!(site.count("site.test", "/who"), 2);
    assert_eq!(site.count("SITE.test", "/who?a=1"), 1);
    assert_eq!(site.count("other.test", "/who"), 1);
    assert_eq!(site.count("nobody.test", "/who"), 0);
}

// spec: TECH-007
#[tokio::test]
async fn max_concurrency_sees_parallel_requests() {
    let site = TestSite::start("testsite-routes").await.unwrap();
    assert_eq!(site.max_concurrency(), 0);
    let (a, b, c, d) = tokio::join!(
        get(&site, "x", "/slow"),
        get(&site, "x", "/slow"),
        get(&site, "x", "/slow"),
        get(&site, "x", "/slow"),
    );
    assert!([a.status, b.status, c.status, d.status]
        .iter()
        .all(|s| *s == 200));
    assert_eq!(site.max_concurrency(), 4);
    // Nacheinander ändert den Höchstwert nicht.
    get(&site, "x", "/slow").await;
    assert_eq!(site.max_concurrency(), 4);
    site.reset_request_log();
    get(&site, "x", "/slow").await;
    assert_eq!(site.max_concurrency(), 1);
}

// spec: TECH-007
#[tokio::test]
async fn connections_count_tcp_connections_not_requests() {
    let site = TestSite::start("testsite-routes").await.unwrap();
    let mut stream = TcpStream::connect(site.addr()).await.unwrap();
    // Zwei Requests über eine Keep-Alive-Verbindung (ohne Connection: close).
    for _ in 0..2 {
        stream
            .write_all(b"GET /reason HTTP/1.1\r\nHost: x\r\n\r\n")
            .await
            .unwrap();
        let mut got = Vec::new();
        use tokio::io::AsyncReadExt;
        let mut buf = [0u8; 1024];
        while !got.ends_with(b"ok") {
            let n = stream.read(&mut buf).await.unwrap();
            assert!(n > 0);
            got.extend_from_slice(&buf[..n]);
        }
    }
    assert_eq!(site.connections(), 1);
    let log = site.requests();
    assert_eq!(log.len(), 2);
    assert_eq!(log[0].conn_id, log[1].conn_id);

    get(&site, "x", "/reason").await;
    get(&site, "x", "/reason").await;
    assert_eq!(site.connections(), 3);
    let ids: std::collections::BTreeSet<u64> = site.requests().iter().map(|r| r.conn_id).collect();
    assert_eq!(ids.len(), 3);

    site.reset_request_log();
    assert_eq!(site.connections(), 0);
    assert!(site.requests().is_empty());
    // conn_id bleibt über das Zurücksetzen hinweg eindeutig.
    get(&site, "x", "/reason").await;
    assert!(site.requests()[0].conn_id > *ids.iter().max().unwrap());
    assert_eq!(site.requests()[0].seq, 1);
}

// spec: TECH-007
#[tokio::test]
async fn synthetic_sites_log_details_only_on_request() {
    let cfg = SyntheticConfig {
        pages: 20,
        ..Default::default()
    };
    let quiet = TestSite::start_synthetic(cfg.clone(), SiteOptions::default())
        .await
        .unwrap();
    get(&quiet, "x", "/").await;
    assert!(quiet.requests().is_empty());
    assert_eq!(quiet.connections(), 1);

    let logged = TestSite::start_synthetic(
        cfg,
        SiteOptions {
            log_requests: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    get(&logged, "x", "/").await;
    assert_eq!(logged.requests().len(), 1);
    assert_eq!(logged.request_counts()["/"], 1);
    // Die JSON-Abfrage der Pfadzähler bleibt kompatibel und zählt sich nicht selbst.
    let json = get(&logged, "x", "/__testsite/requests").await;
    assert!(json.text().contains("\"total\":1"), "{}", json.text());
    assert_eq!(logged.requests().len(), 1);
}

// spec: TECH-007
#[tokio::test]
async fn http10_and_head_and_malformed_requests() {
    let site = TestSite::start("testsite-routes").await.unwrap();
    // HTTP/1.0 ohne Keep-Alive: Server schließt nach der Antwort.
    let mut s = TcpStream::connect(site.addr()).await.unwrap();
    s.write_all(b"GET /reason HTTP/1.0\r\n\r\n").await.unwrap();
    let (raw, err) = read_all(&mut s).await;
    assert!(err.is_none());
    let r = parse(&raw).unwrap();
    assert_eq!((r.status, r.text().as_str()), (200, "ok"));

    // HEAD liefert Content-Length, aber keinen Body.
    let mut s = TcpStream::connect(site.addr()).await.unwrap();
    s.write_all(b"HEAD /reason HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let (raw, _) = read_all(&mut s).await;
    let r = parse(&raw).unwrap();
    assert_eq!(r.header("content-length"), Some("2"));
    assert!(r.body.is_empty());

    // Unsinn bekommt 400.
    let mut s = TcpStream::connect(site.addr()).await.unwrap();
    s.write_all(b"NONSENSE\r\n\r\n").await.unwrap();
    let (raw, _) = read_all(&mut s).await;
    assert_eq!(parse(&raw).unwrap().status, 400);

    // POST mit Body wird verbraucht, die Verbindung bleibt benutzbar.
    let mut s = TcpStream::connect(site.addr()).await.unwrap();
    let mut req = b"POST /reason HTTP/1.1\r\nHost: x\r\nContent-Length: 5\r\n\r\nhello".to_vec();
    req.extend_from_slice(&request_bytes("x", "/reason", &[]));
    s.write_all(&req).await.unwrap();
    let (raw, _) = read_all(&mut s).await;
    let text = String::from_utf8_lossy(&raw);
    assert_eq!(text.matches("HTTP/1.1 200 Alles Klar").count(), 2, "{text}");
}
