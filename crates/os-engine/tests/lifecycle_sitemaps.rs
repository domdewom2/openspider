//! Fixture `crawl-sitemaps` (CRAWL-058/059/060, TECH-210, K-41): jede Sitemap-Datei genau einmal und nur über den
//! Sitemap-Ingest, Weiterleitungen bis 5 Hops, Index-Tiefe ab Seed 0 (Dateien bis Tiefe 3, darüber Warnung als
//! Ereignis `kind = warn`), Einträge fremder Hosts nicht übernommen, Sitemap-hreflang mit `source = 'sitemap'`.

#[path = "lifecycle_support.rs"]
mod support;

use os_model::CrawlConfig;
use os_testsite::TestSite;
use support::*;

async fn crawl(sitemaps: &[&str]) -> (TestSite, tempfile::TempDir, std::path::PathBuf) {
    let site = TestSite::start("crawl-sitemaps").await.unwrap();
    let work = tempfile::tempdir().unwrap();
    let path = crawl_file_in(&work, "crawl.ospider");
    let mut cfg: CrawlConfig = config_for(&site, 2);
    cfg.sitemaps.crawl_linked = true;
    cfg.sitemaps.urls = sitemaps.iter().map(|s| url(s)).collect();
    run_to_end(start(cfg, &path, deps()).await).await;
    (site, work, path)
}

fn count(site: &TestSite, path: &str) -> usize {
    site.requests().iter().filter(|r| r.path() == path).count()
}

// spec: TECH-210
#[tokio::test]
async fn robots_and_configured_sitemaps_are_each_fetched_once_and_orphans_are_crawled() {
    let (site, _work, path) = crawl(&["/extra-sitemap.xml", "/pages.xml"]).await;
    // robots.txt nennt /sitemap_index.xml (relativ) und news.xml; pages.xml steht zusätzlich in der Konfiguration
    for p in [
        "/sitemap_index.xml",
        "/pages.xml",
        "/pages2.xml.gz",
        "/news.xml",
        "/extra-sitemap.xml",
    ] {
        assert_eq!(count(&site, p), 1, "{p} genau einmal abgerufen");
    }
    let file = open_ro(&path);
    // Sitemap-Dateien stehen in `sitemaps`, nicht in den Tabellen
    assert_eq!(scalar(&file, "SELECT COUNT(*) FROM sitemaps"), 5);
    assert_eq!(
        scalar(
            &file,
            "SELECT COUNT(*) FROM urls u JOIN sitemaps s ON s.url_id = u.id WHERE u.in_tables = 1"
        ),
        0
    );
    // die verwaiste URL: gecrawlt, Tiefe leer, nur über Sitemap entdeckt, keine Inlinks
    let (depth, via, status, inlinks): (Option<i64>, String, i64, i64) = file
        .conn()
        .query_row(
            "SELECT crawl_depth, discovered_via, status_code, inlinks FROM urls WHERE url = 'http://site.test/nur-in-sitemap'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(
        (depth, via.as_str(), status, inlinks),
        (None, "sitemap", 200, 0)
    );
    // der Eintrag eines fremden Hosts wurde nicht übernommen
    assert_eq!(
        scalar(
            &file,
            "SELECT COUNT(*) FROM urls WHERE url LIKE '%other.test%'"
        ),
        0
    );
    assert_eq!(count(&site, "/extern"), 0);
    // gz-Sitemap wurde gelesen
    assert_eq!(
        scalar(
            &file,
            "SELECT status_code FROM urls WHERE url = 'http://site.test/gz-seite'"
        ),
        200
    );
    assert_eq!(duplicate_requests(&site), vec![]);
}

// spec: CRAWL-060, TECH-210
#[tokio::test]
async fn nested_indexes_are_followed_to_depth_three_and_deeper_ones_give_a_warning() {
    let (site, _work, path) = crawl(&["/index-nested.xml", "/index-too-deep.xml"]).await;
    // index-nested: Tiefe 0 … 3 – alle vier Dateien, der Eintrag der letzten steht in der Warteschlange
    for p in [
        "/index-nested.xml",
        "/index-nested-1.xml",
        "/index-nested-2.xml",
        "/index-nested-3.xml",
    ] {
        assert_eq!(count(&site, p), 1, "{p}");
    }
    let file = open_ro(&path);
    assert_eq!(
        scalar(
            &file,
            "SELECT status_code FROM urls WHERE url = 'http://site.test/tief-3'"
        ),
        200
    );
    // index-too-deep: Dateien der Tiefe 0 … 3 werden abgerufen, die Datei der Tiefe 4 nicht
    for p in [
        "/index-too-deep.xml",
        "/index-too-deep-1.xml",
        "/index-too-deep-2.xml",
        "/index-too-deep-3.xml",
    ] {
        assert_eq!(count(&site, p), 1, "{p}");
    }
    assert_eq!(
        count(&site, "/index-too-deep-4.xml"),
        0,
        "Tiefe 4 liegt über der Grenze"
    );
    assert_eq!(
        scalar(&file, "SELECT COUNT(*) FROM urls WHERE url LIKE '%/tief-4'"),
        0
    );
    let warn: String = file
        .conn()
        .query_row(
            "SELECT message FROM crawl_events WHERE kind = 'warn' AND code = 'sitemap_index_too_deep'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        warn.contains("index-too-deep-3"),
        "die Warnung nennt den Index der Tiefe 3: {warn}"
    );
    let depths: Vec<i64> = {
        let mut stmt = file
            .conn()
            .prepare("SELECT depth FROM sitemaps WHERE url LIKE '%index-too-deep%' ORDER BY depth")
            .unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert_eq!(depths, vec![0, 1, 2, 3]);
}

// spec: TECH-210
#[tokio::test]
async fn sitemap_redirects_are_followed_and_loops_do_not_hang_the_crawl() {
    let (site, _work, path) = crawl(&["/sitemap-redirect.xml", "/redirect-loop.xml"]).await;
    // die Weiterleitung führt auf /pages.xml: genau ein Abruf der Datei, der Eintrag /nur-in-sitemap kommt an
    assert_eq!(count(&site, "/sitemap-redirect.xml"), 1);
    assert_eq!(count(&site, "/pages.xml"), 1);
    let file = open_ro(&path);
    assert_eq!(
        scalar(
            &file,
            "SELECT status_code FROM urls WHERE url = 'http://site.test/nur-in-sitemap'"
        ),
        200
    );
    // die Schleife endet nach höchstens 5 Hops; jede URL der Schleife wird höchstens einmal je Hop angefragt
    assert!(count(&site, "/redirect-loop.xml") + count(&site, "/redirect-loop2.xml") <= 6);
}

// spec: TECH-210
#[tokio::test]
async fn sitemap_hreflang_is_stored_with_source_sitemap() {
    let (_site, _work, path) = crawl(&["/hreflang-sitemap.xml"]).await;
    let file = open_ro(&path);
    let rows = scalar(
        &file,
        "SELECT COUNT(*) FROM hreflang WHERE source = 'sitemap'",
    );
    assert_eq!(rows, 4, "zwei URLs mit je zwei Alternativen");
    assert_eq!(
        scalar(
            &file,
            "SELECT COUNT(DISTINCT sitemap_id) FROM hreflang WHERE source = 'sitemap'"
        ),
        1
    );
    // die Alternativen sind zugleich Einträge: /de/ und /en/ wurden gecrawlt
    assert_eq!(
        scalar(
            &file,
            "SELECT status_code FROM urls WHERE url = 'http://site.test/de/'"
        ),
        200
    );
    assert_eq!(
        scalar(
            &file,
            "SELECT COUNT(*) FROM hreflang WHERE source = 'sitemap' AND target_id IS NOT NULL"
        ),
        4
    );
}
