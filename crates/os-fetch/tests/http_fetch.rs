//! Abruf gegen den Mini-Server: Header, Kompression, Byte-Zählung, Fehlerfälle, Zeitgrenzen,
//! Rate-Limit und Host-Auflösung (CRAWL-011/013/022/095/113/114/123, ONPAGE-065).
//!
//! Der Mini-Server steuert die Antworten byte-genau (`http_support.rs`); die Fixture-Server-Fälle
//! stehen in `http_status.rs` und, soweit sie auf M1-P09 warten, in `http_p09.rs`.

#[path = "http_support.rs"]
mod support;

use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

use md5::{Digest, Md5};
use os_fetch::{Fetch, FetchPurpose, FetchRequest};
use os_model::{FixedClock, NoResponseReason, SystemClock, Timestamp};
use support::*;
use url::Url;

fn gzip(data: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

fn zlib(data: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

fn raw_deflate(data: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

fn brotli_bytes(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut w = brotli::CompressorWriter::new(&mut out, 4096, 5, 22);
        w.write_all(data).unwrap();
    }
    out
}

fn html_50k() -> Vec<u8> {
    let mut s = String::from("<!doctype html><html><body>");
    let mut i = 0;
    while s.len() < 50_000 {
        s.push_str(&format!("<p>Absatz {i} mit etwas Text</p>"));
        i += 1;
    }
    let mut bytes = s.into_bytes();
    bytes.truncate(50_000);
    bytes
}

fn md5_hex(data: &[u8]) -> String {
    hex::encode(Md5::digest(data))
}

fn encoded_server_handler(req: &Request) -> Reply {
    let text =
        b"<html><body>Hallo Welt, Hallo Welt, Hallo Welt, Hallo Welt</body></html>".as_slice();
    let ct = ("content-type", "text/html; charset=utf-8");
    match req.path() {
        "/gzip" => Reply::Raw(response(
            200,
            "OK",
            &[ct, ("content-encoding", "gzip")],
            &gzip(text),
        )),
        "/br" => Reply::Raw(response(
            200,
            "OK",
            &[ct, ("content-encoding", "br")],
            &brotli_bytes(text),
        )),
        "/deflate-zlib" => Reply::Raw(response(
            200,
            "OK",
            &[ct, ("content-encoding", "deflate")],
            &zlib(text),
        )),
        "/deflate-raw" => Reply::Raw(response(
            200,
            "OK",
            &[ct, ("content-encoding", "deflate")],
            &raw_deflate(text),
        )),
        "/gzip-then-br" => Reply::Raw(response(
            200,
            "OK",
            &[ct, ("content-encoding", "gzip, br")],
            &brotli_bytes(&gzip(text)),
        )),
        "/chunked" => Reply::Raw(response(
            200,
            "OK",
            &[ct, ("transfer-encoding", "chunked")],
            &chunked(&[&text[..10], &text[10..30], &text[30..]]),
        )),
        "/chunked-gzip" => {
            let z = gzip(text);
            Reply::Raw(response(
                200,
                "OK",
                &[
                    ct,
                    ("content-encoding", "gzip"),
                    ("transfer-encoding", "chunked"),
                ],
                &chunked(&[&z[..7], &z[7..]]),
            ))
        }
        "/broken-gzip" => Reply::Raw(response(
            200,
            "OK",
            &[ct, ("content-encoding", "gzip")],
            text,
        )),
        "/truncated-gzip" => {
            let z = gzip(&text.repeat(40));
            let half = &z[..z.len() / 2];
            Reply::Raw(response(
                200,
                "OK",
                &[ct, ("content-encoding", "gzip")],
                half,
            ))
        }
        "/gzip-50k" => Reply::Raw(response(
            200,
            "OK",
            &[ct, ("content-encoding", "gzip")],
            &gzip(&html_50k()),
        )),
        "/plain-50k" => Reply::Raw(response(200, "OK", &[ct], &html_50k())),
        _ => Reply::Raw(response(404, "Not Found", &[], b"")),
    }
}

const TEXT: &[u8] = b"<html><body>Hallo Welt, Hallo Welt, Hallo Welt, Hallo Welt</body></html>";

// spec: CRAWL-011, CRAWL-114
#[tokio::test]
async fn sends_the_fixed_header_set_and_the_configured_user_agent() {
    let server = MiniServer::start(|req| {
        let echoed: Vec<String> = req
            .headers
            .iter()
            .map(|(n, v)| format!("{n}: {v}"))
            .collect();
        Reply::Raw(response(
            200,
            "OK",
            &[
                ("content-type", "text/plain"),
                ("last-modified", "Wed, 21 Oct 2015 07:28:00 GMT"),
            ],
            echoed.join("\n").as_bytes(),
        ))
    })
    .await;
    let ua = "Mozilla/5.0 (compatible; Test-UA/1.0; +https://example.org/bot)";
    let f = fetcher_with(Arc::new(SystemClock), |c| c.user_agent = ua.to_string());
    let resp = expect_response(get(&f, server.url("/echo-headers")).await);
    let echoed = String::from_utf8(resp.body.to_vec()).unwrap();
    let lines: Vec<&str> = echoed.lines().collect();
    assert!(
        lines.contains(&format!("user-agent: {ua}").as_str()),
        "{echoed}"
    );
    assert!(
        lines.contains(&"accept-encoding: gzip, deflate, br"),
        "{echoed}"
    );
    assert!(lines.contains(&"connection: keep-alive"), "{echoed}");
    let accept = lines
        .iter()
        .find(|l| l.starts_with("accept: "))
        .expect("Accept fehlt");
    assert!(
        accept.contains("text/html") && accept.contains("*/*"),
        "{accept}"
    );
    assert!(
        !echoed.contains("cookie:"),
        "keine Cookies ohne Modus: {echoed}"
    );
    assert!(!echoed.contains("authorization:"), "{echoed}");
    // Antwort-Header kommen unverändert in Antwortreihenfolge durch (Last-Modified für CRAWL-022)
    assert_eq!(
        header(&resp, "last-modified"),
        Some("Wed, 21 Oct 2015 07:28:00 GMT")
    );
    assert_eq!(resp.http_version, "HTTP/1.1");
}

// spec: CRAWL-011
#[tokio::test]
async fn only_get_requests_are_sent() {
    let server = MiniServer::start(|_| ok_text("x")).await;
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    for path in ["/a", "/b", "/c"] {
        let _ = expect_response(get(&f, server.url(path)).await);
    }
    let reqs = server.requests();
    assert_eq!(reqs.len(), 3);
    assert!(reqs.iter().all(|r| r.method == "GET"), "{reqs:?}");
}

// spec: CRAWL-011
#[tokio::test]
async fn gzip_br_deflate_and_chunked_bodies_are_decoded() {
    let server = MiniServer::start(encoded_server_handler).await;
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    for path in [
        "/gzip",
        "/br",
        "/deflate-zlib",
        "/deflate-raw",
        "/gzip-then-br",
        "/chunked",
        "/chunked-gzip",
    ] {
        let resp = expect_response(get(&f, server.url(path)).await);
        assert_eq!(resp.status, 200, "{path}");
        assert_eq!(resp.decode_error, None, "{path}");
        assert_eq!(resp.body.as_ref(), TEXT, "{path}");
        assert_eq!(
            resp.size_bytes,
            TEXT.len() as u64,
            "{path}: Größe = dekomprimiert"
        );
        assert_eq!(
            resp.md5_hex,
            md5_hex(TEXT),
            "{path}: MD5 des dekodierten Bodys"
        );
        assert_eq!(resp.status_text, "OK", "{path}");
    }
}

// spec: CRAWL-011, CRAWL-022
#[tokio::test]
async fn size_is_decompressed_and_transferred_is_the_compressed_body_without_head() {
    // K-02: `Transferred` = Body-Bytes auf der Leitung (gzip: die komprimierte Länge), ohne Kopf.
    let server = MiniServer::start(encoded_server_handler).await;
    let f = fetcher_with(Arc::new(SystemClock), |_| {});

    let compressed_len = gzip(&html_50k()).len() as u64;
    let before = server.bytes_sent();
    let resp = expect_response(get(&f, server.url("/gzip-50k")).await);
    let sent = server.bytes_sent() - before;
    assert_eq!(resp.size_bytes, 50_000, "Größe = dekomprimierter Body");
    assert_eq!(resp.body.len(), 50_000);
    assert!(
        resp.transferred_bytes < 20_000,
        "Übertragen ist die komprimierte Größe, nicht {}",
        resp.transferred_bytes
    );
    // Abweichung zur komprimierten Body-Länge: genau 0; der gesendete Kopf kommt nicht hinzu
    assert_eq!(resp.transferred_bytes, compressed_len, "komprimierter Body");
    assert!(
        sent > compressed_len,
        "der Server sendet zusätzlich den Kopf"
    );

    let plain = expect_response(get(&f, server.url("/plain-50k")).await);
    assert_eq!(plain.size_bytes, 50_000);
    assert_eq!(
        plain.transferred_bytes, 50_000,
        "unkomprimiert: Transferred = Size, ohne Kopf"
    );
}

// spec: CRAWL-011
#[tokio::test]
async fn broken_compression_keeps_the_status_and_reports_connection_error() {
    let server = MiniServer::start(encoded_server_handler).await;
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    // Header sagt gzip, Body ist unkomprimiert
    let resp = expect_response(get(&f, server.url("/broken-gzip")).await);
    assert_eq!(resp.status, 200, "empfangener Statuscode bleibt");
    assert_eq!(resp.status_text, "Connection Error");
    let detail = resp.decode_error.expect("Detailtext");
    assert!(detail.contains("gzip"), "{detail}");
    // mittendrin abgeschnittener gzip-Strom
    let resp = expect_response(get(&f, server.url("/truncated-gzip")).await);
    assert_eq!(resp.status, 200);
    assert!(resp.decode_error.is_some());
    assert_eq!(resp.status_text, "Connection Error");
    // der Abrufer läuft weiter
    let ok = expect_response(get(&f, server.url("/gzip")).await);
    assert_eq!(ok.decode_error, None);
}

// spec: CRAWL-011, CRAWL-075
#[tokio::test]
async fn body_is_cut_at_the_maximum_size() {
    let server = MiniServer::start(encoded_server_handler).await;
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    let mut req = page_request(server.url("/plain-50k"));
    req.max_body_bytes = 1000;
    let resp = expect_response(f.fetch(req).await);
    assert!(resp.truncated);
    assert_eq!(resp.body.len(), 1000);
    assert_eq!(resp.size_bytes, 1000);
    // komprimiert: Obergrenze gilt für den dekomprimierten Body
    let mut req = page_request(server.url("/gzip-50k"));
    req.max_body_bytes = 2000;
    let resp = expect_response(f.fetch(req).await);
    assert!(resp.truncated);
    assert_eq!(resp.body.len(), 2000);
    assert_eq!(
        resp.decode_error, None,
        "selbst gesetzte Grenze ist kein Kompressionsfehler"
    );
}

// spec: CRAWL-011
#[tokio::test]
async fn keep_alive_reuses_the_connection_per_host() {
    let server = MiniServer::start(|_| ok_text("seite")).await;
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    for i in 0..20 {
        let resp = expect_response(get(&f, server.url(&format!("/seite-{i}"))).await);
        assert_eq!(resp.status, 200);
    }
    assert!(
        server.connections() <= 2,
        "20 Seiten mit einem Thread: {} Verbindungen",
        server.connections()
    );
}

// spec: CRAWL-012
#[tokio::test]
async fn all_listed_status_codes_are_kept_with_iana_or_empty_text() {
    let server = MiniServer::start(|req| {
        let n: u16 = req.path().trim_start_matches("/code/").parse().unwrap();
        // Der Server sendet bewusst keine Reason-Phrase (wie HTTP/2): der Fetcher setzt den IANA-Text ein.
        let body: &[u8] = if matches!(n, 204 | 304) { b"" } else { b"x" };
        Reply::Raw(response(n, "", &[], body))
    })
    .await;
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    let expected: &[(u16, &str)] = &[
        (200, "OK"),
        (201, "Created"),
        (204, "No Content"),
        (206, "Partial Content"),
        (218, ""),
        (299, ""),
        (301, "Moved Permanently"),
        (302, "Found"),
        (303, "See Other"),
        (304, "Not Modified"),
        (307, "Temporary Redirect"),
        (308, "Permanent Redirect"),
        (400, "Bad Request"),
        (401, "Unauthorized"),
        (403, "Forbidden"),
        (404, "Not Found"),
        (410, "Gone"),
        (418, ""),
        (429, "Too Many Requests"),
        (451, "Unavailable For Legal Reasons"),
        (499, ""),
        (500, "Internal Server Error"),
        (502, "Bad Gateway"),
        (503, "Service Unavailable"),
        (504, "Gateway Timeout"),
        (520, ""),
        (600, ""),
        (999, ""),
    ];
    for (code, text) in expected {
        let resp = expect_response(get(&f, server.url(&format!("/code/{code}"))).await);
        assert_eq!(
            resp.status, *code,
            "Statuscode {code} wird unverändert übernommen"
        );
        assert_eq!(resp.status_text, *text, "Status zu {code}");
        assert_eq!(resp.decode_error, None);
    }
}

// spec: CRAWL-012
#[tokio::test]
async fn informational_responses_are_skipped() {
    let server = MiniServer::start(|_| {
        let mut bytes =
            b"HTTP/1.1 103 Early Hints\r\nlink: </style.css>; rel=preload; as=style\r\n\r\n"
                .to_vec();
        bytes.extend(response(
            200,
            "OK",
            &[("content-type", "text/html")],
            b"<html>fertig</html>",
        ));
        Reply::Raw(bytes)
    })
    .await;
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    let resp = expect_response(get(&f, server.url("/early-hints")).await);
    assert_eq!(resp.status, 200, "gespeichert wird die finale Antwort");
    assert_eq!(resp.body.as_ref(), b"<html>fertig</html>");
    assert_eq!(
        header(&resp, "link"),
        None,
        "Header der 103-Antwort gehören nicht dazu"
    );
}

// spec: CRAWL-012
#[tokio::test]
async fn not_modified_without_condition_is_a_plain_3xx_without_redirect() {
    let server = MiniServer::start(|_| Reply::Raw(response(304, "Not Modified", &[], b""))).await;
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    let resp = expect_response(get(&f, server.url("/not-modified")).await);
    assert_eq!(resp.status, 304);
    assert_eq!(header(&resp, "location"), None);
    assert!(resp.body.is_empty());
}

// spec: CRAWL-012
#[tokio::test]
async fn redirects_are_not_followed_by_the_fetcher() {
    let server = MiniServer::start(|req| match req.path() {
        "/r301" => Reply::Raw(response(
            301,
            "Moved Permanently",
            &[("location", "/ziel")],
            b"",
        )),
        _ => ok_text("ziel"),
    })
    .await;
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    let resp = expect_response(get(&f, server.url("/r301")).await);
    assert_eq!(resp.status, 301);
    assert_eq!(header(&resp, "location"), Some("/ziel"));
    assert_eq!(
        server.requests().len(),
        1,
        "die Engine verfolgt Location selbst"
    );
}

// spec: CRAWL-013, ONPAGE-065
#[tokio::test]
async fn unresolvable_host_is_dns_lookup_failed() {
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    let (reason, _detail, _) =
        expect_no_response(get(&f, Url::parse("http://nicht-existent.invalid/").unwrap()).await);
    assert_eq!(reason, NoResponseReason::DnsLookupFailed);
    assert_eq!(reason.status_text(), "DNS Lookup Failed");
}

// spec: CRAWL-013, ONPAGE-065
#[tokio::test]
async fn closed_port_is_connection_refused() {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    let (reason, _, _) = expect_no_response(
        get(
            &f,
            Url::parse(&format!("http://127.0.0.1:{port}/")).unwrap(),
        )
        .await,
    );
    assert_eq!(reason, NoResponseReason::ConnectionRefused);
    assert_eq!(reason.status_text(), "Connection Refused");
}

// spec: CRAWL-013, ONPAGE-065
#[tokio::test]
async fn connection_dropped_mid_body_or_after_headers_is_connection_error() {
    let server = MiniServer::start(|req| match req.path() {
        // Länge angekündigt, nach 10 Bytes abgebrochen
        "/reset-mid-body" => {
            let mut bytes = head_only(200, "OK", &[("content-type", "text/html")], 1000);
            bytes.extend_from_slice(b"0123456789");
            Reply::RawThenClose(bytes)
        }
        // Verbindung wird nach dem Header-Versand geschlossen (F-Erweiterung `/drop`)
        "/drop" => Reply::RawThenClose(head_only(200, "OK", &[("content-type", "text/html")], 100)),
        // gar keine Antwort
        _ => Reply::Close,
    })
    .await;
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    for path in ["/reset-mid-body", "/drop", "/nichts"] {
        let (reason, detail, _) = expect_no_response(get(&f, server.url(path)).await);
        assert_eq!(
            reason,
            NoResponseReason::ConnectionError,
            "{path}: {detail}"
        );
        assert_eq!(reason.status_text(), "Connection Error");
    }
}

// spec: CRAWL-013, ONPAGE-065
#[tokio::test]
async fn https_against_a_plain_server_is_a_tls_error() {
    // Der Server antwortet auf das TLS-ClientHello mit Klartext-HTTP und schließt.
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        while let Ok((mut s, _)) = listener.accept().await {
            let mut buf = [0u8; 2048];
            let _ = s.read(&mut buf).await;
            let _ = s
                .write_all(b"HTTP/1.1 400 Bad Request\r\ncontent-length: 0\r\n\r\n")
                .await;
            let _ = s.shutdown().await;
        }
    });
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    let (reason, detail, _) = expect_no_response(
        get(
            &f,
            Url::parse(&format!("https://127.0.0.1:{port}/")).unwrap(),
        )
        .await,
    );
    assert_eq!(reason, NoResponseReason::TlsError, "{detail}");
    assert_eq!(reason.status_text(), "TLS Error");
}

// spec: CRAWL-013, ONPAGE-065
#[tokio::test]
async fn unsupported_schemes_are_malformed() {
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    for u in [
        "ftp://example.org/datei",
        "file:///etc/hosts",
        "mailto:a@example.org",
    ] {
        let (reason, detail, _) = expect_no_response(get(&f, Url::parse(u).unwrap()).await);
        assert_eq!(reason, NoResponseReason::Malformed, "{u}: {detail}");
        assert_eq!(reason.status_text(), "Malformed");
    }
}

// Zeitkriterien laufen hier mit kurzen echten Zeiten (Millisekunden bis Sekundenbruchteile) gegen
// echte Sockets: `start_paused` und echte Ein-/Ausgabe vertragen sich nicht (die Uhr springt, sobald
// die Laufzeit auf ein Socket-Ereignis wartet). Die exakten Zahlen aus der Spec (2,0 URLs/s über 20
// Seiten ≥ 9,5 s, Abstand 2 s bei 0,5 URLs/s, 20-s-Timeout) prüfen die Tests ohne Socket in `rate.rs`.

// spec: CRAWL-095, CRAWL-013
#[tokio::test]
async fn response_timeout_aborts_after_the_configured_time() {
    let server = MiniServer::start(|req| match req.path() {
        "/slow-long" => Reply::After(Duration::from_secs(5), Box::new(ok_text("spät"))),
        "/slow-short" => Reply::After(Duration::from_millis(150), Box::new(ok_text("rechtzeitig"))),
        _ => Reply::Hang,
    })
    .await;
    let f = fetcher_with(Arc::new(SystemClock), |c| {
        c.response_timeout = Duration::from_millis(600)
    });

    // Server antwortet viel später als das Timeout: „Antwort-Timeout“ kurz nach Ablauf
    let (reason, _, elapsed_ms) = expect_no_response(get(&f, server.url("/slow-long")).await);
    assert_eq!(reason, NoResponseReason::ResponseTimeout);
    assert!((600..=1500).contains(&elapsed_ms), "nach {elapsed_ms} ms");
    // K-04: im Export erscheint beides als „Connection Timeout“, die Ursache bleibt unterscheidbar
    assert_eq!(reason.status_text(), "Connection Timeout");
    assert_ne!(reason, NoResponseReason::ConnectionTimeout);

    // Antwort innerhalb des Timeouts: normaler 200
    let resp = expect_response(get(&f, server.url("/slow-short")).await);
    assert_eq!(resp.status, 200);
    assert!(resp.response_time_ms >= 150, "{} ms", resp.response_time_ms);

    // nie eine Antwort
    let (reason, _, _) = expect_no_response(get(&f, server.url("/hang")).await);
    assert_eq!(reason, NoResponseReason::ResponseTimeout);
}

// spec: CRAWL-095
#[tokio::test]
async fn trickling_body_is_cut_off_by_the_total_timeout() {
    let server = MiniServer::start(|_| Reply::Trickle {
        head: head_only(200, "OK", &[("content-type", "text/html")], 100),
        body: vec![b'x'; 100],
        interval: Duration::from_millis(200),
    })
    .await;
    let f = fetcher_with(Arc::new(SystemClock), |c| {
        c.response_timeout = Duration::from_millis(600)
    });
    // Der Kopf kommt sofort, der Body tröpfelt (20 s Gesamtdauer): Der Timeout gilt für die ganze Antwort.
    let (reason, _, elapsed_ms) = expect_no_response(get(&f, server.url("/trickle")).await);
    assert_eq!(reason, NoResponseReason::ResponseTimeout);
    assert!((600..=1500).contains(&elapsed_ms), "nach {elapsed_ms} ms");
}

// spec: CRAWL-022
#[tokio::test]
async fn response_time_includes_the_server_delay() {
    let server = MiniServer::start(|req| match req.path() {
        "/slow" => Reply::After(Duration::from_millis(300), Box::new(ok_text("langsam"))),
        _ => ok_text("schnell"),
    })
    .await;
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    let slow = expect_response(get(&f, server.url("/slow")).await);
    assert!(
        (300..=1000).contains(&slow.response_time_ms),
        "{} ms",
        slow.response_time_ms
    );
    let fast = expect_response(get(&f, server.url("/fast")).await);
    assert!(fast.response_time_ms < 150, "{} ms", fast.response_time_ms);
}

// spec: CRAWL-022
#[tokio::test]
async fn response_time_comes_from_clock_monotonic_not_the_wall_clock() {
    let server = MiniServer::start(|_| ok_text("x")).await;
    let start = Timestamp::from_unix_millis(1_700_000_000_000);
    let clock = Arc::new(FixedClock::new(start, Duration::from_millis(37)));
    let f = fetcher_with(clock, |_| {});
    let r = get(&f, server.url("/")).await;
    assert_eq!(r.started_at, start, "Zeitstempel = Request-Beginn der Uhr");
    let resp = expect_response(r);
    assert_eq!(
        resp.response_time_ms, 37,
        "deterministisch: ein Uhrenschritt"
    );
}

// spec: CRAWL-022, CRAWL-113
#[tokio::test]
async fn throttling_wait_is_not_part_of_the_response_time() {
    let server = MiniServer::start(|_| ok_text("schnell")).await;
    // 2 URLs/s: der zweite Abruf wartet rund 500 ms auf das Rate-Limit
    let f = fetcher_with(Arc::new(SystemClock), |c| c.max_urls_per_sec = Some(2.0));
    let a = expect_response(get(&f, server.url("/fast")).await);
    let t = std::time::Instant::now();
    let b = expect_response(get(&f, server.url("/fast")).await);
    let total = t.elapsed();
    assert!(
        total >= Duration::from_millis(450),
        "Drossel hat gewartet: {total:?}"
    );
    assert!(a.response_time_ms < 400);
    assert!(
        b.response_time_ms < 400,
        "Wartezeit ({total:?}) wird nicht mitgemessen: {} ms",
        b.response_time_ms
    );
}

// spec: CRAWL-113
#[tokio::test]
async fn rate_limit_applies_to_every_purpose_including_robots_and_sitemaps() {
    let server = MiniServer::start(|_| ok_text("x")).await;
    // 10 URLs/s = 100 ms Abstand; 11 Abrufe über alle fünf Zwecke
    let f = fetcher_with(Arc::new(SystemClock), |c| c.max_urls_per_sec = Some(10.0));
    let purposes = [
        FetchPurpose::Page,
        FetchPurpose::Resource,
        FetchPurpose::External,
        FetchPurpose::RobotsTxt,
        FetchPurpose::Sitemap,
    ];
    for i in 0..11 {
        let req = FetchRequest {
            url: server.url(&format!("/u{i}")),
            purpose: purposes[i % purposes.len()],
            cookies: None,
            max_body_bytes: 1 << 20,
        };
        let _ = expect_response(f.fetch(req).await);
    }
    assert_eq!(server.requests().len(), 11);
    // Der erste Abruf baut die Verbindung auf und kommt entsprechend später an; er bleibt außen vor.
    // Ankunftszeiten am Server schwanken außerdem um einige Millisekunden gegenüber den Abrufbeginnen.
    let times: Vec<Duration> = server.requests().iter().skip(1).map(|r| r.at).collect();
    let total = *times.last().unwrap() - times[0];
    assert!(
        total >= Duration::from_millis(800),
        "Gesamtdauer {total:?} (9 × 100 ms)"
    );
    for w in times.windows(2) {
        assert!(
            w[1] - w[0] >= Duration::from_millis(20),
            "Abstand {:?}",
            w[1] - w[0]
        );
    }
}

// spec: CRAWL-113
#[tokio::test]
async fn concurrent_fetches_share_the_rate_limit() {
    let server = MiniServer::start(|_| ok_text("x")).await;
    let f = Arc::new(fetcher_with(Arc::new(SystemClock), |c| {
        c.max_urls_per_sec = Some(10.0)
    }));
    let mut tasks = Vec::new();
    for i in 0..6 {
        let f = f.clone();
        let url = server.url(&format!("/p{i}"));
        tasks.push(tokio::spawn(async move { get(&f, url).await }));
    }
    for t in tasks {
        let _ = expect_response(t.await.unwrap());
    }
    let mut times: Vec<Duration> = server.requests().iter().map(|r| r.at).collect();
    times.sort();
    // ohne gemeinsames Budget kämen alle sechs innerhalb weniger Millisekunden an
    let span = *times.last().unwrap() - times[0];
    assert!(
        span >= Duration::from_millis(350),
        "Zeitspanne {span:?} (5 × 100 ms)"
    );
    for w in times.windows(2) {
        assert!(
            w[1] - w[0] >= Duration::from_millis(40),
            "{:?}",
            w[1] - w[0]
        );
    }
}

// spec: CRAWL-123
#[tokio::test]
async fn wildcard_resolve_maps_fixture_hosts_with_port_and_keeps_the_host_header() {
    let server = MiniServer::start(|_| ok_text("angekommen")).await;
    let port = server.addr.port();
    let f = fetcher_with(Arc::new(SystemClock), |c| {
        c.resolve = vec![format!("*.site.test=127.0.0.1:{port}")];
    });
    // URL ohne Port: der Port der Regel gilt (K-11)
    let resp = expect_response(get(&f, Url::parse("http://a.site.test/seite").unwrap()).await);
    assert_eq!(resp.body.as_ref(), b"angekommen");
    let req = server.last_request("/seite").unwrap();
    assert_eq!(
        req.header("host"),
        Some("a.site.test"),
        "Host-Header behält den Hostnamen"
    );
    // tiefere Subdomain und die Basis-Domain selbst
    for u in ["http://x.y.site.test/z", "http://site.test/z2"] {
        assert_eq!(
            expect_response(get(&f, Url::parse(u).unwrap()).await).status,
            200,
            "{u}"
        );
    }
}

// spec: CRAWL-123
#[tokio::test]
async fn explicit_url_port_beats_the_port_of_the_rule() {
    let server = MiniServer::start(|_| ok_text("hier")).await;
    let port = server.addr.port();
    let f = fetcher_with(Arc::new(SystemClock), |c| {
        // Die Regel zeigt auf einen Port, auf dem niemand lauscht.
        c.resolve = vec!["*.site.test=127.0.0.1:9".to_string()];
    });
    let url = Url::parse(&format!("http://a.site.test:{port}/p")).unwrap();
    let resp = expect_response(get(&f, url).await);
    assert_eq!(resp.status, 200);
    assert_eq!(
        server.last_request("/p").unwrap().header("host"),
        Some(format!("a.site.test:{port}").as_str())
    );
}

// spec: CRAWL-123
#[tokio::test]
async fn without_a_resolve_rule_the_fixture_host_does_not_resolve() {
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    let (reason, _, _) =
        expect_no_response(get(&f, Url::parse("http://site.test/").unwrap()).await);
    assert_eq!(reason, NoResponseReason::DnsLookupFailed);
    // und eine andere Domain bleibt von der Regel unberührt
    let f = fetcher_with(Arc::new(SystemClock), |c| {
        c.resolve = vec!["*.site.test=127.0.0.1:9".to_string()];
    });
    let (reason, _, _) =
        expect_no_response(get(&f, Url::parse("http://other.invalid/").unwrap()).await);
    assert_eq!(reason, NoResponseReason::DnsLookupFailed);
}

// spec: CRAWL-123
#[test]
fn invalid_resolve_rules_fail_at_construction() {
    let mut cfg = config(Arc::new(SystemClock));
    cfg.resolve = vec!["kaputt".to_string()];
    assert!(os_fetch::HttpFetcher::new(cfg).is_err());
}

// spec: CRAWL-011, CRAWL-114
#[test]
fn invalid_user_agent_and_zero_timeout_fail_at_construction() {
    let mut cfg = config(Arc::new(SystemClock));
    cfg.user_agent = "Bot\r\nInjected: 1".to_string();
    assert!(os_fetch::HttpFetcher::new(cfg).is_err());
    let mut cfg = config(Arc::new(SystemClock));
    cfg.response_timeout = Duration::ZERO;
    assert!(os_fetch::HttpFetcher::new(cfg).is_err());
}

// spec: CRAWL-011
#[tokio::test]
async fn md5_is_taken_from_the_decoded_body() {
    let server = MiniServer::start(|_| ok_text("hello")).await;
    let f = fetcher_with(Arc::new(SystemClock), |_| {});
    let resp = expect_response(get(&f, server.url("/")).await);
    assert_eq!(resp.md5_hex, "5d41402abc4b2a76b9719d911017c592");
}

// spec: CRAWL-114
#[tokio::test]
async fn googlebot_preset_user_agent_reaches_the_server() {
    let server = MiniServer::start(|req| ok_text(req.header("user-agent").unwrap_or(""))).await;
    let mut cfg = os_model::CrawlConfig::default();
    cfg.user_agent.preset = "googlebot-desktop".to_string();
    let fc = os_fetch::FetchConfig::from_crawl_config(
        &cfg,
        &os_fetch::MemorySecretStore::new(),
        Arc::new(SystemClock),
    )
    .unwrap();
    let f = os_fetch::HttpFetcher::new(fc).unwrap();
    let resp = expect_response(get(&f, server.url("/")).await);
    assert_eq!(
        String::from_utf8_lossy(&resp.body),
        "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)"
    );
}

// spec: CRAWL-095
#[test]
fn connect_timeout_is_the_smaller_of_ten_seconds_and_the_response_timeout() {
    let connect = |secs: u64| {
        fetcher_with(Arc::new(SystemClock), |c| {
            c.response_timeout = Duration::from_secs(secs);
        })
        .connect_timeout()
    };
    assert_eq!(
        connect(20),
        Duration::from_secs(10),
        "Standard: 20 s Antwort, 10 s Verbindung"
    );
    assert_eq!(connect(3), Duration::from_secs(3));
    assert_eq!(connect(10), Duration::from_secs(10));
    // Standardwert der Konfiguration ist 20 s (Referenz-Default)
    assert_eq!(
        config(Arc::new(SystemClock)).response_timeout,
        Duration::from_secs(20)
    );
}
