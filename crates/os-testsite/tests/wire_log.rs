//! Request-Log: TLS-SNI-Name je Verbindung (`sni`) und gesendete Bytes je Antwort (`sent_bytes`).

mod common;

use std::path::PathBuf;
use std::time::Duration;

use common::{read_all, request_bytes, tls_connect};
use os_testsite::{RequestRecord, TestSite};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

async fn site() -> TestSite {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios/wire");
    TestSite::start_dir(dir, 0).await.unwrap()
}

/// Wartet, bis der Server die Bytes aller bisher geloggten Requests verbucht hat (er tut das
/// nach dem Schreiben der Antwort, der Client kann schneller sein).
async fn settled(site: &TestSite, expected_requests: usize) -> Vec<RequestRecord> {
    for _ in 0..500 {
        let log = site.requests();
        if log.len() == expected_requests && log.iter().all(|r| r.sent_bytes > 0) {
            return log;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("Log nicht vollständig verbucht: {:?}", site.requests());
}

/// GET über Klartext mit `Connection: close`; liefert die Rohbytes der Antwort.
async fn raw_get(site: &TestSite, method: &str, path: &str) -> Vec<u8> {
    let mut stream = TcpStream::connect(site.addr()).await.unwrap();
    let req = String::from_utf8(request_bytes("x", path, &[])).unwrap();
    stream
        .write_all(req.replacen("GET", method, 1).as_bytes())
        .await
        .unwrap();
    read_all(&mut stream).await.0
}

// spec: CRAWL-123
#[tokio::test]
async fn sni_is_the_name_of_the_client_hello_and_independent_of_the_host_header() {
    let site = site().await;
    // Klartext: kein SNI
    raw_get(&site, "GET", "/plain").await;
    // TLS mit SNI a.site.test, aber Host-Header b.site.test
    let mut stream = tls_connect(&site, site.addr(), "a.site.test")
        .await
        .unwrap();
    stream
        .write_all(&request_bytes("b.site.test", "/plain", &[]))
        .await
        .unwrap();
    read_all(&mut stream).await;
    // TLS per IP-Adresse: der Client sendet keinen SNI
    let mut stream = tls_connect(&site, site.addr(), "127.0.0.1").await.unwrap();
    stream
        .write_all(&request_bytes("127.0.0.1", "/plain", &[]))
        .await
        .unwrap();
    read_all(&mut stream).await;

    let log = settled(&site, 3).await;
    assert_eq!(log[0].sni, None);
    assert!(!log[0].tls);
    assert_eq!(log[1].sni.as_deref(), Some("a.site.test"));
    assert_eq!(log[1].host, "b.site.test");
    assert!(log[1].tls);
    assert_eq!(log[2].sni, None, "IP-Adresse: kein SNI");
    assert!(log[2].tls);
}

// spec: CRAWL-123
#[tokio::test]
async fn all_requests_of_a_keep_alive_connection_share_the_sni_name() {
    let site = site().await;
    let mut stream = tls_connect(&site, site.addr(), "keep.site.test")
        .await
        .unwrap();
    for _ in 0..2 {
        let req = String::from_utf8(request_bytes("keep.site.test", "/plain", &[])).unwrap();
        stream
            .write_all(req.replace("Connection: close\r\n", "").as_bytes())
            .await
            .unwrap();
    }
    stream.flush().await.unwrap();
    let log = loop {
        let log = site.requests();
        if log.len() == 2 && log.iter().all(|r| r.sent_bytes > 0) {
            break log;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    };
    assert_eq!(log[0].conn_id, log[1].conn_id);
    assert_eq!(log[0].sni.as_deref(), Some("keep.site.test"));
    assert_eq!(log[1].sni, log[0].sni);
}

// spec: TECH-007
#[tokio::test]
async fn sent_bytes_of_a_content_length_response_are_head_plus_body() {
    let site = site().await;
    let raw = raw_get(&site, "GET", "/plain").await;
    let log = settled(&site, 1).await;
    let r = &log[0];
    assert_eq!(r.sent_bytes, raw.len() as u64, "{raw:?}");
    assert_eq!((r.sent_framing_bytes, r.sent_interim_bytes), (0, 0));
    assert_eq!(r.sent_payload_bytes(), raw.len() as u64);
    // 10 Byte Body ("plain body"), der Rest ist der Kopf
    let head = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
    assert_eq!(raw.len() - head, 10);
}

// spec: TECH-007
#[tokio::test]
async fn sent_bytes_of_head_and_bodyless_responses_cover_the_head_only() {
    let site = site().await;
    let head_raw = raw_get(&site, "HEAD", "/plain").await;
    let no_content = raw_get(&site, "GET", "/nocontent").await;
    let log = settled(&site, 2).await;
    assert_eq!(log[0].sent_bytes, head_raw.len() as u64);
    assert!(log[0].sent_bytes > 0 && head_raw.ends_with(b"\r\n\r\n"));
    assert_eq!(log[1].sent_bytes, no_content.len() as u64);
}

// spec: TECH-007
#[tokio::test]
async fn sent_bytes_of_a_chunked_response_split_into_payload_and_framing() {
    let site = site().await;
    let raw = raw_get(&site, "GET", "/chunked").await;
    let log = settled(&site, 1).await;
    let r = &log[0];
    assert_eq!(r.sent_bytes, raw.len() as u64, "alle geschriebenen Bytes");
    let head = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
    let decoded = common::dechunk(&raw[head..]).unwrap();
    assert_eq!(decoded.len(), 300);
    assert_eq!(
        r.sent_framing_bytes,
        (raw.len() - head - decoded.len()) as u64,
        "Chunk-Rahmen = Rohbody minus Nutzdaten"
    );
    // 128 + 128 + 44 Byte: je Chunk Größenzeile (`80\r\n`, 4 Byte) und abschließendes CRLF,
    // dazu der Schlusschunk `0\r\n\r\n`
    assert_eq!(r.sent_framing_bytes, (4 + 2) * 3 + 5);
    assert_eq!(r.sent_payload_bytes(), (head + 300) as u64);
}

// spec: TECH-007
#[tokio::test]
async fn sent_bytes_count_interim_responses_separately() {
    let site = site().await;
    let raw = raw_get(&site, "GET", "/hints").await;
    let log = settled(&site, 1).await;
    let r = &log[0];
    assert_eq!(r.sent_bytes, raw.len() as u64);
    let interim = "HTTP/1.1 103 Early Hints\r\nLink: </a.css>; rel=preload; as=style\r\nLink: </b.js>; rel=preload; as=script\r\n\r\n";
    assert_eq!(r.sent_interim_bytes, interim.len() as u64);
    assert!(raw.starts_with(interim.as_bytes()));
    assert_eq!(r.sent_payload_bytes(), (raw.len() - interim.len()) as u64);
}

// spec: TECH-007
#[tokio::test]
async fn sent_bytes_over_tls_are_counted_before_encryption() {
    let site = site().await;
    let mut stream = tls_connect(&site, site.addr(), "site.test").await.unwrap();
    stream
        .write_all(&request_bytes("site.test", "/plain", &[]))
        .await
        .unwrap();
    let (raw, _) = read_all(&mut stream).await;
    let log = settled(&site, 1).await;
    assert_eq!(log[0].sent_bytes, raw.len() as u64);
}

// spec: TECH-007
#[tokio::test]
async fn a_late_booking_after_reset_does_not_touch_new_entries() {
    let site = site().await;
    raw_get(&site, "GET", "/plain").await;
    settled(&site, 1).await;
    site.reset_request_log();
    assert!(site.requests().is_empty());
    let raw = raw_get(&site, "GET", "/plain").await;
    let log = settled(&site, 1).await;
    assert_eq!((log[0].seq, log[0].sent_bytes), (1, raw.len() as u64));
}
