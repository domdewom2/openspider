//! Parser-Tests gegen die Fixtures `crawl-robots` (CRAWL-098): die robots.txt wird vom Testserver
//! ausgeliefert und aus der HTTP-Antwort gelesen.

use os_fetch::robots::{parse, RobotsMatch, RobotsTxt};
use os_testsite::client::get_once;
use os_testsite::{fixtures_root, TestSite};

/// Liefert die ausgelieferte robots.txt eines Fixture-Verzeichnisses (relativ zu `tests/fixtures`).
async fn served(dir: &str) -> (TestSite, String) {
    let site = TestSite::start_dir(fixtures_root().join(dir), 0)
        .await
        .expect("Testserver");
    let resp = get_once(site.addr(), "/robots.txt").await.expect("GET");
    assert_eq!(resp.status, 200);
    (site, resp.text())
}

fn eval(txt: &RobotsTxt, token: &str, path: &str) -> Option<RobotsMatch> {
    txt.evaluate(token, path)
}

fn blocked_at(txt: &RobotsTxt, token: &str, path: &str) -> Option<String> {
    eval(txt, token, path)
        .filter(|m| !m.allowed)
        .map(|m| m.matched_line())
}

// spec: CRAWL-098
#[tokio::test]
async fn fixture_group_selection() {
    let (_site, body) = served("crawl-robots").await;
    let txt = parse(&body);
    // OpenSpider (beliebige Schreibweise): eigene Gruppe, `*` und Googlebot gelten nicht.
    for token in ["OpenSpider", "openspider", "OPENSPIDER"] {
        assert_eq!(eval(&txt, token, "/frei/a"), None, "{token}");
        assert_eq!(eval(&txt, token, "/nur-google/a"), None, "{token}");
    }
    // Googlebot: nur seine Gruppe.
    assert_eq!(
        blocked_at(&txt, "Googlebot", "/nur-google/a").as_deref(),
        Some("Zeile 7: Disallow: /nur-google/")
    );
    assert_eq!(eval(&txt, "googlebot", "/privat/x"), None);
    // Unbekannter Agent: `*` (sperrt alles, Zeile 4).
    assert_eq!(
        blocked_at(&txt, "Irgendwer", "/frei/a").as_deref(),
        Some("Zeile 4: Disallow: /")
    );
}

// spec: CRAWL-098
#[tokio::test]
async fn fixture_longest_match_and_allow_on_tie() {
    let (_site, body) = served("crawl-robots").await;
    let txt = parse(&body);
    let t = "OpenSpider";
    assert_eq!(
        blocked_at(&txt, t, "/privat/x").as_deref(),
        Some("Zeile 10: Disallow: /privat/")
    );
    assert_eq!(
        blocked_at(&txt, t, "/privat/").as_deref(),
        Some("Zeile 10: Disallow: /privat/")
    );
    // Allow (Zeile 11) ist länger als Disallow /privat/ (Zeile 10).
    for p in ["/privat/oeffentlich", "/privat/oeffentlich/unterseite"] {
        let m = eval(&txt, t, p).expect(p);
        assert!(m.allowed, "{p}");
        assert_eq!(m.line_no, 11);
    }
    // `/privat` ohne Slash trifft kein Muster.
    assert_eq!(eval(&txt, t, "/privat"), None);
    // `Allow: /seite$` (7 Zeichen) schlägt `Disallow: /seite` (6) nur für genau /seite.
    let m = eval(&txt, t, "/seite").expect("/seite");
    assert!(m.allowed);
    assert_eq!(m.line_no, 14);
    assert_eq!(
        blocked_at(&txt, t, "/seite2").as_deref(),
        Some("Zeile 15: Disallow: /seite")
    );
    assert_eq!(
        blocked_at(&txt, t, "/seite/unterseite").as_deref(),
        Some("Zeile 15: Disallow: /seite")
    );
}

// spec: CRAWL-098
#[tokio::test]
async fn fixture_wildcards_anchor_and_query() {
    let (_site, body) = served("crawl-robots").await;
    let txt = parse(&body);
    let t = "OpenSpider";
    for p in ["/dokument.pdf", "/ordner/dokument.pdf"] {
        assert_eq!(
            blocked_at(&txt, t, p).as_deref(),
            Some("Zeile 12: Disallow: /*.pdf$"),
            "{p}"
        );
    }
    // `$` verankert am Ende des Pfads inklusive Query.
    assert_eq!(eval(&txt, t, "/dokument.pdf?download=1"), None);
    assert_eq!(eval(&txt, t, "/dokument.pdfx"), None);
    assert_eq!(
        blocked_at(&txt, t, "/liste?sessionid=abc").as_deref(),
        Some("Zeile 13: Disallow: /*?sessionid=")
    );
    assert_eq!(eval(&txt, t, "/liste?seite=2&sessionid=abc"), None);
    assert_eq!(eval(&txt, t, "/liste?seite=2"), None);
    // Prozentkodierte Pfade aus den Links.
    assert_eq!(eval(&txt, t, "/caf%C3%A9"), None);
}

// spec: CRAWL-098
#[tokio::test]
async fn fixture_merged_groups_unknown_directives_and_crawl_delay() {
    let (site, body) = served("crawl-robots").await;
    let txt = parse(&body);
    let t = "OpenSpider";
    // Zweite Gruppe `user-agent: openspider` (Zeile 22) wird mit der ersten zusammengeführt.
    assert_eq!(
        blocked_at(&txt, t, "/zweite-gruppe/a").as_deref(),
        Some("Zeile 23: Disallow: /zweite-gruppe/")
    );
    // Leerzeichen vor dem Doppelpunkt, Kommentar am Zeilenende.
    assert_eq!(
        blocked_at(&txt, t, "/mit-leerzeichen/a").as_deref(),
        Some("Zeile 16: Disallow : /mit-leerzeichen/")
    );
    // Unbekannte Direktiven (`Noindex`, `Host`) sperren nichts; Crawl-delay wird gelesen, nicht angewandt.
    assert_eq!(eval(&txt, t, "/unbekannte-direktive/a"), None);
    assert_eq!(txt.crawl_delay(t), Some(2.0));
    assert_eq!(eval(&txt, t, "/frei/a"), None);
    // Sitemap-Zeilen in Dateireihenfolge.
    let base = site.base_url();
    assert_eq!(
        txt.sitemaps(),
        [
            format!("{base}/sitemap.xml"),
            format!("{base}/sitemap-news.xml")
        ]
    );
}

// spec: CRAWL-098
#[tokio::test]
async fn bom_and_crlf_host_variant() {
    let (_site, body) = served("crawl-robots/hosts/bom.site.test").await;
    assert!(
        body.starts_with('\u{feff}') && body.contains("\r\n"),
        "Fixture hat BOM und CRLF"
    );
    let txt = parse(&body);
    assert_eq!(
        blocked_at(&txt, "OpenSpider", "/privat/x").as_deref(),
        Some("Zeile 2: Disallow: /privat/")
    );
    assert_eq!(
        eval(&txt, "OpenSpider", "/privat/oeffentlich").map(|m| m.allowed),
        Some(true)
    );
    assert_eq!(eval(&txt, "OpenSpider", "/frei"), None);
}

// spec: CRAWL-098
#[tokio::test]
async fn redirect_target_file_of_the_redirect_variant_parses() {
    // Die Datei, auf die `redirect.site.test/robots.txt` weiterleitet.
    let site = TestSite::start_dir(
        fixtures_root().join("crawl-robots/hosts/redirect.site.test"),
        0,
    )
    .await
    .expect("Testserver");
    let first = get_once(site.addr(), "/robots.txt").await.expect("GET");
    assert_eq!(first.status, 301);
    assert_eq!(first.header("location"), Some("/robots-real.txt"));
    let real = get_once(site.addr(), "/robots-real.txt")
        .await
        .expect("GET");
    let txt = parse(&real.text());
    assert_eq!(
        blocked_at(&txt, "OpenSpider", "/privat/x").as_deref(),
        Some("Zeile 2: Disallow: /privat/")
    );
}

// spec: CRAWL-098
#[test]
fn three_megabyte_robots_txt_does_not_crash() {
    // Entspricht dem Host big.site.test (3 MB); der Inhalt wird hier erzeugt statt eingecheckt.
    let mut text = String::from("User-agent: *\nDisallow: /privat/\n");
    let mut i = 0u64;
    while text.len() < 3 * 1024 * 1024 {
        text.push_str(&format!("Disallow: /sperre-{i}/*/x$\n"));
        i += 1;
    }
    let txt = parse(&text);
    assert!(txt
        .evaluate("OpenSpider", "/privat/x")
        .is_some_and(|m| !m.allowed));
    assert!(txt
        .evaluate("OpenSpider", "/sperre-0/a/x")
        .is_some_and(|m| !m.allowed));
    // Regeln hinter der 500-KiB-Grenze gelten nicht mehr.
    assert_eq!(
        txt.evaluate("OpenSpider", &format!("/sperre-{}/a/x", i - 1)),
        None
    );
}
