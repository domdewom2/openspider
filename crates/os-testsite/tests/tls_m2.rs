use os_testsite::TestSite;
use std::sync::Arc;

// spec: CRAWL-024
#[tokio::test]
async fn wire_modes_force_http1_for_the_entire_scenario() {
    for mode in ["chunked", "trickle", "abort", "reset", "informational"] {
        let site = TestSite::start(&format!("crawl-m2-http/wire-{mode}"))
            .await
            .unwrap();
        let mut cfg = (*site.tls_client_config().unwrap()).clone();
        cfg.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
        let connector = tokio_rustls::TlsConnector::from(Arc::new(cfg));
        let tcp = tokio::net::TcpStream::connect(site.addr()).await.unwrap();
        let tls = connector
            .connect(
                rustls::pki_types::ServerName::try_from("site.test").unwrap(),
                tcp,
            )
            .await
            .unwrap();
        assert_eq!(
            tls.get_ref().1.alpn_protocol(),
            Some(b"http/1.1".as_slice()),
            "{mode}"
        );
    }
}
