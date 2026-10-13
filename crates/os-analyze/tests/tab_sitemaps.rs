//! Tab „Sitemaps“ und Nachlauf-Schritt „Sitemaps“ (M1-P25): In/Not In Sitemap, Orphans (Folge-Orphans,
//! `Found Via`, Ressourcen nie), Non-Indexable In Sitemap, Zustand der Nachlauf-Filter (TECH-005).
//! Fixtures `tests/fixtures/tech/{sitemaps,crawl-analysis}`, Goldens unter `tests/golden/tech/…`.
//!
//! P24 (Indexierbarkeit: noindex, Canonical) ist parallel in Arbeit: Wo die Spec nicht indexierbare Seiten
//! voraussetzt, die nur P24 erkennt (`/noindex`, `/canonicalised`), setzt `simulate_indexability` die
//! Bewertung, die `os_model::indexability::evaluate` liefern muss, falls die Engine sie (noch) nicht
//! bestimmt hat. Statuscode-Fälle (301, 410, robots) bewertet die Engine selbst.

use std::path::{Path, PathBuf};

use os_analyze::analysis::{self, CancelToken};
use os_analyze::views::{
    filter_counts, query_view, state_of, ColumnSelection, ViewPage, ViewQuery,
};
use os_engine::testing::{crawl_fixture_from, FixtureCrawl};
use os_model::time::{IdSource, SeededIds};
use os_model::{CrawlConfig, CrawlMode, FilterState, StepState, Timestamp, Value};
use os_store::{CrawlFile, CrawlInit, StorageProfile};
use os_testsite::golden;

const BASE: &str = "http://site.test";

fn golden_dir(scenario: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/golden/tech")
        .join(scenario)
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

fn page(file: &CrawlFile, cfg: &CrawlConfig, tab: &str, filter: &str, all: bool) -> ViewPage {
    let mut q = ViewQuery::new(tab, filter);
    q.limit = 500;
    if all {
        q.columns = ColumnSelection::All;
    }
    query_view(file, cfg, &q).unwrap()
}

fn list(file: &CrawlFile, cfg: &CrawlConfig, tab: &str, filter: &str) -> Vec<String> {
    page(file, cfg, tab, filter, false)
        .rows
        .iter()
        .map(|r| text(&r[0]).replace(BASE, ""))
        .collect()
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

/// Spec-abgeleitete Erwartung und Golden-Datei einer Adressliste des Tabs „Sitemaps“.
fn check(scenario: &str, file: &CrawlFile, cfg: &CrawlConfig, filter: &str, expected: &[&str]) {
    let got = list(file, cfg, "Sitemaps", filter);
    assert_eq!(got, expected, "Sitemaps:{filter}");
    let mut body = String::from("Address\n");
    for a in &got {
        body.push_str(a);
        body.push('\n');
    }
    golden::assert_golden(
        golden_dir(scenario).join(format!("sitemaps__{}.csv", slug(filter))),
        &body,
    );
}

/// Zähler der Tabs „Sitemaps“ und „Hreflang“ (eine Zeile je Filter; Zustand statt Zahl, wenn nicht bereit).
fn counts(file: &CrawlFile, cfg: &CrawlConfig, tab: &str) -> String {
    let mut out = String::new();
    for c in filter_counts(file, cfg)
        .unwrap()
        .into_iter()
        .filter(|c| c.tab == tab)
    {
        let value = match (&c.state, c.count) {
            (FilterState::Ready, Some(n)) => n.to_string(),
            (state, _) => state.as_str().to_string(),
        };
        out.push_str(&format!("{}: {value}\n", c.filter));
    }
    out
}

fn run_analysis(file: &CrawlFile, cfg: &CrawlConfig) {
    let outcome = analysis::run(file, cfg, &CancelToken::new(), &|_, _| {}).unwrap();
    assert!(outcome.complete);
}

/// Setzt die Bewertung, die P24 für noindex bzw. Canonical liefert, falls die Engine sie noch nicht kennt.
fn simulate_indexability(file: &CrawlFile) {
    let conn = file.conn();
    for path in ["/noindex", "/noindex-not-in-sitemap"] {
        conn.execute(
            "UPDATE urls SET indexability = 'Non-Indexable', indexability_status = 'noindex' \
             WHERE address = ?1 AND state = 'done' AND indexability = 'Indexable'",
            [format!("{BASE}{path}")],
        )
        .unwrap();
    }
    conn.execute(
        "UPDATE urls SET indexability = 'Non-Indexable', indexability_status = 'Canonicalised' \
         WHERE address = ?1 AND state = 'done' AND indexability = 'Indexable'",
        [format!("{BASE}/canonicalised")],
    )
    .unwrap();
}

fn cfg_sitemaps() -> CrawlConfig {
    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = 1;
    // robots.txt → Sitemap-Index (CRAWL-058, TECH-210)
    cfg.sitemaps.crawl_linked = true;
    cfg
}

async fn crawl_sitemaps(cfg: &CrawlConfig) -> FixtureCrawl {
    let fx = crawl_fixture_from("tech/sitemaps", &format!("{BASE}/"), cfg.clone()).await;
    simulate_indexability(&fx.file);
    fx
}

// spec: TECH-005, TECH-211
#[tokio::test]
async fn post_crawl_filters_wait_for_the_analysis_and_all_is_live() {
    let cfg = cfg_sitemaps();
    let fx = crawl_sitemaps(&cfg).await;
    let post_crawl = [
        "URLs In Sitemap",
        "URLs Not In Sitemap",
        "Orphan URLs",
        "Non-Indexable URLs In Sitemap",
    ];
    for filter in post_crawl {
        assert_eq!(
            state_of(&fx.file, &cfg, "Sitemaps", filter).unwrap(),
            FilterState::NeedsCrawlAnalysis,
            "{filter}"
        );
        let p = page(&fx.file, &cfg, "Sitemaps", filter, false);
        assert_eq!(p.total, 0, "{filter}: nie Teilergebnisse");
    }
    assert_eq!(
        state_of(&fx.file, &cfg, "Sitemaps", "All").unwrap(),
        FilterState::Ready
    );
    assert!(!list(&fx.file, &cfg, "Sitemaps", "All").is_empty());
    golden::assert_golden(
        golden_dir("sitemaps").join("counts_before.txt"),
        &counts(&fx.file, &cfg, "Sitemaps"),
    );

    run_analysis(&fx.file, &cfg);
    for filter in post_crawl {
        assert_eq!(
            state_of(&fx.file, &cfg, "Sitemaps", filter).unwrap(),
            FilterState::Ready,
            "{filter}"
        );
    }
    golden::assert_golden(
        golden_dir("sitemaps").join("counts_after.txt"),
        &counts(&fx.file, &cfg, "Sitemaps"),
    );
}

// spec: TECH-211, TECH-212
#[tokio::test]
async fn urls_in_sitemap_compares_the_exact_url() {
    let cfg = cfg_sitemaps();
    let fx = crawl_sitemaps(&cfg).await;
    run_analysis(&fx.file, &cfg);
    // Einträge von sitemap-pages.xml und sitemap-posts.xml.gz (über robots.txt und den Index gefunden);
    // `/a/` steht in der Sitemap, `/a` nicht (zwei verschiedene URLs).
    let in_sitemap = [
        "/",
        "/a/",
        "/blocked",
        "/both",
        "/canonicalised",
        "/dup-same-file",
        "/gone",
        "/in-sitemap-and-linked",
        "/noindex",
        "/orphan-page",
        "/orphan.png",
        "/post-1",
        "/post-2",
        "/redirected",
    ];
    check("sitemaps", &fx.file, &cfg, "URLs In Sitemap", &in_sitemap);
    let all = list(&fx.file, &cfg, "Sitemaps", "All");
    assert!(all.contains(&"/a".to_string()), "/a ist gecrawlt");
    assert!(!in_sitemap.contains(&"/a"));

    // Population: interne URLs der Tabellen, keine Sitemap-Dateien.
    assert!(all
        .iter()
        .all(|a| !a.contains("sitemap_index") && !a.contains("sitemap-pages")));

    // Spalte „Sitemaps“: /both steht in zwei Dateien, /dup-same-file zweimal in derselben Datei (zählt einmal).
    let p = page(&fx.file, &cfg, "Sitemaps", "All", true);
    assert_eq!(cell(&p, "/both", "Sitemaps"), "2");
    assert_eq!(cell(&p, "/dup-same-file", "Sitemaps"), "1");
    assert_eq!(cell(&p, "/not-in-sitemap", "Sitemaps"), "0");
    // Referenzspalten
    let default = page(&fx.file, &cfg, "Sitemaps", "All", false);
    assert_eq!(
        default.columns,
        [
            "Address",
            "Content Type",
            "Status Code",
            "Status",
            "Indexability",
            "Indexability Status"
        ]
    );
}

// spec: CRAWL-060, TECH-210
#[tokio::test]
async fn sitemap_index_and_gzip_files_are_resolved_from_robots_txt() {
    let cfg = cfg_sitemaps();
    let fx = crawl_sitemaps(&cfg).await;
    let mut stmt = fx
        .file
        .conn()
        .prepare(
            "SELECT s.url, s.kind, s.entries, s.depth, COALESCE(p.url, '') FROM sitemaps s \
             LEFT JOIN sitemaps p ON p.id = s.parent_id ORDER BY s.depth, s.url",
        )
        .unwrap();
    let rows: Vec<(String, String, i64, i64, String)> = stmt
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let index = format!("{BASE}/sitemap_index.xml");
    assert_eq!(
        rows,
        [
            (index.clone(), "index".to_string(), 2, 0, String::new()),
            (
                format!("{BASE}/sitemap-pages.xml"),
                "urlset".to_string(),
                13,
                1,
                index.clone()
            ),
            (
                format!("{BASE}/sitemap-posts.xml.gz"),
                "urlset".to_string(),
                3,
                1,
                index
            ),
        ]
    );
}

// spec: TECH-213
#[tokio::test]
async fn urls_not_in_sitemap_lists_indexable_html_pages_only() {
    let cfg = cfg_sitemaps();
    let fx = crawl_sitemaps(&cfg).await;
    run_analysis(&fx.file, &cfg);
    // `/noindex-not-in-sitemap` (noindex), `/redirected`, `/gone` und Bilder fehlen. Folge-Seiten der Orphans
    // sind gecrawlte, indexierbare HTML-Seiten und stehen nicht in einer Sitemap.
    check(
        "sitemaps",
        &fx.file,
        &cfg,
        "URLs Not In Sitemap",
        &[
            "/a",
            "/not-in-sitemap",
            "/orphan-child",
            "/orphan-grandchild",
            "/shared-child",
        ],
    );
}

// spec: TECH-213, TECH-211
#[tokio::test]
async fn without_a_processed_sitemap_the_tab_is_empty_and_not_in_sitemap_reports_nothing() {
    let mut cfg = cfg_sitemaps();
    cfg.sitemaps.crawl_linked = false; // robots.txt-Sitemaps werden nicht verfolgt
    let fx = crawl_sitemaps(&cfg).await;
    run_analysis(&fx.file, &cfg);
    assert!(list(&fx.file, &cfg, "Sitemaps", "All").is_empty());
    for filter in [
        "URLs In Sitemap",
        "URLs Not In Sitemap",
        "Orphan URLs",
        "Non-Indexable URLs In Sitemap",
    ] {
        assert!(
            list(&fx.file, &cfg, "Sitemaps", filter).is_empty(),
            "{filter}"
        );
    }
    golden::assert_golden(
        golden_dir("sitemaps").join("counts_without-sitemap.txt"),
        &counts(&fx.file, &cfg, "Sitemaps"),
    );
}

// spec: TECH-214
#[tokio::test]
async fn orphans_include_follow_up_orphans_and_exclude_linked_pages_and_resources() {
    let cfg = cfg_sitemaps();
    let fx = crawl_sitemaps(&cfg).await;
    run_analysis(&fx.file, &cfg);
    // /orphan-page steht nur in der Sitemap; /orphan-child wird nur von dort verlinkt, /orphan-grandchild nur
    // von /orphan-child. /shared-child ist zusätzlich von der Startseite verlinkt (kein Orphan), ebenso
    // /in-sitemap-and-linked. Nicht indexierbare (/noindex) und Nicht-HTML-URLs (/orphan.png) fehlen.
    check(
        "sitemaps",
        &fx.file,
        &cfg,
        "Orphan URLs",
        &["/orphan-child", "/orphan-grandchild", "/orphan-page"],
    );

    // Found Via: Wurzel-Orphans leer, Folge-Orphans nennen die Wurzel-Orphan-URL.
    let all = page(&fx.file, &cfg, "Sitemaps", "Orphan URLs", true);
    assert_eq!(cell(&all, "/orphan-page", "Found Via"), "");
    assert_eq!(
        cell(&all, "/orphan-child", "Found Via"),
        format!("{BASE}/orphan-page")
    );
    assert_eq!(
        cell(&all, "/orphan-grandchild", "Found Via"),
        format!("{BASE}/orphan-page")
    );

    // Bei „Ignore Non-Indexable URLs for Issues“ aus erscheint auch die nicht indexierbare Wurzel /noindex.
    let mut keep = cfg.clone();
    keep.issues.ignore_non_indexable = false;
    assert_eq!(
        list(&fx.file, &keep, "Sitemaps", "Orphan URLs"),
        [
            "/noindex",
            "/orphan-child",
            "/orphan-grandchild",
            "/orphan-page"
        ]
    );
    // Ressourcen nie: das Bild /orphan.png steht in einer Sitemap, ist aber keine Seite.
    let flagged: i64 = fx
        .file
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM url_flags f JOIN urls u ON u.id = f.url_id \
             WHERE f.flag = 'orphan' AND u.content_class NOT IN ('HTML', 'PDF')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(flagged, 0);
}

// spec: TECH-214
#[tokio::test]
async fn orphan_result_is_deterministic_and_a_rerun_changes_nothing() {
    let cfg = cfg_sitemaps();
    let fx = crawl_sitemaps(&cfg).await;
    run_analysis(&fx.file, &cfg);
    let flags = |fx: &FixtureCrawl| -> Vec<(i64, String, Option<String>)> {
        let mut stmt = fx
            .file
            .conn()
            .prepare("SELECT url_id, flag, value FROM url_flags WHERE flag IN ('in_sitemap', 'orphan') ORDER BY url_id, flag")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    let first = flags(&fx);
    let seq: String = fx.file.get_meta("change_seq").unwrap().unwrap();
    run_analysis(&fx.file, &cfg);
    assert_eq!(flags(&fx), first, "gleiche Eingabe, gleiche Marken");
    assert_eq!(
        fx.file.get_meta("change_seq").unwrap().unwrap(),
        seq,
        "unveränderte Marken stempeln den Änderungsfeed nicht erneut"
    );
}

// spec: TECH-215
#[tokio::test]
async fn non_indexable_urls_in_sitemap_lists_fetched_urls_only() {
    let cfg = cfg_sitemaps();
    let fx = crawl_sitemaps(&cfg).await;
    run_analysis(&fx.file, &cfg);
    // 301, 410, noindex, canonicalised und robots-gesperrt (jeder Grund aus TECH-001).
    check(
        "sitemaps",
        &fx.file,
        &cfg,
        "Non-Indexable URLs In Sitemap",
        &[
            "/blocked",
            "/canonicalised",
            "/gone",
            "/noindex",
            "/redirected",
        ],
    );
    // Nicht abgerufene Sitemap-URLs (z. B. durch ein Limit) werden nicht gelistet.
    fx.file
        .conn()
        .execute(
            "UPDATE urls SET state = 'discovered' WHERE address = ?1",
            [format!("{BASE}/gone")],
        )
        .unwrap();
    let got = list(&fx.file, &cfg, "Sitemaps", "Non-Indexable URLs In Sitemap");
    assert!(!got.contains(&"/gone".to_string()));
    assert_eq!(got.len(), 4);
}

// --- tech/crawl-analysis (TECH-005) -------------------------------------------------------------------

fn cfg_analysis() -> CrawlConfig {
    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = 1;
    cfg.spider.hreflang = os_model::StoreCrawl::ON;
    cfg.sitemaps.urls = vec![format!("{BASE}/sitemap.xml")];
    cfg
}

async fn crawl_analysis(cfg: &CrawlConfig) -> FixtureCrawl {
    crawl_fixture_from("tech/crawl-analysis", &format!("{BASE}/"), cfg.clone()).await
}

// spec: TECH-005, TECH-214, TECH-076
#[tokio::test]
async fn live_filters_count_without_analysis_while_orphans_wait_for_it() {
    let cfg = cfg_analysis();
    let fx = crawl_analysis(&cfg).await;

    // Vor der Analyse: der hreflang-Rücklink fehlt (/en/ → /nl/ ohne Gegenstück) und ist schon gezählt;
    // die Orphan-Filter zeigen „Crawl-Analyse erforderlich“ statt Zahlen.
    assert_eq!(
        list(&fx.file, &cfg, "Hreflang", "Missing Return Links"),
        ["/en/"]
    );
    assert_eq!(
        state_of(&fx.file, &cfg, "Hreflang", "Missing Return Links").unwrap(),
        FilterState::Ready
    );
    assert_eq!(
        state_of(&fx.file, &cfg, "Sitemaps", "Orphan URLs").unwrap(),
        FilterState::NeedsCrawlAnalysis
    );
    assert!(list(&fx.file, &cfg, "Sitemaps", "Orphan URLs").is_empty());
    golden::assert_golden(
        golden_dir("crawl-analysis").join("counts_before.txt"),
        &format!(
            "{}{}",
            counts(&fx.file, &cfg, "Hreflang"),
            counts(&fx.file, &cfg, "Sitemaps")
        ),
    );

    // Nach der Analyse: genau ein Orphan (/nl/ ist per hreflang-Kante erreichbar und kein Orphan).
    run_analysis(&fx.file, &cfg);
    assert_eq!(
        state_of(&fx.file, &cfg, "Sitemaps", "Orphan URLs").unwrap(),
        FilterState::Ready
    );
    check(
        "crawl-analysis",
        &fx.file,
        &cfg,
        "Orphan URLs",
        &["/sitemap-orphan/"],
    );
    assert_eq!(
        list(&fx.file, &cfg, "Hreflang", "Missing Return Links"),
        ["/en/"],
        "unverändert"
    );
    golden::assert_golden(
        golden_dir("crawl-analysis").join("counts_after.txt"),
        &format!(
            "{}{}",
            counts(&fx.file, &cfg, "Hreflang"),
            counts(&fx.file, &cfg, "Sitemaps")
        ),
    );
}

// spec: TECH-005
#[tokio::test]
async fn a_deselected_step_keeps_its_filters_pending() {
    let mut cfg = cfg_analysis();
    cfg.analysis.sitemaps = false;
    let fx = crawl_analysis(&cfg).await;
    run_analysis(&fx.file, &cfg);

    let state: String = fx
        .file
        .conn()
        .query_row(
            "SELECT state FROM analysis_steps WHERE step = 'sitemaps'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(state, StepState::Skipped.as_str());
    for filter in [
        "URLs In Sitemap",
        "URLs Not In Sitemap",
        "Orphan URLs",
        "Non-Indexable URLs In Sitemap",
    ] {
        assert_eq!(
            state_of(&fx.file, &cfg, "Sitemaps", filter).unwrap(),
            FilterState::NeedsCrawlAnalysis,
            "{filter}"
        );
        assert!(
            list(&fx.file, &cfg, "Sitemaps", filter).is_empty(),
            "{filter}"
        );
    }
    // Live-Filter bleiben davon unberührt.
    assert_eq!(
        list(&fx.file, &cfg, "Hreflang", "Missing Return Links"),
        ["/en/"]
    );
    golden::assert_golden(
        golden_dir("crawl-analysis").join("counts_sitemaps-off.txt"),
        &format!(
            "{}{}",
            counts(&fx.file, &cfg, "Hreflang"),
            counts(&fx.file, &cfg, "Sitemaps")
        ),
    );
}

// spec: TECH-005
#[tokio::test]
async fn a_cancelled_run_leaves_the_step_pending_without_partial_results() {
    let cfg = cfg_analysis();
    let fx = crawl_analysis(&cfg).await;
    let cancel = CancelToken::new();
    cancel.cancel();
    let outcome = analysis::run(&fx.file, &cfg, &cancel, &|_, _| {}).unwrap();
    assert!(!outcome.complete);
    assert_eq!(
        state_of(&fx.file, &cfg, "Sitemaps", "Orphan URLs").unwrap(),
        FilterState::NeedsCrawlAnalysis
    );
    let flags: i64 = fx
        .file
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM url_flags WHERE flag IN ('in_sitemap', 'orphan')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(flags, 0);
}

// --- Schritt auf synthetischen Graphen ---------------------------------------------------------------

fn synthetic_file(dir: &Path, mode: CrawlMode) -> CrawlFile {
    let created = Timestamp::from_db("2026-10-13T07:35:00.000Z").unwrap();
    let init = CrawlInit {
        crawl_id: SeededIds::new(1).new_crawl_id(created),
        mode,
        start_url: Some("https://site.test/".into()),
        list_source: None,
        project: "Projekt".into(),
        name: "sitemaps".into(),
        config: CrawlConfig::default(),
        created_at: created,
    };
    CrawlFile::create(&dir.join("c.ospider"), &init, StorageProfile::Balanced).unwrap()
}

/// Seiten 1 (Start) … 6; Kanten: 1→2, 2→3 (Hyperlink), 4→5 (Hyperlink), 1→6 nur als Resource-Link (Bild).
/// Sitemap enthält 2 und 4. Erwartung: Orphans 4 (Wurzel) und 5 (Folge); 6 ist weder in R noch Folge-Orphan.
fn graph(file: &CrawlFile) {
    let conn = file.conn();
    for id in 1..=6 {
        conn.execute(
            "INSERT INTO urls (id, url, address, origin, is_internal, state, discovered_via, folder_depth, \
             url_length, status_code, content_class) VALUES (?1, ?2, ?2, 'https://site.test', 1, 'done', \
             ?3, 1, 20, 200, 'HTML')",
            rusqlite::params![
                id,
                format!("https://site.test/p{id}"),
                if id == 1 { "start" } else { "link" }
            ],
        )
        .unwrap();
    }
    let links = [
        (1, 2, "Hyperlink"),
        (2, 3, "Hyperlink"),
        (4, 5, "Hyperlink"),
        (1, 6, "Image"),
    ];
    for (n, (from, to, kind)) in links.into_iter().enumerate() {
        conn.execute(
            "INSERT INTO links (source_id, target_id, target_raw, link_type, position, follow) \
             VALUES (?1, ?2, 'x', ?3, ?4, 1)",
            rusqlite::params![from, to, kind, n as i64],
        )
        .unwrap();
    }
    conn.execute(
        "INSERT INTO sitemaps (id, url, kind, depth, found_via, entries) \
         VALUES (1, 'https://site.test/sitemap.xml', 'urlset', 0, 'start', 2)",
        [],
    )
    .unwrap();
    for url_id in [2, 4] {
        conn.execute(
            "INSERT INTO sitemap_entries (sitemap_id, url_id, loc_raw) VALUES (1, ?1, 'x')",
            [url_id],
        )
        .unwrap();
    }
}

fn flag_ids(file: &CrawlFile, flag: &str) -> Vec<(i64, Option<String>)> {
    let mut stmt = file
        .conn()
        .prepare("SELECT url_id, value FROM url_flags WHERE flag = ?1 ORDER BY url_id")
        .unwrap();
    stmt.query_map([flag], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

// spec: TECH-214
#[test]
fn orphan_step_walks_hyperlink_edges_but_not_resource_links() {
    let dir = tempfile::tempdir().unwrap();
    let file = synthetic_file(dir.path(), CrawlMode::Spider);
    graph(&file);
    let state = os_analyze::analysis::steps::sitemaps::run(
        &file,
        &CrawlConfig::default(),
        &CancelToken::new(),
    )
    .unwrap();
    assert_eq!(state, StepState::Done);
    assert_eq!(
        flag_ids(&file, "in_sitemap"),
        [(2, None), (4, None)],
        "beide Sitemap-Einträge"
    );
    // 2 steht in der Sitemap, ist aber von 1 aus verlinkt (R); 4 ist Wurzel-Orphan, 5 Folge-Orphan über 4.
    assert_eq!(
        flag_ids(&file, "orphan"),
        [(4, None), (5, Some("4".to_string()))]
    );
}

// spec: TECH-214
#[test]
fn orphan_step_reports_nothing_in_list_mode() {
    let dir = tempfile::tempdir().unwrap();
    let file = synthetic_file(dir.path(), CrawlMode::List);
    graph(&file);
    let state = os_analyze::analysis::steps::sitemaps::run(
        &file,
        &CrawlConfig::default(),
        &CancelToken::new(),
    )
    .unwrap();
    assert_eq!(state, StepState::Done);
    assert!(flag_ids(&file, "orphan").is_empty());
    assert_eq!(flag_ids(&file, "in_sitemap").len(), 2);
}

// spec: TECH-214
#[test]
fn orphan_step_clears_marks_that_no_longer_apply() {
    let dir = tempfile::tempdir().unwrap();
    let file = synthetic_file(dir.path(), CrawlMode::Spider);
    graph(&file);
    let run = || {
        os_analyze::analysis::steps::sitemaps::run(
            &file,
            &CrawlConfig::default(),
            &CancelToken::new(),
        )
        .unwrap()
    };
    run();
    assert_eq!(flag_ids(&file, "orphan").len(), 2);
    // Die Seite 1 verlinkt nun auch 4: 4 und 5 sind erreichbar, die Marken verschwinden.
    file.conn()
        .execute(
            "INSERT INTO links (source_id, target_id, target_raw, link_type, position, follow) \
             VALUES (1, 4, 'x', 'Hyperlink', 9, 1)",
            [],
        )
        .unwrap();
    run();
    assert!(flag_ids(&file, "orphan").is_empty());
    assert_eq!(flag_ids(&file, "in_sitemap").len(), 2);
}
