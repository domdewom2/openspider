//! Crawl-Bereich im Spider-Modus (CRAWL-026): intern/extern, Subdomains, Unterordner.
//!
//! Fixtures: `crawl-scope-basic` (Start auf `www.site.test`, K-21), `crawl-scope-subdomains` (zwei Läufe),
//! `crawl-scope-folder`. Die Hostnamen löst die Resolve-Regel `*.test=…` auf die Testsite auf.

#[path = "core_support.rs"]
mod support;

use os_engine::testing::{crawl_fixture_from, FixtureCrawl};
use os_model::CrawlConfig;
use support::*;

fn internal(fx: &FixtureCrawl, url: &str) -> bool {
    row_of(fx, url).is_internal
}

// spec: CRAWL-026
#[tokio::test]
async fn subdomain_scope_treats_other_hosts_as_external_and_keeps_scheme_and_port_changes_internal()
{
    let fx = crawl_fixture_from(
        "crawl-scope-basic",
        "http://www.site.test/",
        CrawlConfig::default(),
    )
    .await;
    let port2 = fx.site.port2();
    // intern: Start, Schemawechsel, anderer Port
    assert!(internal(&fx, "http://www.site.test/"));
    assert!(internal(&fx, "https://www.site.test/"));
    assert!(internal(&fx, &format!("http://www.site.test:{port2}/")));
    // extern: Stammdomain, Subdomain, fremde Domain
    for u in [
        "http://site.test/",
        "http://blog.site.test/",
        "http://other.test/",
    ] {
        assert!(!internal(&fx, u), "{u} muss extern sein");
    }
    // die internen Varianten wurden tatsächlich abgerufen (200)
    for u in [
        "http://www.site.test/".to_string(),
        "https://www.site.test/".to_string(),
        format!("http://www.site.test:{port2}/"),
    ] {
        assert_eq!(row_of(&fx, &u).status_code, Some(200), "{u}");
    }
    // Links der fremden Domain werden nie verfolgt, die Seite wird nicht geparst
    assert_eq!(count_requests(&fx, "other.test", "/secret-1"), 0);
    assert_eq!(count_requests(&fx, "other.test", "/secret-2"), 0);
    assert!(row(&fx, "http://other.test/secret-1").is_none());
    let other = row_of(&fx, "http://other.test/");
    let outlinks = scalar(
        &fx,
        &format!("SELECT COUNT(*) FROM links WHERE source_id = {}", other.id),
    );
    assert_eq!(
        outlinks, 0,
        "externe Seiten werden nie nach Links durchsucht"
    );
}

// spec: CRAWL-026
#[tokio::test]
async fn start_on_the_root_domain_makes_every_subdomain_internal() {
    let fx = crawl_fixture_from(
        "crawl-scope-subdomains",
        "http://site.test/",
        CrawlConfig::default(),
    )
    .await;
    for host in [
        "site.test",
        "www.site.test",
        "blog.site.test",
        "shop.site.test",
    ] {
        let r = row_of(&fx, &format!("http://{host}/"));
        assert!(r.is_internal, "{host} intern (Stammdomain-Ausnahme)");
        assert_eq!(r.status_code, Some(200), "{host} abgerufen");
    }
}

// spec: CRAWL-026
#[tokio::test]
async fn start_on_www_keeps_the_other_subdomains_external() {
    let fx = crawl_fixture_from(
        "crawl-scope-subdomains",
        "http://www.site.test/",
        CrawlConfig::default(),
    )
    .await;
    assert!(internal(&fx, "http://www.site.test/"));
    for host in ["site.test", "blog.site.test", "shop.site.test"] {
        assert!(!internal(&fx, &format!("http://{host}/")), "{host} extern");
        let r = row_of(&fx, &format!("http://{host}/"));
        let out = scalar(
            &fx,
            &format!("SELECT COUNT(*) FROM links WHERE source_id = {}", r.id),
        );
        assert_eq!(out, 0, "{host}: nicht weiterverfolgt");
    }
}

// spec: CRAWL-026
#[tokio::test]
async fn all_subdomains_option_makes_them_internal_even_from_www() {
    let mut cfg = CrawlConfig::default();
    cfg.scope.all_subdomains = true;
    let fx = crawl_fixture_from("crawl-scope-subdomains", "http://www.site.test/", cfg).await;
    for host in ["site.test", "blog.site.test", "shop.site.test"] {
        let r = row_of(&fx, &format!("http://{host}/"));
        assert!(r.is_internal, "{host}");
        assert_eq!(r.status_code, Some(200));
    }
}

// spec: CRAWL-026
#[tokio::test]
async fn subfolder_scope_crawls_only_below_the_start_folder() {
    let mut cfg = CrawlConfig::default();
    cfg.scope.subfolder_only = true;
    cfg.scope.crawl_outside_start_folder = false;
    let fx = crawl_fixture_from("crawl-scope-folder", "http://site.test/blog/", cfg).await;
    for p in ["/blog/", "/blog/a.html", "/blog/sub/b.html"] {
        let r = row_of(&fx, &url("site.test", p));
        assert!(r.is_internal && r.status_code == Some(200), "{p}: {r:?}");
    }
    for p in ["/", "/other/c.html"] {
        let r = row_of(&fx, &url("site.test", p));
        assert!(!r.is_internal, "{p} liegt außerhalb des Startordners");
    }
    // ohne die Option sind alle fünf Seiten intern
    let fx = crawl_fixture_from(
        "crawl-scope-folder",
        "http://site.test/blog/",
        CrawlConfig::default(),
    )
    .await;
    for p in ["/", "/other/c.html", "/blog/a.html"] {
        assert!(row_of(&fx, &url("site.test", p)).is_internal, "{p}");
    }
}
