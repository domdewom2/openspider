//! Indexierbarkeitsmodell im echten Crawl (TECH-001, TECH-002, TECH-068): die Engine bewertet jede URL mit
//! `os_model::evaluate` und dem URL-Vergleich aus `os-url`; hier wird das Ergebnis aus der Crawl-Datei gelesen.
//!
//! Fixtures: `tech/indexability` (je Seite ein Grund der Prüfreihenfolge) und `tech/url-compare`
//! (Canonical-Varianten). Die Tab-Sicht auf dieselben Daten prüft `tab_canonicals.rs`.
//!
//! Golden-Dateien unter `tests/golden/tech/indexability/` (TECH-007) halten je Tab und Filter Adresse,
//! Indexability und Indexability Status fest; die Werte stehen vorab ausdrücklich in den Tests oben
//! (aus TECH-001 abgeleitet), die Goldens sichern darüber hinaus die Filterzugehörigkeit.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use os_analyze::defs::SortKey;
use os_analyze::views::{query_view, ColumnSelection, ViewQuery};
use os_engine::testing::{crawl_fixture, FixtureCrawl};
use os_model::{CrawlConfig, Value};

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

/// Konfiguration mit `advanced.respect_self_ref_meta_refresh = true` (K-57: der Standard ist aus).
fn respecting_self_refresh() -> CrawlConfig {
    let mut cfg = CrawlConfig::default();
    cfg.advanced.respect_self_ref_meta_refresh = true;
    cfg
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

// spec: TECH-001, TECH-068
#[tokio::test]
async fn meta_refresh_to_another_url_is_redirected_and_to_itself_is_not_indexable() {
    // „Self Referencing Meta Refresh“ nur mit der Option (K-57: Standard aus)
    let fx = crawl_fixture("tech/indexability", respecting_self_refresh()).await;
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

// spec: TECH-001, TECH-068
#[tokio::test]
async fn self_referencing_refresh_is_ignored_without_the_option() {
    // TECH-068, K-57: Standard (Option aus) → Selbst-Refresh ändert die Indexierbarkeit nicht.
    let fx = crawl_fixture("tech/indexability", CrawlConfig::default()).await;
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

// ---------------------------------------------------------------------------------------------------------------
// Golden-Dateien (TECH-007)

fn text(v: &Value) -> String {
    match v {
        Value::Text(t) => t.clone(),
        Value::Int(i) => i.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Null => String::new(),
    }
}

/// Zeilen einer Ansicht (Adresse aufsteigend), Basis-URL und externe Basis zu `{BASE}` bzw. `{EXT}` normalisiert.
fn view_rows(fx: &FixtureCrawl, tab: &str, filter: &str, columns: &[&str]) -> Vec<Vec<String>> {
    let base = fx.site.base_url();
    let ext = format!("http://localhost:{}", fx.site.port());
    let mut q = ViewQuery::new(tab, filter);
    q.columns = ColumnSelection::Named(columns.iter().map(|c| c.to_string()).collect());
    q.limit = 500;
    q.sort = vec![SortKey {
        column: "Address".to_string(),
        descending: false,
    }];
    let page = query_view(&fx.file, &CrawlConfig::default(), &q)
        .unwrap_or_else(|e| panic!("{tab}:{filter}: {e}"));
    assert_eq!(page.columns, columns, "Spalten von {tab}:{filter}");
    page.rows
        .iter()
        .map(|r| {
            r.iter()
                .map(|v| text(v).replace(&base, "{BASE}").replace(&ext, "{EXT}"))
                .collect()
        })
        .collect()
}

fn csv(columns: &[&str], rows: &[Vec<String>]) -> String {
    let mut out = format!("{}\n", columns.join(","));
    for r in rows {
        out.push_str(&r.join(","));
        out.push('\n');
    }
    out
}

fn assert_golden(name: &str, columns: &[&str], rows: &[Vec<String>]) {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "../../tests/golden/tech/indexability",
        name,
    ]
    .iter()
    .collect();
    os_testsite::golden::assert_golden(path, &csv(columns, rows));
}

fn addresses(rows: &[Vec<String>]) -> BTreeSet<String> {
    rows.iter().map(|r| r[0].clone()).collect()
}

const COLUMNS: &[&str] = &["Address", "Indexability", "Indexability Status"];

// spec: TECH-007, TECH-001
#[tokio::test]
async fn goldens_indexability_per_url_and_filter() {
    // das Golden hält die ganze Prüfreihenfolge fest, also auch „Self Referencing Meta Refresh“ (Option an)
    let fx = crawl_fixture("tech/indexability", respecting_self_refresh()).await;

    let internal = view_rows(&fx, "Internal", "All", COLUMNS);
    let external = view_rows(&fx, "Response Codes", "External All", COLUMNS);

    // Vorab die aus TECH-001 abgeleiteten Werte (je Grund der Prüfreihenfolge eine Zeile), danach erst das Golden.
    let row = |rows: &[Vec<String>], address: &str| -> (String, String) {
        let r = rows
            .iter()
            .find(|r| r[0] == address)
            .unwrap_or_else(|| panic!("{address} fehlt: {rows:#?}"));
        (r[1].clone(), r[2].clone())
    };
    for (address, status) in [
        ("{BASE}/ok.html", ""),
        ("{BASE}/blocked.html", "Blocked by Robots.txt"),
        ("{BASE}/aborted", "No Response"),
        ("{BASE}/moved", "Redirected"),
        ("{BASE}/gone", "Client Error"),
        ("{BASE}/error", "Server Error"),
        ("{BASE}/refresh-self.html", "Self Referencing Meta Refresh"),
        ("{BASE}/noindex.html", "noindex"),
        ("{BASE}/canonical-other.html", "Canonicalised"),
    ] {
        let wanted = if status.is_empty() {
            "Indexable"
        } else {
            "Non-Indexable"
        };
        assert_eq!(
            row(&internal, address),
            (wanted.to_string(), status.to_string()),
            "{address}"
        );
    }
    // Externe URLs: nur der Statuscode zählt (200 indexierbar, 404 nicht).
    assert_eq!(
        row(&external, "{EXT}/ext-ok.html"),
        ("Indexable".to_string(), String::new())
    );
    assert_eq!(
        row(&external, "{EXT}/ext-gone"),
        ("Non-Indexable".to_string(), "Client Error".to_string())
    );

    assert_golden("internal__all.csv", COLUMNS, &internal);
    assert_golden("response_codes__external_all.csv", COLUMNS, &external);

    // Filterzugehörigkeit: Canonicals:Canonicalised (Canonical zeigt auf andere URL, TECH-014) und
    // Directives:Noindex (Meta Robots und X-Robots-Tag mit `noindex`; `none` hat den eigenen Filter None).
    let canonicalised = view_rows(
        &fx,
        "Canonicals",
        "Canonicalised",
        &["Address", "Indexability Status"],
    );
    assert!(addresses(&canonicalised).contains("{BASE}/canonical-other.html"));
    assert_golden(
        "canonicals__canonicalised.csv",
        &["Address", "Indexability Status"],
        &canonicalised,
    );
    let noindex = view_rows(
        &fx,
        "Directives",
        "Noindex",
        &["Address", "Indexability Status"],
    );
    for path in ["/noindex.html", "/noindex.pdf", "/noindex-canonical.html"] {
        assert!(
            addresses(&noindex).contains(&format!("{{BASE}}{path}")),
            "{path} fehlt in Directives:Noindex"
        );
    }
    assert!(!addresses(&noindex).contains("{BASE}/ok.html"));
    let none = view_rows(&fx, "Directives", "None", &["Address"]);
    assert!(addresses(&none).contains("{BASE}/none.html"));
    assert_golden(
        "directives__noindex.csv",
        &["Address", "Indexability Status"],
        &noindex,
    );
}
