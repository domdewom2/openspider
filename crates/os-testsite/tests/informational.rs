//! Route-Optionen `informational` und `informational_headers`: 1xx-Zwischenantworten (z. B. 103
//! Early Hints) vor der finalen Antwort.

mod common;

use std::path::PathBuf;

use common::{read_all, tls_connect};
use os_testsite::{Scenario, TestSite};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

async fn site() -> TestSite {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios/wire");
    TestSite::start_dir(dir, 0).await.unwrap()
}

/// Eine auf dem Draht gelesene Antwort (1xx oder final).
#[derive(Debug)]
struct Wire {
    status_line: String,
    status: u16,
    /// Header mit Kleinbuchstaben-Namen in Eingangsreihenfolge.
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Wire {
    fn header_all(&self, name: &str) -> Vec<&str> {
        self.headers
            .iter()
            .filter(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
            .collect()
    }
}

/// Zerlegt die Bytes einer Verbindung in aufeinanderfolgende Antworten; `None`, wenn der Puffer
/// mit einer unvollständigen Antwort endet.
fn split_responses(mut raw: &[u8]) -> Option<Vec<Wire>> {
    let mut out = Vec::new();
    while !raw.is_empty() {
        let end = raw.windows(4).position(|w| w == b"\r\n\r\n")?;
        let head = String::from_utf8_lossy(&raw[..end]).into_owned();
        let mut lines = head.split("\r\n");
        let status_line = lines.next()?.to_string();
        let status: u16 = status_line.split(' ').nth(1)?.parse().ok()?;
        let headers: Vec<(String, String)> = lines
            .filter_map(|l| l.split_once(':'))
            .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
            .collect();
        raw = &raw[end + 4..];
        let chunked = headers
            .iter()
            .any(|(k, v)| k == "transfer-encoding" && v == "chunked");
        let body = if status < 200 || status == 204 || status == 304 {
            Vec::new()
        } else if chunked {
            let end = raw.windows(5).position(|w| w == b"0\r\n\r\n")? + 5;
            let body = common::dechunk(&raw[..end])?;
            raw = &raw[end..];
            body
        } else {
            let len: usize = headers
                .iter()
                .find(|(k, _)| k == "content-length")?
                .1
                .parse()
                .ok()?;
            if raw.len() < len {
                return None;
            }
            let body = raw[..len].to_vec();
            raw = &raw[len..];
            body
        };
        out.push(Wire {
            status_line,
            status,
            headers,
            body,
        });
    }
    Some(out)
}

fn request(version: &str, path: &str, extra: &str) -> Vec<u8> {
    format!("GET {path} {version}\r\nHost: x\r\n{extra}\r\n").into_bytes()
}

/// Sendet einen Request, liest bis zum Schließen und zerlegt die Antworten.
async fn exchange(site: &TestSite, version: &str, path: &str) -> Vec<Wire> {
    let mut stream = TcpStream::connect(site.addr()).await.unwrap();
    stream
        .write_all(&request(version, path, "Connection: close\r\n"))
        .await
        .unwrap();
    let (raw, _) = read_all(&mut stream).await;
    split_responses(&raw)
        .unwrap_or_else(|| panic!("unvollständig: {:?}", String::from_utf8_lossy(&raw)))
}

// spec: TECH-007
#[tokio::test]
async fn informational_103_comes_before_the_final_response_with_its_headers() {
    let site = site().await;
    let r = exchange(&site, "HTTP/1.1", "/hints").await;
    assert_eq!(r.len(), 2, "{r:?}");
    assert_eq!(r[0].status_line, "HTTP/1.1 103 Early Hints");
    assert!(r[0].body.is_empty());
    assert_eq!(
        r[0].header_all("link"),
        vec![
            "</a.css>; rel=preload; as=style",
            "</b.js>; rel=preload; as=script"
        ],
        "Array erzeugt mehrere gleichnamige Header in der Zwischenantwort"
    );
    assert_eq!(r[1].status_line, "HTTP/1.1 200 OK");
    assert_eq!(r[1].body, b"final body");
    assert!(
        r[1].header_all("link").is_empty(),
        "die Header der 103-Antwort gehören nicht zur finalen Antwort"
    );
    // Die Zwischenantwort ist kein eigener Request.
    assert_eq!(site.requests().len(), 1);
}

// spec: TECH-007
#[tokio::test]
async fn informational_without_headers_sends_a_bare_103() {
    let site = site().await;
    let r = exchange(&site, "HTTP/1.1", "/hints-no-headers").await;
    assert_eq!(r.len(), 2);
    assert_eq!((r[0].status, r[0].headers.len()), (103, 0));
    assert_eq!(
        (r[1].status, r[1].body.as_slice()),
        (200, &b"final body"[..])
    );
}

// spec: TECH-007
#[tokio::test]
async fn several_codes_are_sent_in_order_and_each_carries_the_interim_headers() {
    let site = site().await;
    let r = exchange(&site, "HTTP/1.1", "/multi").await;
    let codes: Vec<u16> = r.iter().map(|w| w.status).collect();
    assert_eq!(codes, vec![102, 103, 404]);
    assert_eq!(r[0].status_line, "HTTP/1.1 102 Processing");
    assert_eq!(r[0].header_all("x-interim"), vec!["1"]);
    assert_eq!(r[1].header_all("x-interim"), vec!["1"]);
    assert!(r[2].header_all("x-interim").is_empty());
    assert_eq!(r[2].body, b"not found");
}

// spec: TECH-007
#[tokio::test]
async fn routes_without_informational_send_no_interim_response() {
    let site = site().await;
    for path in ["/plain", "/nocontent", "/chunked", "/missing"] {
        let r = exchange(&site, "HTTP/1.1", path).await;
        assert_eq!(r.len(), 1, "{path}: {r:?}");
        assert!(r[0].status >= 200, "{path}");
    }
}

// spec: TECH-007
#[tokio::test]
async fn http_1_0_clients_get_no_interim_response() {
    let site = site().await;
    let mut stream = TcpStream::connect(site.addr()).await.unwrap();
    stream
        .write_all(&request("HTTP/1.0", "/hints", ""))
        .await
        .unwrap();
    let (raw, _) = read_all(&mut stream).await;
    let r = split_responses(&raw).expect("vollständig");
    assert_eq!(r.len(), 1, "RFC 9110, 15.2: keine 1xx an HTTP/1.0: {r:?}");
    assert_eq!(
        (r[0].status, r[0].body.as_slice()),
        (200, &b"final body"[..])
    );
}

// spec: TECH-007
#[tokio::test]
async fn keep_alive_connection_stays_in_step_after_an_interim_response() {
    let site = site().await;
    let mut stream = TcpStream::connect(site.addr()).await.unwrap();
    stream
        .write_all(&request("HTTP/1.1", "/hints", ""))
        .await
        .unwrap();
    // Auf die finale Antwort warten (die Verbindung bleibt offen).
    let mut raw = Vec::new();
    let first = loop {
        let mut chunk = [0u8; 4096];
        let n = stream.read(&mut chunk).await.unwrap();
        assert!(n > 0, "Verbindung unerwartet geschlossen");
        raw.extend_from_slice(&chunk[..n]);
        // 103 und 200 können in getrennten Segmenten ankommen: erst bei der finalen Antwort weiter.
        if let Some(r) = split_responses(&raw).filter(|r| r.iter().any(|w| w.status >= 200)) {
            break r;
        }
    };
    assert_eq!(
        first.iter().map(|w| w.status).collect::<Vec<_>>(),
        [103, 200]
    );
    // Zweiter Request auf derselben Verbindung: Antwort ohne Rest der ersten.
    stream
        .write_all(&request("HTTP/1.1", "/plain", "Connection: close\r\n"))
        .await
        .unwrap();
    let (rest, _) = read_all(&mut stream).await;
    let second = split_responses(&rest).expect("vollständig");
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].body, b"plain body");
    assert_eq!(site.connections(), 1);
}

// spec: TECH-007
#[tokio::test]
async fn interim_responses_also_work_over_tls_and_with_chunked_bodies() {
    let site = site().await;
    let mut stream = tls_connect(&site, site.addr(), "site.test").await.unwrap();
    stream
        .write_all(&request(
            "HTTP/1.1",
            "/chunked-hints",
            "Connection: close\r\n",
        ))
        .await
        .unwrap();
    let (raw, _) = read_all(&mut stream).await;
    let all = split_responses(&raw).expect("vollständig");
    assert_eq!(all.iter().map(|w| w.status).collect::<Vec<_>>(), [103, 200]);
    assert_eq!(all[1].body.len(), 300);
    assert_eq!(all[1].header_all("transfer-encoding"), vec!["chunked"]);
}

// spec: TECH-007
#[test]
fn scenario_validation_rejects_impossible_informational_options() {
    let bad = |toml: &str| Scenario::parse(toml).unwrap().validate();
    for code in [101, 200, 99, 404] {
        let err = bad(&format!(
            "[[route]]\npath = \"/x\"\ninformational = [{code}]"
        ))
        .unwrap_err();
        assert!(err.contains("/x") && err.contains("informational"), "{err}");
    }
    let err =
        bad("[[route]]\npath = \"/x\"\n[route.informational_headers]\nLink = \"a\"").unwrap_err();
    assert!(err.contains("informational_headers"), "{err}");
    // gültig: 100, 102, 103 und Header als Text oder Array
    bad("[[route]]\npath = \"/x\"\ninformational = [100, 102, 103]\n[route.informational_headers]\nLink = [\"a\", \"b\"]")
        .unwrap();
}
