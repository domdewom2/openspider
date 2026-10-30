//! Tab „Canonicals“ (TECH-011 bis TECH-018) und Extraktion (TECH-010) gegen echte Crawls der Fixtures
//! `tech/canonical-basic`, `tech/canonical-extract`, `tech/url-compare` und `tech/multi-columns`.
//!
//! Die erwarteten Filtermengen stehen hier ausdrücklich im Test (aus den Akzeptanzkriterien der Spec
//! abgeleitet); die Golden-Dateien unter `tests/golden/tech/<slug>/` halten dieselben Mengen mit den
//! wichtigsten Spalten fest (TECH-007).

use std::collections::BTreeSet;
use std::path::PathBuf;

use os_analyze::analysis::{run, CancelToken};
use os_analyze::defs::SortKey;
use os_analyze::views::{filter_counts, query_view, ColumnSelection, ViewPage, ViewQuery};
use os_engine::testing::{crawl_fixture, crawl_fixture_from, FixtureCrawl};
use os_model::{AnalysisStep, CrawlConfig, FilterState, StepState, Value};
use os_store::queries;

const TAB: &str = "Canonicals";

// ---------------------------------------------------------------------------------------------------------------
// Hilfen

struct Crawled {
    fx: FixtureCrawl,
    cfg: CrawlConfig,
}

/// Crawlt das Szenario; mit `analyse` läuft danach die Crawl-Analyse (Schritt „Kanonisierungen“).
async fn crawl(scenario: &str, cfg: CrawlConfig, analyse: bool) -> Crawled {
    let fx = crawl_fixture(scenario, cfg.clone()).await;
    let c = Crawled { fx, cfg };
    if analyse {
        analyse_now(&c);
    }
    c
}

/// Wie `crawl`, aber ab einem Startpfad (Teilbaum `/n16/` von `tech/canonical-basic`).
async fn crawl_from(scenario: &str, start: &str, cfg: CrawlConfig) -> Crawled {
    let fx = crawl_fixture_from(scenario, start, cfg.clone()).await;
    let c = Crawled { fx, cfg };
    analyse_now(&c);
    c
}

fn analyse_now(c: &Crawled) {
    run(&c.fx.file, &c.cfg, &CancelToken::new(), &|_, _| {}).expect("Analyse läuft");
}

impl Crawled {
    /// Adresse ohne Basis: interne Pfade beginnen mit `/`, andere Ursprünge als `{EXT}` bzw. `{HTTPS}`.
    fn norm(&self, text: &str) -> String {
        let site = &self.fx.site;
        text.replace(&site.https_base_url(), "{HTTPS}")
            .replace(&site.base_url(), "")
            .replace(&format!("http://localhost:{}", site.port()), "{EXT}")
    }

    fn page(&self, filter: &str, columns: &[&str]) -> ViewPage {
        let mut q = ViewQuery::new(TAB, filter);
        q.columns = ColumnSelection::Named(columns.iter().map(|c| c.to_string()).collect());
        q.limit = 500;
        q.sort = vec![SortKey {
            column: "Address".to_string(),
            descending: false,
        }];
        query_view(&self.fx.file, &self.cfg, &q).unwrap_or_else(|e| panic!("{TAB}:{filter}: {e}"))
    }

    /// Adressen eines Filters, sortiert nach Address.
    fn addresses(&self, filter: &str) -> Vec<String> {
        self.page(filter, &["Address"])
            .rows
            .iter()
            .map(|r| self.norm(&text(&r[0])))
            .collect()
    }

    fn set(&self, filter: &str) -> BTreeSet<String> {
        self.addresses(filter).into_iter().collect()
    }

    /// Wert einer Spalte für eine Adresse. Nummerierte Spalten (`Basis n`) werden über ihre Basis angefragt; eine
    /// Nummer jenseits des Maximums der Ansicht ergibt einen leeren Text.
    fn cell(&self, filter: &str, address: &str, column: &str) -> Value {
        let (request, wanted) = match column.rsplit_once(' ') {
            Some((base, n)) if n.parse::<u32>().is_ok() && !column.starts_with("rel=") => {
                (base, column)
            }
            _ => (column, column),
        };
        let page = self.page(filter, &["Address", request]);
        let row = page
            .rows
            .iter()
            .find(|r| self.norm(&text(&r[0])) == address)
            .unwrap_or_else(|| panic!("{address} fehlt im Filter {filter}"));
        match page.columns.iter().position(|c| c == wanted) {
            Some(i) => row[i].clone(),
            None => Value::Text(String::new()),
        }
    }

    fn text_cell(&self, filter: &str, address: &str, column: &str) -> String {
        self.norm(&text(&self.cell(filter, address, column)))
    }

    /// Diagnoseausgabe für Assert-Meldungen (TECH-012-Flake): Endzustand des Crawls, Zeilen des Filters „All“ mit
    /// Indexierbarkeit und Occurrences sowie die Abrufe der Testsite je Pfad. Der Fehlertext eines gescheiterten
    /// Abrufs (`FetchOutcome::NoResponse.detail`) wird nicht gespeichert und ist hier nicht erreichbar.
    fn diagnostics(&self) -> String {
        let mut out = format!(
            "Crawl: state={:?} end_reason={:?} crawled={}\n",
            self.fx.summary.state, self.fx.summary.end_reason, self.fx.summary.crawled
        );
        let page = self.page(
            "All",
            &[
                "Address",
                "Occurrences",
                "Indexability",
                "Indexability Status",
            ],
        );
        for r in &page.rows {
            let cells: Vec<String> = r.iter().map(|v| self.norm(&text(v))).collect();
            out.push_str(&format!("  Zeile: {}\n", cells.join(" | ")));
        }
        for r in self.fx.site.requests() {
            out.push_str(&format!(
                "  Abruf {}: {} {} (Verbindung {})\n",
                r.seq, r.method, r.path_and_query, r.conn_id
            ));
        }
        out
    }
}

fn text(v: &Value) -> String {
    match v {
        Value::Text(t) => t.clone(),
        Value::Int(i) => i.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Null => String::new(),
    }
}

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn slug(filter: &str) -> String {
    filter.to_ascii_lowercase().replace([' ', '-'], "_")
}

/// Golden-Datei je Filter: Adresse, Occurrences, Indexability Status, maßgebliche Canonical-URL.
fn golden(c: &Crawled, dir: &str, filters: &[&str]) {
    for filter in filters {
        let page = c.page(
            filter,
            &[
                "Address",
                "Occurrences",
                "Indexability Status",
                "Canonical URL",
            ],
        );
        let mut out = String::from("Address,Occurrences,Indexability Status,Canonical URL\n");
        for r in &page.rows {
            let cells: Vec<String> = r.iter().map(|v| c.norm(&text(v))).collect();
            out.push_str(&cells.join(","));
            out.push('\n');
        }
        let path: PathBuf = [
            env!("CARGO_MANIFEST_DIR"),
            "../../tests/golden/tech",
            dir,
            &format!("canonicals__{}.csv", slug(filter)),
        ]
        .iter()
        .collect();
        os_testsite::golden::assert_golden(path, &out);
    }
}

const FILTERS: &[&str] = &[
    "All",
    "Contains Canonical",
    "Self Referencing",
    "Canonicalised",
    "Missing",
    "Multiple",
    "Multiple Conflicting",
    "Non-Indexable Canonical",
];

// ---------------------------------------------------------------------------------------------------------------
// canonical-basic

/// Interne Seiten mit Status 200 und HTML (plus `doc.pdf` mit HTTP-Canonical) in `tech/canonical-basic`.
const BASIC_ALL: &[&str] = &[
    "/",
    "/body-only.html",
    "/canonicalised-target.html",
    "/case/A",
    "/case/a",
    "/doc.pdf",
    "/excluded/noindex-target.html",
    "/fragment.html",
    "/head-self-body-other.html",
    "/header-only.html",
    "/header-over-head.html",
    "/invalid-url.html",
    "/multi-case.html",
    "/multi-conflict-mixed.html",
    "/multi-conflict.html",
    "/multi-fragment.html",
    "/multi-invalid.html",
    "/multi-mixed-same.html",
    "/multi-pct.html",
    "/multi-relabs.html",
    "/multi-same.html",
    "/noindex-target.html",
    "/noindex-with-canonical.html",
    "/none.html",
    "/none.pdf",
    "/pct/a~b.html",
    "/self-abs.html",
    "/self.html",
    "/slash.html",
    "/slash.html/",
    "/target-a.html",
    "/target-b.html",
    "/target.html",
    "/to-404.html",
    "/to-blocked.html",
    "/to-canonicalised.html",
    "/to-excluded.html",
    "/to-external-404.html",
    "/to-external.html",
    "/to-https.html",
    "/to-noindex.html",
    "/to-other.html",
    "/to-redirect.html",
    "{HTTPS}/to-https.html",
];

const BASIC_SELF: &[&str] = &[
    "/",
    "/case/A",
    "/case/a",
    "/excluded/noindex-target.html",
    "/fragment.html",
    "/head-self-body-other.html",
    "/header-only.html",
    "/noindex-target.html",
    "/pct/a~b.html",
    "/self-abs.html",
    "/self.html",
    "/slash.html/",
    "/target-a.html",
    "/target-b.html",
    "/target.html",
    "{HTTPS}/to-https.html",
];

const BASIC_CANONICALISED: &[&str] = &[
    "/canonicalised-target.html",
    "/doc.pdf",
    "/header-over-head.html",
    "/multi-case.html",
    "/multi-conflict-mixed.html",
    "/multi-conflict.html",
    "/multi-fragment.html",
    "/multi-invalid.html",
    "/multi-mixed-same.html",
    "/multi-pct.html",
    "/multi-relabs.html",
    "/multi-same.html",
    "/noindex-with-canonical.html",
    "/slash.html",
    "/to-404.html",
    "/to-blocked.html",
    "/to-canonicalised.html",
    "/to-excluded.html",
    "/to-external-404.html",
    "/to-external.html",
    "/to-https.html",
    "/to-noindex.html",
    "/to-other.html",
    "/to-redirect.html",
];

const BASIC_MULTIPLE: &[&str] = &[
    "/header-over-head.html",
    "/multi-case.html",
    "/multi-conflict-mixed.html",
    "/multi-conflict.html",
    "/multi-fragment.html",
    "/multi-invalid.html",
    "/multi-mixed-same.html",
    "/multi-pct.html",
    "/multi-relabs.html",
    "/multi-same.html",
];

const BASIC_CONFLICTING: &[&str] = &[
    "/header-over-head.html",
    "/multi-case.html",
    "/multi-conflict-mixed.html",
    "/multi-conflict.html",
];

const BASIC_NON_INDEXABLE: &[&str] = &[
    "/to-404.html",
    "/to-blocked.html",
    "/to-canonicalised.html",
    "/to-excluded.html",
    "/to-external-404.html",
    "/to-noindex.html",
    "/to-redirect.html",
];

// spec: TECH-011
#[tokio::test]
async fn population_is_internal_200_html_plus_non_html_with_http_canonical() {
    let c = crawl("tech/canonical-basic", CrawlConfig::default(), true).await;
    let all = c.set("All");
    assert_eq!(all, set(BASIC_ALL));

    // Nicht in der Grundmenge: Nicht-200 (404, 301), robots-gesperrt, robots.txt, extern.
    for gone in [
        "/gone-404",
        "/moved",
        "/blocked-target.html",
        "/robots.txt",
        "{EXT}/ext-ok.html",
        "{EXT}/ext-missing.html",
    ] {
        assert!(!all.contains(gone), "{gone} gehört nicht zur Grundmenge");
    }
    // Nicht-HTML-200 mit HTTP-Canonical ist dabei.
    assert!(all.contains("/doc.pdf"));
    // Filter „All“ = Grundmenge; jeder Filter ist eine Teilmenge.
    for filter in FILTERS {
        assert!(c.set(filter).is_subset(&all), "{filter} ⊆ All");
    }
}

// spec: TECH-011
#[tokio::test]
async fn columns_follow_the_spec_order_with_numbered_canonical_columns() {
    let c = crawl("tech/canonical-basic", CrawlConfig::default(), true).await;
    let mut q = ViewQuery::new(TAB, "All");
    q.limit = 500;
    let page = query_view(&c.fx.file, &c.cfg, &q).unwrap();
    // Höchstens zwei HTML-Elemente und ein Header-Eintrag je Seite (TECH-004): so viele Spalten wie das Maximum.
    assert_eq!(
        page.columns,
        [
            "Address",
            "Occurrences",
            "Indexability",
            "Indexability Status",
            "Canonical Link Element 1",
            "Canonical Link Element 2",
            "HTTP Canonical 1",
            "Meta Robots 1",
            "X-Robots-Tag 1",
            "rel=\"next\" 1",
            "rel=\"prev\" 1",
        ]
    );
    // Die OS-Erweiterung (maßgebliche Canonical-URL) steht nur bei „alle Spalten“, und zwar hinten.
    q.columns = ColumnSelection::All;
    let all = query_view(&c.fx.file, &c.cfg, &q).unwrap();
    assert_eq!(
        all.columns.last().map(String::as_str),
        Some("Canonical URL")
    );
}

// spec: TECH-012
#[tokio::test]
async fn contains_canonical_counts_html_and_header_canonicals() {
    let c = crawl("tech/canonical-basic", CrawlConfig::default(), true).await;
    let contains = c.set("Contains Canonical");
    let expected: BTreeSet<String> = set(BASIC_ALL)
        .into_iter()
        .filter(|a| a != "/none.html" && a != "/none.pdf" && a != "/body-only.html")
        .collect();
    assert_eq!(
        contains,
        expected,
        "Contains Canonical weicht ab; fehlend: {:?}, zusätzlich: {:?}\n{}",
        expected.difference(&contains).collect::<Vec<_>>(),
        contains.difference(&expected).collect::<Vec<_>>(),
        c.diagnostics()
    );
    // nur per HTTP-Header (HTML-Seite und PDF) sowie außerhalb des Heads: enthalten
    for a in ["/header-only.html", "/doc.pdf"] {
        assert!(contains.contains(a), "{a}\n{}", c.diagnostics());
    }
    assert_eq!(
        c.cell("Contains Canonical", "/header-only.html", "Occurrences"),
        Value::Int(1)
    );
}

// spec: TECH-013
#[tokio::test]
async fn self_referencing_compares_the_authoritative_canonical_after_normalisation() {
    let c = crawl("tech/canonical-basic", CrawlConfig::default(), true).await;
    assert_eq!(c.set("Self Referencing"), set(BASIC_SELF));
    // `/a#top` zählt als `/a` (Fragment entfernt); `/a` mit Canonical `/a/` nicht (dort Canonicalised).
    assert!(c.set("Self Referencing").contains("/fragment.html"));
    assert!(!c.set("Self Referencing").contains("/slash.html"));
    assert!(c.set("Canonicalised").contains("/slash.html"));
    // absolute und relative Angabe derselben Adresse
    assert!(c.set("Self Referencing").contains("/self-abs.html"));
}

// spec: TECH-014
#[tokio::test]
async fn canonicalised_pages_point_elsewhere_and_become_non_indexable() {
    let c = crawl("tech/canonical-basic", CrawlConfig::default(), true).await;
    assert_eq!(c.set("Canonicalised"), set(BASIC_CANONICALISED));

    // andere Domain, anderes Schema, anderer Pfad → Canonicalised und Non-Indexable
    for a in ["/to-external.html", "/to-https.html", "/to-other.html"] {
        assert_eq!(
            c.text_cell("Canonicalised", a, "Indexability"),
            "Non-Indexable",
            "{a}"
        );
        assert_eq!(
            c.text_cell("Canonicalised", a, "Indexability Status"),
            "Canonicalised",
            "{a}"
        );
    }
    // ungültige Canonical: nicht Canonicalised, nicht Self Referencing, die Seite bleibt indexierbar
    assert!(!c.set("Canonicalised").contains("/invalid-url.html"));
    assert!(!c.set("Self Referencing").contains("/invalid-url.html"));
    assert!(!c.set("Missing").contains("/invalid-url.html"));
    assert_eq!(
        c.text_cell("All", "/invalid-url.html", "Indexability"),
        "Indexable"
    );
    assert_eq!(
        c.cell("All", "/invalid-url.html", "Occurrences"),
        Value::Int(1)
    );
    assert_eq!(
        c.text_cell("All", "/invalid-url.html", "Canonical Link Element 1"),
        "http://[bad"
    );
    assert_eq!(
        c.cell("All", "/invalid-url.html", "Canonical URL"),
        Value::Null
    );
}

// spec: TECH-015
#[tokio::test]
async fn missing_lists_only_html_pages_without_any_canonical() {
    let c = crawl("tech/canonical-basic", CrawlConfig::default(), true).await;
    assert_eq!(
        c.set("Missing"),
        set(&["/body-only.html", "/none.html", "/none.pdf"])
    );
    // M2-S05: Auch PDFs ohne HTTP-Canonical stehen in Missing.
    assert!(c.set("All").contains("/none.pdf"));
    assert_eq!(
        c.cell("Missing", "/none.html", "Occurrences"),
        Value::Int(0)
    );
}

// spec: TECH-016
#[tokio::test]
async fn multiple_counts_elements_and_headers_together() {
    let c = crawl("tech/canonical-basic", CrawlConfig::default(), true).await;
    assert_eq!(c.set("Multiple"), set(BASIC_MULTIPLE));
    // zwei identische Elemente, Element + identischer Header: Multiple, aber nicht Conflicting
    for a in ["/multi-same.html", "/multi-mixed-same.html"] {
        assert!(c.set("Multiple").contains(a), "{a}");
        assert!(!c.set("Multiple Conflicting").contains(a), "{a}");
        assert_eq!(c.cell("Multiple", a, "Occurrences"), Value::Int(2));
    }
}

// spec: TECH-017
#[tokio::test]
async fn multiple_conflicting_needs_two_different_valid_normalised_urls() {
    let c = crawl("tech/canonical-basic", CrawlConfig::default(), true).await;
    let conflicting = c.set("Multiple Conflicting");
    assert_eq!(conflicting, set(BASIC_CONFLICTING));
    // Jede Seite in „Multiple Conflicting“ steht auch in „Multiple“ (umgekehrt nicht).
    assert!(conflicting.is_subset(&c.set("Multiple")));
    assert!(c.set("Multiple").len() > conflicting.len());
    // Negativfälle: relativ + absolut gleich, Fragment, ungültiger Zweitwert, Prozent-Schreibweise
    for a in [
        "/multi-relabs.html",
        "/multi-fragment.html",
        "/multi-invalid.html",
        "/multi-pct.html",
    ] {
        assert!(!conflicting.contains(a), "{a} ist nicht widersprüchlich");
        assert!(c.set("Multiple").contains(a), "{a} ist Multiple");
    }
    // Pfad ist case-sensitiv (`/a` ≠ `/A`)
    assert!(conflicting.contains("/multi-case.html"));
    // Ungültiger Wert ignoriert, aber mitgezählt: Occurrences = 2
    assert_eq!(
        c.cell("Multiple", "/multi-invalid.html", "Occurrences"),
        Value::Int(2)
    );
}

// spec: TECH-017
#[tokio::test]
async fn authoritative_canonical_prefers_header_then_head_and_ignores_body() {
    let c = crawl("tech/canonical-basic", CrawlConfig::default(), true).await;
    let auth = |filter: &str, a: &str| c.text_cell(filter, a, "Canonical URL");
    let status = |filter: &str, a: &str| c.text_cell(filter, a, "Indexability Status");

    // Header `/a` + Head `/b` → maßgeblich `/a`, Canonicalised
    assert_eq!(auth("Multiple", "/header-over-head.html"), "/target-a.html");
    assert_eq!(
        status("Multiple", "/header-over-head.html"),
        "Canonicalised"
    );
    assert_eq!(
        auth("Multiple", "/multi-conflict-mixed.html"),
        "/target-a.html"
    );
    // Head `/p` + Body `/b` → maßgeblich `/p` (Self Referencing), zugleich Multiple Conflicting
    assert_eq!(
        auth("Self Referencing", "/head-self-body-other.html"),
        "/head-self-body-other.html"
    );
    assert!(c
        .set("Self Referencing")
        .contains("/head-self-body-other.html"));
    assert!(!c
        .set("Multiple Conflicting")
        .contains("/head-self-body-other.html"));
    assert_eq!(
        c.text_cell("All", "/head-self-body-other.html", "Indexability"),
        "Indexable"
    );
    // Body allein → maßgeblich, Canonicalised
    assert_eq!(auth("Missing", "/body-only.html"), "");
    assert_eq!(status("Missing", "/body-only.html"), "");

    // Die Spalten zeigen alle Werte in Quelltext- bzw. Header-Reihenfolge, unabhängig von der Auswahlregel.
    assert_eq!(
        c.text_cell("All", "/header-over-head.html", "Canonical Link Element 1"),
        "/target-b.html"
    );
    assert_eq!(
        c.text_cell("All", "/header-over-head.html", "HTTP Canonical 1"),
        "/target-a.html"
    );
    assert_eq!(
        c.text_cell("All", "/multi-conflict.html", "Canonical Link Element 1"),
        "/target-a.html"
    );
    assert_eq!(
        c.text_cell("All", "/multi-conflict.html", "Canonical Link Element 2"),
        "/target-b.html"
    );
}

// spec: TECH-001
#[tokio::test]
async fn noindex_wins_over_a_foreign_canonical_in_the_tab() {
    let c = crawl("tech/canonical-basic", CrawlConfig::default(), true).await;
    // Reihenfolge der Gründe: noindex vor Canonicalised – die Seite steht aber im Filter Canonicalised.
    assert_eq!(
        c.text_cell(
            "Canonicalised",
            "/noindex-with-canonical.html",
            "Indexability Status"
        ),
        "noindex"
    );
    assert_eq!(
        c.text_cell("All", "/noindex-with-canonical.html", "Meta Robots 1"),
        "noindex"
    );
    // Nicht-HTML (PDF) wird per HTTP-Canonical non-indexable.
    assert_eq!(
        c.text_cell("All", "/doc.pdf", "Indexability Status"),
        "Canonicalised"
    );
    assert_eq!(
        c.text_cell("All", "/doc.pdf", "HTTP Canonical 1"),
        "/target.html"
    );
}

// spec: TECH-018
#[tokio::test]
async fn non_indexable_canonical_is_a_post_crawl_filter() {
    let c = crawl("tech/canonical-basic", CrawlConfig::default(), false).await;

    // Vor der Analyse: Zustand statt Zahl (TECH-005), keine Zeilen.
    let counts = filter_counts(&c.fx.file, &c.cfg).unwrap();
    let nic = counts
        .iter()
        .find(|f| f.tab == TAB && f.filter == "Non-Indexable Canonical")
        .unwrap();
    assert_eq!(nic.state, FilterState::NeedsCrawlAnalysis);
    assert_eq!(nic.count, None);
    // Live-Filter zählen schon.
    let multiple = counts
        .iter()
        .find(|f| f.tab == TAB && f.filter == "Multiple")
        .unwrap();
    assert_eq!(multiple.count, Some(BASIC_MULTIPLE.len() as u64));

    // Nach dem Schritt „Kanonisierungen“: genau die Seiten, deren Canonical-Ziel non-indexable ist.
    let outcome = run(&c.fx.file, &c.cfg, &CancelToken::new(), &|_, _| {}).unwrap();
    assert!(outcome
        .steps
        .contains(&(AnalysisStep::Canonicals, StepState::Done)));
    assert_eq!(c.set("Non-Indexable Canonical"), set(BASIC_NON_INDEXABLE));

    let counts = filter_counts(&c.fx.file, &c.cfg).unwrap();
    let nic = counts
        .iter()
        .find(|f| f.tab == TAB && f.filter == "Non-Indexable Canonical")
        .unwrap();
    assert_eq!(nic.state, FilterState::Ready);
    assert_eq!(nic.count, Some(BASIC_NON_INDEXABLE.len() as u64));

    // Zielstatus: 404, 3xx, noindex, robots-gesperrt, selbst kanonisiert, externer 404 → gelistet.
    // Indexierbares Ziel (auch extern 200) und Self-Referencing-Seiten → nicht gelistet.
    let listed = c.set("Non-Indexable Canonical");
    for a in [
        "/to-other.html",
        "/to-external.html",
        "/self.html",
        "/canonicalised-target.html",
    ] {
        assert!(!listed.contains(a), "{a} ist nicht gelistet");
    }
}

// spec: TECH-018
#[tokio::test]
async fn the_step_is_repeatable_and_stamps_the_change_feed() {
    let c = crawl("tech/canonical-basic", CrawlConfig::default(), false).await;
    let conn = c.fx.file.conn();
    let before = queries::change_seq(conn).unwrap();
    analyse_now(&c);
    let first = queries::change_seq(conn).unwrap();
    assert!(first > before, "neue Markierungen erhöhen die Sequenz");

    // Die markierten Zeilen erscheinen im Änderungsfeed.
    let changed = queries::changed_since(conn, before, 500).unwrap();
    let flagged: i64 = conn.query_row("SELECT COUNT(DISTINCT url_id) FROM url_flags WHERE flag IN ('non_indexable_canonical', 'unlinked_canonical')", [], |r| r.get(0)).unwrap();
    assert!(changed.len() >= flagged as usize);
    // TECH-139/145: Links und Link Score stempeln zusätzlich eigene Ergebniszeilen.
    assert!(BASIC_NON_INDEXABLE
        .iter()
        .all(|path| changed.iter().any(|id| {
            let row = queries::url_by_id(conn, *id).unwrap().unwrap();
            c.norm(&row.address) == *path
        })));

    // Zweiter Lauf: gleiche Menge, keine Änderung.
    analyse_now(&c);
    assert_eq!(queries::change_seq(conn).unwrap(), first);
    assert_eq!(c.set("Non-Indexable Canonical"), set(BASIC_NON_INDEXABLE));
}

// spec: TECH-018
#[tokio::test]
async fn a_target_that_was_not_crawled_is_not_listed() {
    // Ausgeschlossenes Ziel (Exclude) → Status unbekannt → nicht gelistet.
    let mut cfg = CrawlConfig::default();
    cfg.scope.exclude = vec![".*/excluded/.*".to_string()];
    let c = crawl("tech/canonical-basic", cfg, true).await;
    let listed = c.set("Non-Indexable Canonical");
    assert!(!listed.contains("/to-excluded.html"));
    // Die übrigen bleiben.
    let expected: BTreeSet<String> = set(BASIC_NON_INDEXABLE)
        .into_iter()
        .filter(|a| a != "/to-excluded.html")
        .collect();
    assert_eq!(listed, expected);
    // Die Seite selbst steht weiter in Canonicalised.
    assert!(c.set("Canonicalised").contains("/to-excluded.html"));
}

// spec: TECH-018
#[tokio::test]
async fn without_canonical_crawling_no_target_status_is_known() {
    // Canonicals crawlen aus: Ziele, die nur per Canonical bekannt sind, werden nicht abgerufen.
    let mut cfg = CrawlConfig::default();
    cfg.spider.canonicals.crawl = false;
    let c = crawl("tech/canonical-basic", cfg, true).await;
    assert!(c.set("Non-Indexable Canonical").is_empty());
    // Die Canonical-Aussagen der Seiten bleiben davon unberührt; nur per Canonical bekannte Ziele fehlen.
    assert!(c.set("Canonicalised").contains("/to-404.html"));
    assert!(c.set("Canonicalised").contains("/to-other.html"));
    assert!(!c.set("All").contains("/canonicalised-target.html"));
    let counts = filter_counts(&c.fx.file, &c.cfg).unwrap();
    let nic = counts
        .iter()
        .find(|f| f.tab == TAB && f.filter == "Non-Indexable Canonical")
        .unwrap();
    assert_eq!(
        (nic.state.clone(), nic.count),
        (FilterState::Ready, Some(0))
    );
}

// spec: TECH-011
#[tokio::test]
async fn the_tab_is_empty_when_canonicals_are_not_stored() {
    // CRAWL-046: Canonicals speichern aus → Tab leer, kein „Canonicalised“, keine Canonical-Spaltenwerte.
    let mut cfg = CrawlConfig::default();
    cfg.spider.canonicals.store = false;
    let c = crawl("tech/canonical-basic", cfg, true).await;
    for filter in FILTERS {
        if *filter == "Non-Indexable Canonical" {
            continue;
        }
        assert!(c.addresses(filter).is_empty(), "{filter} ist leer");
    }
    assert!(c.addresses("Non-Indexable Canonical").is_empty());
    let n: i64 =
        c.fx.file
            .conn()
            .query_row("SELECT COUNT(*) FROM canonicals", [], |r| r.get(0))
            .unwrap();
    assert_eq!(n, 0, "keine Canonical-Zeilen");
    let canonicalised: i64 =
        c.fx.file
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM urls WHERE indexability_status = 'Canonicalised'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(canonicalised, 0, "kein Status Canonicalised");
}

// Golden-Dateien (TECH-007): Adresse, Occurrences, Status, maßgebliche Canonical je Filter.
// spec: TECH-011
#[tokio::test]
async fn goldens_canonical_basic() {
    let c = crawl("tech/canonical-basic", CrawlConfig::default(), true).await;
    golden(&c, "canonical-basic", FILTERS);
}

// ---------------------------------------------------------------------------------------------------------------
// url-compare (TECH-002)

// spec: TECH-002
#[tokio::test]
async fn canonical_variants_resolve_to_absolute_urls_and_compare_after_normalisation() {
    let c = crawl("tech/url-compare", CrawlConfig::default(), true).await;
    let value = |a: &str| c.text_cell("All", a, "Canonical Link Element 1");

    // In den Spalten steht die aufgelöste absolute URL (Fragment bleibt stehen), bei ungültiger URL der Rohwert.
    assert_eq!(value("/u/rel.html"), "/u/rel.html");
    assert_eq!(value("/u/dotdot.html"), "/u/dotdot.html");
    assert_eq!(value("/u/fragment.html"), "/u/fragment.html");
    assert_eq!(
        value("/u/empty.html"),
        "/u/empty.html",
        "leerer href = Dokument-URL"
    );
    assert_eq!(
        value("/u/base.html"),
        "/other/base.html",
        "relativ zu <base href>"
    );
    assert_eq!(value("/u/broken.html"), "http://[bad");
    assert_eq!(
        value("/u/abs-scheme-upper.html"),
        "/u/abs-scheme-upper.html"
    );

    // Gleich nach TECH-002: Self Referencing; verschieden: Canonicalised.
    assert_eq!(
        c.set("Self Referencing"),
        set(&[
            "/",
            "/other/base.html",
            "/u/abs-scheme-upper.html",
            "/u/dotdot.html",
            "/u/empty.html",
            "/u/explicit-slash.html",
            "/u/fragment.html",
            "/u/query.html?x=1",
            "/u/rel.html",
            "/u/root.html",
            "/u/trailing-slash.html/",
        ])
    );
    assert_eq!(
        c.set("Canonicalised"),
        set(&[
            "/u/base.html",
            "/u/pct-case.html",
            "/u/query.html",
            "/u/trailing-slash.html",
        ])
    );
    // `/x` und `/x/` sind verschieden; ungültige URL ist nie ein Treffer und die Seite bleibt indexierbar.
    assert_eq!(
        c.text_cell("All", "/u/broken.html", "Indexability"),
        "Indexable"
    );
    assert!(!c.set("Self Referencing").contains("/u/broken.html"));
    assert!(!c.set("Canonicalised").contains("/u/broken.html"));
    golden(&c, "url-compare", FILTERS);
}

// ---------------------------------------------------------------------------------------------------------------
// canonical-extract (TECH-010)

// spec: TECH-010
#[tokio::test]
async fn extraction_follows_the_rules_for_rel_tokens_headers_and_hiding_places() {
    let c = crawl("tech/canonical-extract", CrawlConfig::default(), true).await;
    let occurrences = |a: &str| match c.cell("All", a, "Occurrences") {
        Value::Int(i) => i,
        other => panic!("{a}: {other:?}"),
    };

    // rel als Token-Liste, case-insensitiv; `canonicalx` zählt nicht
    for a in ["/html-basic.html", "/rel-upper.html", "/rel-multi.html"] {
        assert_eq!(occurrences(a), 1, "{a}");
    }
    assert_eq!(occurrences("/rel-x.html"), 0);
    // ohne href, in <template>, in Kommentaren: ignoriert; in <noscript> im Head: erfasst (Festlegung)
    for a in ["/no-href.html", "/in-template.html", "/in-comment.html"] {
        assert_eq!(occurrences(a), 0, "{a}");
    }
    assert_eq!(occurrences("/in-noscript.html"), 1);
    // Link-Header: zwei Header, ein Header mit zwei Einträgen, Parameter ohne Anführungszeichen, anderes rel
    assert_eq!(occurrences("/two-headers.html"), 2);
    assert_eq!(
        c.text_cell("All", "/two-headers.html", "HTTP Canonical 1"),
        "/a-one.html"
    );
    assert_eq!(
        c.text_cell("All", "/two-headers.html", "HTTP Canonical 2"),
        "/a-two.html"
    );
    assert_eq!(occurrences("/one-header-two-entries.html"), 2);
    assert_eq!(
        c.text_cell("All", "/one-header-two-entries.html", "HTTP Canonical 1"),
        "/b-one.html"
    );
    assert_eq!(
        c.text_cell("All", "/one-header-two-entries.html", "HTTP Canonical 2"),
        "/b-two.html"
    );
    assert_eq!(occurrences("/header-unquoted.html"), 1);
    assert_eq!(occurrences("/header-other-rel.html"), 0);
    // HTTP-Canonical gilt auch für Nicht-HTML (PDF); auch ohne Header steht das PDF im Tab (M2-S05).
    assert_eq!(occurrences("/header.pdf"), 1);
    assert_eq!(
        c.text_cell("All", "/header.pdf", "HTTP Canonical 1"),
        "/target.html"
    );
    assert!(c.set("All").contains("/plain.pdf"));
    // Seiten ohne gültige Canonical-Angabe stehen in „Missing“ (nur HTML).
    assert_eq!(
        c.set("Missing"),
        set(&[
            "/header-other-rel.html",
            "/in-comment.html",
            "/in-template.html",
            "/no-href.html",
            "/plain.pdf",
            "/rel-x.html",
        ])
    );
    golden(&c, "canonical-extract", FILTERS);
}

// ---------------------------------------------------------------------------------------------------------------
// multi-columns (TECH-004)

// spec: TECH-004
#[tokio::test]
async fn numbered_canonical_columns_reach_the_maximum_of_the_view() {
    let c = crawl("tech/multi-columns", CrawlConfig::default(), true).await;
    let page = c.page("All", &["Address", "Canonical Link Element"]);
    // A mit 3 Canonicals, B mit 1: Spalten 1 bis 3, fehlende Zellen leer (kein null).
    assert_eq!(
        page.columns,
        [
            "Address",
            "Canonical Link Element 1",
            "Canonical Link Element 2",
            "Canonical Link Element 3"
        ]
    );
    let row = |a: &str| -> Vec<String> {
        page.rows
            .iter()
            .find(|r| c.norm(&text(&r[0])) == a)
            .unwrap_or_else(|| panic!("{a}"))
            .iter()
            .skip(1)
            .map(|v| c.norm(&text(v)))
            .collect()
    };
    assert_eq!(row("/a.html"), ["/t1.html", "/t2.html", "/t3.html"]);
    assert_eq!(row("/b.html"), ["/t1.html", "", ""]);
    // In einer Ansicht, in der nur die Seite B steht, gibt es nur eine Spalte.
    let mut q = ViewQuery::new(TAB, "All");
    q.columns = ColumnSelection::Named(vec!["Address".into(), "Canonical Link Element".into()]);
    q.search = Some(os_analyze::views::SearchSpec::Simple("b.html".into()));
    let only_b = query_view(&c.fx.file, &c.cfg, &q).unwrap();
    assert_eq!(only_b.columns, ["Address", "Canonical Link Element 1"]);

    // hreflang-Obergrenze: 510 Annotationen → 500 gespeichert, genau 500 → alle 500 (Standard truncate)
    let stored = |a: &str| -> i64 {
        c.fx.file
            .conn()
            .query_row(
                "SELECT hreflang_count FROM urls WHERE address = ?1",
                [format!("{}{a}", c.fx.site.base_url())],
                |r| r.get(0),
            )
            .unwrap()
    };
    assert_eq!(stored("/c510.html"), 500);
    assert_eq!(stored("/d500.html"), 500);
    golden(&c, "multi-columns", FILTERS);
}

// ---------------------------------------------------------------------------------------------------------------
// M1-N16: 2xx statt 200 (K-56) und `issues.ignore_non_indexable` für Missing (CRAWL-089, B-05)

/// Teilbaum `/n16/` von `tech/canonical-basic`: zwei Seiten mit Status 201, eine noindex-Seite ohne Canonical,
/// eine indexierbare Seite ohne Canonical, die Startseite mit Self-Canonical.
const N16_ALL: &[&str] = &[
    "/n16/created-canon.html",
    "/n16/created.html",
    "/n16/index.html",
    "/n16/noindex-nocanon.html",
    "/n16/plain.html",
];

// spec: TECH-011
#[tokio::test]
async fn population_and_canonicalised_step_cover_status_2xx() {
    let c = crawl_from(
        "tech/canonical-basic",
        "/n16/index.html",
        CrawlConfig::default(),
    )
    .await;
    // Die 201-Seiten stehen in Canonicals › All (Status 2xx statt nur 200).
    assert_eq!(c.set("All"), set(N16_ALL));
    assert!(c
        .set("Contains Canonical")
        .contains("/n16/created-canon.html"));
    // 201 mit Canonical auf eine andere URL: Canonicalised; das Ziel ist noindex, also Non-Indexable Canonical
    // (Schritt „Kanonisierungen“ bewertet Quellen mit Status 2xx).
    assert!(c.set("Canonicalised").contains("/n16/created-canon.html"));
    assert_eq!(
        c.set("Non-Indexable Canonical"),
        set(&["/n16/created-canon.html"])
    );
}

// spec: TECH-015
#[tokio::test]
async fn missing_lists_2xx_html_without_canonical_and_skips_non_indexable_by_default() {
    let c = crawl_from(
        "tech/canonical-basic",
        "/n16/index.html",
        CrawlConfig::default(),
    )
    .await;
    // Standard: `issues.ignore_non_indexable` an → die noindex-Seite fehlt, die 201-Seite steht drin.
    assert!(c.cfg.issues.ignore_non_indexable);
    assert_eq!(
        c.set("Missing"),
        set(&["/n16/created.html", "/n16/plain.html"])
    );
    // „All“ ist von der Option nie betroffen.
    assert!(c.set("All").contains("/n16/noindex-nocanon.html"));
}

// spec: TECH-015, CRAWL-089
#[tokio::test]
async fn missing_lists_non_indexable_pages_when_the_option_is_off() {
    let mut cfg = CrawlConfig::default();
    cfg.issues.ignore_non_indexable = false;
    let c = crawl_from("tech/canonical-basic", "/n16/index.html", cfg).await;
    assert_eq!(
        c.set("Missing"),
        set(&[
            "/n16/created.html",
            "/n16/noindex-nocanon.html",
            "/n16/plain.html"
        ])
    );
    // Die Option ändert nur Missing; die übrigen Canonical-Filter bleiben gleich.
    assert_eq!(c.set("Self Referencing"), set(&["/n16/index.html"]));
}
