//! Tab „Links“ (M1-P25): Population und Spalten, Filter für hohe Crawl-Tiefe, Sackgassen, interne
//! Nofollow-Outlinks und fehlenden Ankertext, Link-Präferenzen. Fixture `tests/fixtures/tech/links`
//! (Hauptbaum ab `/`, Teilbaum `/menu10/`), Goldens unter `tests/golden/tech/links`.

use std::path::{Path, PathBuf};

use os_analyze::views::{filter_counts, query_view, ColumnSelection, ViewPage, ViewQuery};
use os_engine::testing::{crawl_fixture_from, FixtureCrawl};
use os_model::{CrawlConfig, FilterState, StoreCrawl, Value};
use os_store::{CrawlFile, OpenMode};
use os_testsite::golden;

const BASE: &str = "http://site.test";

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/tech/links")
}

fn text(v: &Value) -> String {
    match v {
        Value::Text(s) => s.clone(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
    }
}

fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}

fn page(file: &CrawlFile, cfg: &CrawlConfig, filter: &str, all: bool) -> ViewPage {
    let mut q = ViewQuery::new("Links", filter);
    q.limit = 500;
    if all {
        q.columns = ColumnSelection::All;
    }
    query_view(file, cfg, &q).unwrap()
}

fn list(file: &CrawlFile, cfg: &CrawlConfig, filter: &str) -> Vec<String> {
    page(file, cfg, filter, false)
        .rows
        .iter()
        .map(|r| text(&r[0]).replace(BASE, ""))
        .collect()
}

/// Spec-abgeleitete Erwartung und Golden-Datei einer Adressliste.
fn check(fx: &FixtureCrawl, cfg: &CrawlConfig, filter: &str, expected: &[&str]) {
    let got = list(&fx.file, cfg, filter);
    assert_eq!(got, expected, "Links:{filter}");
    let mut body = String::from("Address\n");
    for a in &got {
        body.push_str(a);
        body.push('\n');
    }
    golden::assert_golden(
        golden_dir().join(format!("links__{}.csv", slug(filter))),
        &body,
    );
}

fn cell(page: &ViewPage, address: &str, column: &str) -> String {
    let col = page
        .columns
        .iter()
        .position(|c| c == column)
        .unwrap_or_else(|| panic!("Spalte {column:?} fehlt in {:?}", page.columns));
    let row = page
        .rows
        .iter()
        .find(|r| text(&r[0]) == format!("{BASE}{address}"))
        .unwrap_or_else(|| panic!("Zeile {address:?} fehlt"));
    text(&row[col])
}

fn cfg_links() -> CrawlConfig {
    let mut cfg = CrawlConfig::default();
    // Eine Verbindung: Tiefen und Reihenfolge sind deterministisch. Externe Ziele nur speichern.
    cfg.speed.threads = 1;
    cfg.spider.external = StoreCrawl::STORE_ONLY;
    cfg
}

async fn crawl_main(cfg: &CrawlConfig) -> FixtureCrawl {
    crawl_fixture_from("tech/links", &format!("{BASE}/"), cfg.clone()).await
}

const ALL_PAGES: [&str; 18] = [
    "/",
    "/anchor-alt-empty.html",
    "/anchor-alt.html",
    "/anchor-area-alt.html",
    "/anchor-area.html",
    "/anchor-aria.html",
    "/anchor-empty.html",
    "/anchor-ok.html",
    "/anchors.html",
    "/attrs.html",
    "/deadend.html",
    "/l1/",
    "/l2/",
    "/l3/",
    "/l4/",
    "/l5/",
    "/meta-nofollow.html",
    "/self-only.html",
];

// spec: TECH-130
#[tokio::test]
async fn population_is_internal_html_200_and_counts_exclude_self_links() {
    let cfg = cfg_links();
    let fx = crawl_main(&cfg).await;
    check(&fx, &cfg, "All", &ALL_PAGES);

    // Standardspalten = Referenz-Spaltenfolge; `Inlinks` ist eine ausgeblendete Zusatzspalte.
    let default = page(&fx.file, &cfg, "All", false);
    assert_eq!(
        default.columns,
        [
            "Address",
            "Indexability",
            "Indexability Status",
            "Crawl Depth",
            "Link Score",
            "Unique Inlinks",
            "Unique JS Inlinks",
            "% of Total",
            "Outlinks",
            "Unique Outlinks",
            "Unique JS Outlinks",
            "External Outlinks",
            "Unique External Outlinks",
            "Unique External JS Outlinks"
        ]
    );
    let all = page(&fx.file, &cfg, "All", true);
    assert_eq!(
        all.columns.last().map(String::as_str),
        Some("Inlinks"),
        "Erweiterungsspalte steht hinter den Referenzspalten"
    );

    // Startseite: Tiefe 0; 5 Ketten-Seiten + /attrs.html verlinken sie (der Selbstlink zählt nicht).
    assert_eq!(cell(&all, "/", "Crawl Depth"), "0");
    assert_eq!(cell(&all, "/", "Inlinks"), "6");
    assert_eq!(cell(&all, "/", "Unique Inlinks"), "6");
    assert_eq!(cell(&all, "/", "% of Total"), "33.33");
    // Outlinks: 7 interne Links (/l1/ doppelt, ohne den Selbstlink) auf 6 Ziele; 3 externe auf 2 Ziele.
    assert_eq!(cell(&all, "/", "Outlinks"), "7");
    assert_eq!(cell(&all, "/", "Unique Outlinks"), "6");
    assert_eq!(cell(&all, "/", "External Outlinks"), "3");
    assert_eq!(cell(&all, "/", "Unique External Outlinks"), "2");

    // Mehrfachlinks: /l1/ wird von / zweimal, von /attrs.html und /meta-nofollow.html je einmal verlinkt.
    assert_eq!(cell(&all, "/l1/", "Inlinks"), "4");
    assert_eq!(cell(&all, "/l1/", "Unique Inlinks"), "3");
    assert_eq!(cell(&all, "/l1/", "% of Total"), "16.67");
    assert_eq!(cell(&all, "/l1/", "Outlinks"), "2");
    assert_eq!(cell(&all, "/l1/", "Unique Outlinks"), "2");

    // Selbstlink: /self-only.html hat zusätzlich einen Link auf sich selbst – er zählt weder als Inlink noch
    // als Outlink.
    assert_eq!(cell(&all, "/self-only.html", "Inlinks"), "2");
    assert_eq!(cell(&all, "/self-only.html", "Unique Inlinks"), "2");
    assert_eq!(cell(&all, "/self-only.html", "Outlinks"), "0");
    assert_eq!(cell(&all, "/self-only.html", "Unique Outlinks"), "0");
    assert_eq!(cell(&all, "/deadend.html", "Unique Inlinks"), "9");

    // Tiefe der Kette
    for (page_path, depth) in [
        ("/l1/", "1"),
        ("/l3/", "3"),
        ("/l5/", "5"),
        ("/anchor-ok.html", "2"),
    ] {
        assert_eq!(cell(&all, page_path, "Crawl Depth"), depth, "{page_path}");
    }

    // Link Score (P1) und JavaScript-Spalten sind in M1 leer.
    for col in [
        "Link Score",
        "Unique JS Inlinks",
        "Unique JS Outlinks",
        "Unique External JS Outlinks",
    ] {
        assert_eq!(cell(&all, "/l1/", col), "", "{col}");
    }

    // Nur Hyperlinks zählen: die Bilder von /anchor-*.html erhöhen keine Zähler (/img/x.png ist keine Seite).
    assert_eq!(cell(&all, "/anchor-alt.html", "Outlinks"), "1");

    golden::assert_golden(golden_dir().join("counts.txt"), &counts(&fx.file, &cfg));
}

/// Filterzahlen des Tabs „Links“ (eine Zeile je Filter).
fn counts(file: &CrawlFile, cfg: &CrawlConfig) -> String {
    let mut out = String::new();
    for c in filter_counts(file, cfg)
        .unwrap()
        .into_iter()
        .filter(|c| c.tab == "Links")
    {
        assert_eq!(c.state, FilterState::Ready, "{}", c.filter);
        out.push_str(&format!("{}: {}\n", c.filter, c.count.unwrap()));
    }
    out
}

// spec: TECH-130
#[tokio::test]
async fn percent_of_total_of_a_menu_page_is_ninety() {
    // 10 Seiten, alle verlinken /menu10/ (auch sich selbst): 9 von 10 ohne den Selbstlink.
    let cfg = cfg_links();
    let fx = crawl_fixture_from("tech/links", &format!("{BASE}/menu10/"), cfg.clone()).await;
    let all = page(&fx.file, &cfg, "All", true);
    assert_eq!(all.rows.len(), 10);
    assert_eq!(cell(&all, "/menu10/", "% of Total"), "90");
    assert_eq!(cell(&all, "/menu10/", "Unique Inlinks"), "9");
    assert_eq!(cell(&all, "/menu10/p1/", "% of Total"), "10");
}

// spec: TECH-132
#[tokio::test]
async fn high_crawl_depth_uses_the_threshold_and_ignores_non_indexable_urls() {
    let cfg = cfg_links();
    let fx = crawl_main(&cfg).await;
    // Kette bis Tiefe 5, Schwelle 4: Tiefe 4 und 5.
    check(&fx, &cfg, "Pages With High Crawl Depth", &["/l4/", "/l5/"]);

    // Schwelle 6: Tiefe 5 nicht gelistet; Schwelle 5: nur Tiefe 5 (Änderung ohne Recrawl, TECH-144).
    let mut c6 = cfg.clone();
    c6.links.high_crawl_depth = 6;
    assert!(list(&fx.file, &c6, "Pages With High Crawl Depth").is_empty());
    let mut c5 = cfg.clone();
    c5.links.high_crawl_depth = 5;
    assert_eq!(list(&fx.file, &c5, "Pages With High Crawl Depth"), ["/l5/"]);

    // „Ignore Non-Indexable URLs for Issues“ (Standard an): nicht indexierbare Seiten fallen heraus.
    fx.file
        .conn()
        .execute(
            "UPDATE urls SET indexability = 'Non-Indexable', indexability_status = 'noindex' \
             WHERE address = 'http://site.test/l5/'",
            [],
        )
        .unwrap();
    assert_eq!(
        list(&fx.file, &cfg, "Pages With High Crawl Depth"),
        ["/l4/"]
    );
    let mut keep = cfg.clone();
    keep.issues.ignore_non_indexable = false;
    assert_eq!(
        list(&fx.file, &keep, "Pages With High Crawl Depth"),
        ["/l4/", "/l5/"]
    );
}

// spec: TECH-133
#[tokio::test]
async fn pages_without_internal_outlinks_count_only_non_self_links() {
    let cfg = cfg_links();
    let fx = crawl_main(&cfg).await;
    // /deadend.html ohne Links; /self-only.html nur mit Selbstlink. /meta-nofollow.html (Seiten-nofollow)
    // und /attrs.html (nofollow-Links) haben Outlinks und fehlen.
    check(
        &fx,
        &cfg,
        "Pages Without Internal Outlinks",
        &["/deadend.html", "/self-only.html"],
    );
}

// spec: TECH-134
#[tokio::test]
async fn internal_nofollow_outlinks_are_rel_based_not_page_based() {
    let cfg = cfg_links();
    let fx = crawl_main(&cfg).await;
    // nofollow, ugc, sponsored auf /attrs.html. `noopener noreferrer` ist folgbar. Die Seite
    // /meta-nofollow.html (Meta robots nofollow) löst den Filter nicht aus.
    check(&fx, &cfg, "Internal Nofollow Outlinks", &["/attrs.html"]);
    // Der Link wird trotzdem gespeichert: Seiten-nofollow setzt `follow = false` an allen Links der Seite.
    let follow: i64 = fx
        .file
        .conn()
        .query_row(
            "SELECT follow FROM links l JOIN urls u ON u.id = l.source_id \
             WHERE u.address = 'http://site.test/meta-nofollow.html' AND l.link_type = 'Hyperlink'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(follow, 0);
}

// spec: TECH-135
#[tokio::test]
async fn internal_outlinks_with_no_anchor_text_cover_images_areas_and_aria_labels() {
    let cfg = cfg_links();
    let fx = crawl_main(&cfg).await;
    // Leerer Link, Bild-Link mit leerem alt, <area> ohne alt und ein Link nur mit aria-label/title zählen.
    // Bild-Link mit alt="Logo", <area alt> und Text-Link zählen nicht.
    check(
        &fx,
        &cfg,
        "Internal Outlinks With No Anchor Text",
        &[
            "/anchor-alt-empty.html",
            "/anchor-area.html",
            "/anchor-aria.html",
            "/anchor-empty.html",
        ],
    );
}

// spec: TECH-144
#[tokio::test]
async fn link_preferences_live_in_the_crawl_file_and_apply_without_recrawl() {
    let cfg = cfg_links();
    let fx = crawl_main(&cfg).await;

    // Standardwerte (Festlegung der Spec)
    let defaults = CrawlConfig::default().links;
    assert_eq!(defaults.high_crawl_depth, 4);
    assert_eq!(defaults.high_internal_outlinks, 1000);
    assert_eq!(defaults.high_external_outlinks, 100);
    assert_eq!(defaults.non_descriptive_anchors.len(), 26);
    for entry in ["click here", "read more", "hier klicken", "details ansehen"] {
        assert!(
            defaults.non_descriptive_anchors.iter().any(|a| a == entry),
            "{entry}"
        );
    }

    // Geänderte Schwelle (6) wirkt nach set_config sofort und steht danach in der Datei.
    let mut changed = cfg.clone();
    changed.links.high_crawl_depth = 6;
    changed.links.non_descriptive_anchors = vec!["foo".to_string()];
    fx.file.set_config(&changed).unwrap();
    let reopened = CrawlFile::open(fx.file.path(), OpenMode::ReadOnly).unwrap();
    let stored = reopened.config().unwrap();
    assert_eq!(stored.links.high_crawl_depth, 6);
    assert_eq!(stored.links.non_descriptive_anchors, ["foo"]);
    assert!(list(&reopened, &stored, "Pages With High Crawl Depth").is_empty());
    fx.file.set_config(&cfg).unwrap();
    assert_eq!(
        list(
            &fx.file,
            &fx.file.config().unwrap(),
            "Pages With High Crawl Depth"
        ),
        ["/l4/", "/l5/"]
    );
}
