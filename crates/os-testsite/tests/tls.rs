mod common;

use common::{get, get_tls, send_and_read, tls_connect};
use os_testsite::TestSite;
use tokio::io::AsyncWriteExt;

async fn site() -> TestSite {
    TestSite::start("testsite-hosts").await.unwrap()
}

// spec: TECH-007
#[tokio::test]
async fn http_and_https_share_one_port() {
    let site = site().await;
    let plain = get(&site, "site.test", "/").await;
    assert!(plain.text().contains("host:site.test"));
    let tls = get_tls(&site, "site.test", "/").await.unwrap();
    assert_eq!(tls.status, 200);
    assert!(tls.text().contains("host:site.test"));
    // Beide Requests gingen an denselben Port; das Log unterscheidet sie.
    let log = site.requests();
    assert_eq!(log.len(), 2);
    assert!(!log[0].tls && log[1].tls);
    assert_ne!(log[0].conn_id, log[1].conn_id);
    assert!(site.https_base_url().starts_with("https://127.0.0.1:"));
}

// spec: TECH-007
#[tokio::test]
async fn certificates_are_issued_per_sni_name_by_the_test_ca() {
    let site = site().await;
    let pem = site.ca_cert_pem();
    assert!(pem.starts_with("-----BEGIN CERTIFICATE-----"));
    assert!(!site.ca_cert_der().is_empty());
    // Beliebige Namen werden akzeptiert, solange sie auf Port und CA treffen.
    for name in ["site.test", "other.test", "deep.sub.site.test"] {
        let r = get_tls(&site, name, "/").await.unwrap();
        assert_eq!(r.status, 200, "{name}");
    }
    // Ohne SNI (IP-Adresse) liefert die Site ein Zertifikat mit IP-SAN.
    let r = get_tls(&site, "127.0.0.1", "/").await.unwrap();
    assert_eq!(r.status, 200);
}

// spec: TECH-007
#[tokio::test]
async fn expired_host_gets_an_expired_certificate() {
    let site = site().await;
    let Err(err) = tls_connect(&site, site.addr(), "expired.site.test").await else {
        panic!("Handshake muss am abgelaufenen Zertifikat scheitern");
    };
    assert!(
        err.to_string().to_ascii_lowercase().contains("expired"),
        "{err}"
    );
    // Der gleiche Host ohne Präfix ist gültig.
    assert_eq!(get_tls(&site, "site.test", "/").await.unwrap().status, 200);
}

// spec: TECH-007
#[tokio::test]
async fn foreign_ca_is_rejected() {
    let site = site().await;
    let other = TestSite::start("testsite-hosts").await.unwrap();
    // Jede Site hat ihre eigene CA: der Client der anderen Site vertraut dem Zertifikat nicht.
    let connector = tokio_rustls::TlsConnector::from(other.tls_client_config().unwrap());
    let tcp = tokio::net::TcpStream::connect(site.addr()).await.unwrap();
    let name = rustls::pki_types::ServerName::try_from("site.test").unwrap();
    let Err(err) = connector.connect(name, tcp).await else {
        panic!("Zertifikat der fremden CA darf nicht akzeptiert werden");
    };
    // Gleicher CA-Name, anderer Schlüssel: die Signatur passt nicht (sonst unbekannter Aussteller).
    let msg = err.to_string();
    assert!(
        msg.contains("BadSignature") || msg.contains("UnknownIssuer"),
        "{msg}"
    );
}

// spec: TECH-007
#[tokio::test]
async fn keep_alive_over_tls_and_clear_text_garbage() {
    let site = site().await;
    let mut stream = tls_connect(&site, site.addr(), "site.test").await.unwrap();
    for _ in 0..2 {
        stream
            .write_all(b"GET / HTTP/1.1\r\nHost: site.test\r\n\r\n")
            .await
            .unwrap();
        let mut head = vec![0u8; 0];
        use tokio::io::AsyncReadExt;
        let mut buf = [0u8; 4096];
        while !String::from_utf8_lossy(&head).contains("</p>") {
            let n = stream.read(&mut buf).await.unwrap();
            assert!(n > 0);
            head.extend_from_slice(&buf[..n]);
        }
        assert!(String::from_utf8_lossy(&head).starts_with("HTTP/1.1 200"));
    }
    assert_eq!(site.connections(), 1);
    assert_eq!(site.requests().len(), 2);

    // Ein kaputter Handshake (erstes Byte 0x16, danach Müll) beendet nur diese Verbindung.
    let mut raw = tokio::net::TcpStream::connect(site.addr()).await.unwrap();
    raw.write_all(&[0x16, 0x03, 0x01, 0x00, 0x02, 0xff, 0xff])
        .await
        .unwrap();
    let (_, _, _) = send_and_read(&mut raw, "x", "/").await;
    assert_eq!(get(&site, "site.test", "/").await.status, 200);
}
