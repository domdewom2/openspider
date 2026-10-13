mod common;

use common::{get, get_at};
use os_testsite::TestSite;

async fn site() -> TestSite {
    TestSite::start("testsite-hosts").await.unwrap()
}

// spec: TECH-007
#[tokio::test]
async fn placeholders_base_url_port_and_port2() {
    let site = site().await;
    let r = get(&site, "127.0.0.1", "/").await;
    assert_eq!(r.status, 200);
    let text = r.text();
    assert!(text.contains(&format!(
        "<p id=\"base\">http://127.0.0.1:{}</p>",
        site.port()
    )));
    assert!(text.contains(&format!("<p id=\"port\">{}</p>", site.port())));
    assert!(text.contains(&format!("<p id=\"port2\">{}</p>", site.port2())));
    assert_ne!(site.port(), site.port2());
    assert!(!text.contains("{{"));
}

// spec: TECH-007
#[tokio::test]
async fn second_listener_serves_the_same_site() {
    let site = site().await;
    let r = get_at(site.addr2(), "127.0.0.1", "/", &[]).await;
    assert_eq!(r.status, 200);
    assert!(r
        .text()
        .contains(&format!("<p id=\"port2\">{}</p>", site.port2())));
    assert_eq!(
        site.base_url2(),
        format!("http://127.0.0.1:{}", site.port2())
    );
    // Beide Listener schreiben in dasselbe Log.
    assert_eq!(site.count("127.0.0.1", "/"), 1);
}

// spec: TECH-007
#[tokio::test]
async fn host_directories_select_content_by_host_header() {
    let site = site().await;
    let a = get(&site, "site.test", "/").await;
    assert_eq!(a.status, 200);
    assert!(a.text().contains("host:site.test"));
    // Platzhalter gelten auch in Host-Verzeichnissen.
    assert!(a.text().contains(&format!("port {}", site.port())));
    let b = get(&site, "other.test", "/").await;
    assert!(b.text().contains("host:other.test"));
    // Port im Host-Header und Groß-/Kleinschreibung spielen keine Rolle.
    let c = get(&site, &format!("Site.Test:{}", site.port()), "/").await;
    assert!(c.text().contains("host:site.test"));
    // Unbekannte Hosts und die Loopback-Adressen bedienen das Wurzelverzeichnis.
    for host in ["127.0.0.1", "localhost", "unknown.test", ""] {
        let r = get(&site, host, "/").await;
        assert!(r.text().contains("id=\"base\""), "{host}: {}", r.text());
    }
}

// spec: TECH-007
#[tokio::test]
async fn host_directories_are_isolated() {
    let site = site().await;
    assert_eq!(get(&site, "127.0.0.1", "/only-root.html").await.status, 200);
    // Ein Host mit eigenem Verzeichnis sieht die Wurzeldateien nicht.
    assert_eq!(get(&site, "site.test", "/only-root.html").await.status, 404);
    // Die Host-Verzeichnisse sind nicht als Pfad auf dem Wurzel-Host erreichbar.
    assert_eq!(
        get(&site, "127.0.0.1", "/hosts/site.test/index.html")
            .await
            .status,
        404
    );
}

// spec: TECH-007
#[tokio::test]
async fn routes_bind_to_hosts_with_precedence() {
    let site = site().await;
    assert_eq!(
        get(&site, "site.test", "/who").await.text(),
        "route:site.test"
    );
    assert_eq!(
        get(&site, "a.site.test", "/who").await.text(),
        "route:wildcard"
    );
    assert_eq!(
        get(&site, "x.y.site.test", "/who").await.text(),
        "route:wildcard"
    );
    // Der Wildcard passt nicht auf den Apex, hostlose Routen gelten für alle übrigen.
    assert_eq!(get(&site, "127.0.0.1", "/who").await.text(), "route:any");
    assert_eq!(get(&site, "notsite.test", "/who").await.text(), "route:any");
    // Hostgebundene Route ohne Treffer: andere Hosts sehen sie nicht.
    assert_eq!(
        get(&site, "other.test", "/only-other").await.text(),
        "only other"
    );
    assert_eq!(get(&site, "site.test", "/only-other").await.status, 404);
    assert_eq!(get(&site, "127.0.0.1", "/only-other").await.status, 404);
}

// spec: TECH-007
#[tokio::test]
async fn listens_on_ipv6_loopback_with_the_same_port() {
    let site = site().await;
    let Some(addr6) = site.addr6() else {
        eprintln!("kein IPv6-Loopback auf diesem System – Test übersprungen");
        return;
    };
    assert_eq!(addr6.port(), site.port());
    assert!(addr6.ip().is_loopback() && addr6.is_ipv6());
    let r = get_at(addr6, &format!("[::1]:{}", addr6.port()), "/", &[]).await;
    assert_eq!(r.status, 200);
    assert!(r.text().contains("id=\"base\""));
    // Der Host-Header mit IPv6-Literal landet im Log ohne Port.
    assert_eq!(site.count("[::1]", "/"), 1);
}

// spec: TECH-007
#[tokio::test]
async fn absolute_form_target_selects_host() {
    let site = site().await;
    let r = get_at(site.addr(), "ignored.example", "http://site.test/", &[]).await;
    // RFC 9112: bei absoluter Zielform gilt deren Host, der Host-Header wird ignoriert.
    assert_eq!(r.status, 200);
    assert!(r.text().contains("host:site.test"));
    assert_eq!(site.count("site.test", "/"), 1);
}
