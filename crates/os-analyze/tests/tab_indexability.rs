//! Indexierbarkeitsmodell im echten Crawl (TECH-001, TECH-002, TECH-068): die Engine bewertet jede URL mit
//! `os_model::evaluate` und dem URL-Vergleich aus `os-url`; hier wird das Ergebnis aus der Crawl-Datei gelesen.
//!
//! Fixtures: `tech/indexability` (je Seite ein Grund der Prüfreihenfolge) und `tech/url-compare`
//! (Canonical-Varianten). Die Tab-Sicht auf dieselben Daten prüft `tab_canonicals.rs`.

use std::collections::BTreeMap;

use os_engine::testing::{crawl_fixture, FixtureCrawl};
use os_model::CrawlConfig;

/// (Indexability, Indexability Status) je Pfad; externe URLs tragen das Präfix `ext:`.
fn indexability(fx: &FixtureCrawl) -> BTreeMap<String, (String, String)> {
    let base = fx.site.base_url();
    let ext = format!("http://localhost:{}", fx.site.port());
    let mut stmt = fx
        .file
        .conn()
        .prepare("SELECT address, indexability, COALESCE(indexability_status, '') FROM urls WHERE state = 'done'")
        .unwrap();
    stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?.unwrap_or_default(),
            r.get::<_, String>(2)?,
        ))
    })
    .unwrap()
    .map(|r| {
        let (address, i, s) = r.unwrap();
        let key = match address.strip_prefix(&base) {
            Some(path) => path.to_string(),
            None => format!("ext:{}", address.strip_prefix(&ext).unwrap_or(&address)),
        };
        (key, (i, s))
    })
    .collect()
}

fn expect(map: &BTreeMap<String, (String, String)>, path: &str, status: &str) {
    let (i, s) = map
        .get(path)
        .unwrap_or_else(|| panic!("{path} fehlt in der Crawl-Datei: {map:#?}"));
    let wanted = if status.is_empty() {
        "Indexable"
    } else {
        "Non-Indexable"
    };
    assert_eq!(i, wanted, "Indexability von {path}");
    assert_eq!(s, status, "Indexability Status von {path}");
}

// spec: TECH-001
#[tokio::test]
async fn each_reason_of_the_check_order_has_its_page() {
    let fx = crawl_fixture("tech/indexability", CrawlConfig::default()).await;
    let m = indexability(&fx);

    expect(&m, "/", "");
    expect(&m, "/ok.html", "");
    expect(&m, "/robots.txt", "");
    expect(&m, "/blocked.html", "Blocked by Robots.txt");
    expect(&m, "/aborted", "No Response");
    expect(&m, "/moved", "Redirected");
    expect(&m, "/gone", "Client Error");
    expect(&m, "/error", "Server Error");
    expect(&m, "/noindex.html", "noindex");
    expect(&m, "/none.html", "noindex");
    expect(&m, "/noindex.pdf", "noindex");
    expect(&m, "/canonical-other.html", "Canonicalised");
    // Reihenfolge: noindex schlägt den fremden Canonical.
    expect(&m, "/noindex-canonical.html", "noindex");
}

// spec: TECH-001
#[tokio::test]
async fn meta_refresh_to_another_url_is_redirected_and_to_itself_is_not_indexable() {
    let fx = crawl_fixture("tech/indexability", CrawlConfig::default()).await;
    let m = indexability(&fx);

    // Ziel ≠ eigene URL (jede Verzögerung, Meta-Element und Header) → Redirected.
    expect(&m, "/refresh-other.html", "Redirected");
    expect(&m, "/refresh-header.html", "Redirected");
    // andere Query = andere URL; die Query-Variante verweist dann auf sich selbst
    expect(&m, "/refresh-query.html", "Redirected");
    expect(
        &m,
        "/refresh-query.html?x=1",
        "Self Referencing Meta Refresh",
    );
    // Selbst-Refresh: eigene URL (relativ, absolut, mit Fragment), ohne Ziel, Header-Variante.
    for path in [
        "/refresh-self.html",
        "/refresh-self-abs.html",
        "/refresh-bare.html",
        "/refresh-fragment.html",
        "/refresh-self-header.html",
    ] {
        expect(&m, path, "Self Referencing Meta Refresh");
    }
}

// spec: TECH-001
#[tokio::test]
async fn self_referencing_refresh_is_ignored_without_the_option() {
    // TECH-068: Option aus → Selbst-Refresh ändert die Indexierbarkeit nicht.
    let mut cfg = CrawlConfig::default();
    cfg.advanced.respect_self_ref_meta_refresh = false;
    let fx = crawl_fixture("tech/indexability", cfg).await;
    let m = indexability(&fx);

    for path in [
        "/refresh-self.html",
        "/refresh-self-abs.html",
        "/refresh-bare.html",
        "/refresh-fragment.html",
        "/refresh-self-header.html",
        "/refresh-query.html?x=1",
    ] {
        expect(&m, path, "");
    }
    // Weiterleitungen und die übrigen Gründe bleiben.
    expect(&m, "/refresh-other.html", "Redirected");
    expect(&m, "/refresh-query.html", "Redirected");
    expect(&m, "/noindex.html", "noindex");
}

// spec: TECH-001
#[tokio::test]
async fn external_urls_are_judged_by_their_status_only() {
    let fx = crawl_fixture("tech/indexability", CrawlConfig::default()).await;
    let m = indexability(&fx);

    // Ein externer 200 ist indexierbar – auch mit noindex im (nie geladenen) HTML.
    expect(&m, "ext:/ext-ok.html", "");
    expect(&m, "ext:/ext-noindex.html", "");
    expect(&m, "ext:/ext-gone", "Client Error");
}

// spec: TECH-001
#[tokio::test]
async fn canonicals_that_are_not_stored_never_canonicalise() {
    // CRAWL-046: Canonicals speichern aus → kein Status „Canonicalised“.
    let mut cfg = CrawlConfig::default();
    cfg.spider.canonicals.store = false;
    let fx = crawl_fixture("tech/indexability", cfg).await;
    let m = indexability(&fx);

    expect(&m, "/canonical-other.html", "");
    expect(&m, "/noindex-canonical.html", "noindex");
}

// spec: TECH-002, TECH-001
#[tokio::test]
async fn canonical_variants_are_compared_after_url_normalisation() {
    let fx = crawl_fixture("tech/url-compare", CrawlConfig::default()).await;
    let m = indexability(&fx);

    // gleich nach TECH-002: relativ, `../`, Wurzelpfad, Schema in Großbuchstaben, absolut, Fragment, leerer href
    for path in [
        "/u/rel.html",
        "/u/dotdot.html",
        "/u/root.html",
        "/u/abs-scheme-upper.html",
        "/u/explicit-slash.html",
        "/u/fragment.html",
        "/u/empty.html",
    ] {
        expect(&m, path, "");
    }
    // verschieden: Slash am Ende, Query, `<base href>` mit anderem Ordner
    expect(&m, "/u/trailing-slash.html", "Canonicalised");
    expect(&m, "/u/query.html", "Canonicalised");
    expect(&m, "/u/base.html", "Canonicalised");
    // ungültige URL: nie ein Treffer, die Seite bleibt indexierbar
    expect(&m, "/u/broken.html", "");
}
