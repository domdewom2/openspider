use os_testsite::TestSite;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Minimaler HTTP/1.1-Client: gibt (Statuscode, Header-Text, Body) zurück.
async fn get(site: &TestSite, path: &str) -> (u16, String, String) {
    let mut stream = TcpStream::connect(site.addr()).await.unwrap();
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        site.addr()
    );
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await.unwrap();
    let text = String::from_utf8_lossy(&raw).into_owned();
    let (head, body) = text.split_once("\r\n\r\n").unwrap();
    let status = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .unwrap();
    (status, head.to_ascii_lowercase(), body.to_string())
}

#[tokio::test]
async fn serves_static_page_on_random_port() {
    let site = TestSite::start("basic").await.unwrap();
    assert_ne!(site.addr().port(), 0);
    let (status, head, body) = get(&site, "/index.html").await;
    assert_eq!(status, 200);
    assert!(head.contains("content-type: text/html"));
    assert!(body.contains("<h1>Basic Site</h1>"));
    // Platzhalter wird ersetzt.
    assert!(body.contains(&format!("{}/", site.base_url())));
    assert!(!body.contains("{{BASE_URL}}"));
}

#[tokio::test]
async fn root_serves_index() {
    let site = TestSite::start("basic").await.unwrap();
    let (status, _, body) = get(&site, "/").await;
    assert_eq!(status, 200);
    assert!(body.contains("Basic Site"));
}

#[tokio::test]
async fn scenario_redirect_and_404() {
    let site = TestSite::start("basic").await.unwrap();

    let (status, head, _) = get(&site, "/old-page").await;
    assert_eq!(status, 301);
    assert!(head.contains("location: /about.html"));

    let (status, _, body) = get(&site, "/missing.html").await;
    assert_eq!(status, 404);
    assert!(body.contains("<h1>404</h1>"));

    let (status, _, _) = get(&site, "/does-not-exist").await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn serves_robots_and_sitemap() {
    let site = TestSite::start("basic").await.unwrap();
    let (status, _, body) = get(&site, "/robots.txt").await;
    assert_eq!(status, 200);
    assert!(body.contains(&format!("Sitemap: {}/sitemap.xml", site.base_url())));

    let (status, head, body) = get(&site, "/sitemap.xml").await;
    assert_eq!(status, 200);
    assert!(head.contains("content-type: application/xml"));
    assert_eq!(body.matches("<loc>").count(), 3);
}

#[tokio::test]
async fn blocks_traversal_and_scenario_file() {
    let site = TestSite::start("basic").await.unwrap();
    let (status, _, _) = get(&site, "/scenario.toml").await;
    assert_eq!(status, 404);
    let (status, _, _) = get(&site, "/../Cargo.toml").await;
    assert_eq!(status, 404);
}
