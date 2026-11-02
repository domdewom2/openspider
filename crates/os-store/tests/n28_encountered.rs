//! M2-N28: `Total URLs Encountered` nach der OS-Festlegung zu UI-061 (02.11.2026). Gezählt werden alle eindeutigen
//! normalisierten URLs, die der Crawl als Ziel kennengelernt hat, auch verborgene; nicht gezählt werden die
//! Hilfsabrufe (robots.txt-Zeilen nach K-54/CRAWL-098, Sitemap-Dateien). Eine Funktion
//! (`os_store::counters::encountered`) für Overview, Bericht, Statusleiste, Crawl-Liste und Fortschritt (CRAWL-130).

use os_model::time::{IdSource, SeededIds};
use os_model::{CrawlConfig, CrawlMode, Timestamp};
use os_store::{CrawlFile, CrawlInit, StorageProfile};

fn file(dir: &std::path::Path) -> CrawlFile {
    let at = Timestamp::from_db("2026-11-02T12:00:00.000Z").unwrap();
    let init = CrawlInit {
        crawl_id: SeededIds::new(28).new_crawl_id(at),
        mode: CrawlMode::Spider,
        start_url: Some("https://site.test/".into()),
        list_source: None,
        project: "Projekt".into(),
        name: "N28".into(),
        config: CrawlConfig::default(),
        created_at: at,
    };
    CrawlFile::create(&dir.join("n28.ospider"), &init, StorageProfile::Balanced).unwrap()
}

/// `(id, url, internal, in_tables, state, discovered_via, robots_blocked)`
type Row = (
    i64,
    &'static str,
    bool,
    bool,
    &'static str,
    &'static str,
    bool,
);

/// Alle Arten von Zeilen eines Crawls: angezeigt, verborgen (gefunden) und Hilfsabrufe.
const ROWS: [Row; 12] = [
    // angezeigt
    (1, "https://site.test/", true, true, "done", "start", false),
    (2, "https://site.test/a", true, true, "done", "link", false),
    (
        3,
        "https://other.test/x",
        false,
        true,
        "done",
        "link",
        false,
    ),
    // verborgen, aber gefunden: per Exclude ausgeschlossen, robots-gesperrt ohne Anzeige, nur per hreflang
    // bekannt (Speichern an, Crawlen aus), nur per Sitemap bekannt und noch in der Warteschlange (Limit)
    (
        4,
        "https://site.test/ausgeschlossen",
        true,
        false,
        "discovered",
        "link",
        false,
    ),
    (
        5,
        "https://site.test/privat/x",
        true,
        false,
        "done",
        "link",
        true,
    ),
    (
        6,
        "https://site.test/de/",
        true,
        false,
        "discovered",
        "link",
        false,
    ),
    (
        7,
        "https://site.test/nur-sitemap",
        true,
        false,
        "queued",
        "sitemap",
        false,
    ),
    // Hilfsabrufe: robots.txt beider Hosts (K-54), eine Sitemap-Datei
    (
        8,
        "https://site.test/robots.txt",
        true,
        false,
        "done",
        "robots_txt",
        false,
    ),
    (
        9,
        "https://other.test/robots.txt",
        false,
        false,
        "done",
        "robots_txt",
        false,
    ),
    (
        10,
        "https://site.test/sitemap.xml",
        true,
        false,
        "done",
        "sitemap",
        false,
    ),
    // Eine verlinkte und dadurch angezeigte Sitemap-Datei bzw. robots.txt ist eine gefundene URL
    // (`Total URLs Displayed` ≤ `Total URLs Encountered`).
    (
        11,
        "https://site.test/sitemap-2.xml",
        true,
        true,
        "done",
        "link",
        false,
    ),
    (
        12,
        "https://third.test/robots.txt",
        false,
        true,
        "done",
        "robots_txt",
        false,
    ),
];

fn crawl(file: &CrawlFile) {
    let tx = file.conn().unchecked_transaction().unwrap();
    for (id, url, internal, in_tables, state, via, blocked) in ROWS {
        let origin = url.splitn(4, '/').take(3).collect::<Vec<_>>().join("/");
        tx.execute(
            "INSERT INTO urls (id, url, address, origin, is_internal, in_tables, state, discovered_via, folder_depth, \
             url_length, status_code, content_class, robots_blocked) \
             VALUES (?1, ?2, ?2, ?3, ?4, ?5, ?6, ?7, 1, 30, ?8, 'HTML', ?9)",
            rusqlite::params![
                id,
                url,
                origin,
                internal,
                in_tables,
                state,
                via,
                (state == "done").then_some(if blocked { 0 } else { 200 }),
                blocked
            ],
        )
        .unwrap();
    }
    for (origin, robots) in [
        ("https://site.test", "https://site.test/robots.txt"),
        ("https://other.test", "https://other.test/robots.txt"),
        ("https://third.test", "https://third.test/robots.txt"),
    ] {
        tx.execute(
            "INSERT INTO robots_files (origin, robots_url, fetch_status, status_code, attempts, body) \
             VALUES (?1, ?2, 'ok', 200, 1, 'User-agent: *')",
            [origin, robots],
        )
        .unwrap();
    }
    for (id, url, url_id) in [
        (1, "https://site.test/sitemap.xml", 10),
        (2, "https://site.test/sitemap-2.xml", 11),
    ] {
        tx.execute(
            "INSERT INTO sitemaps (id, url, url_id, kind, depth, found_via) VALUES (?1, ?2, ?3, 'urlset', 0, 'config')",
            rusqlite::params![id, url, url_id],
        )
        .unwrap();
    }
    tx.commit().unwrap();
}

fn scalar(file: &CrawlFile, sql: &str) -> u64 {
    file.conn().query_row(sql, [], |r| r.get(0)).unwrap()
}

/// Die Definition als Lauf über alle Zeilen (Vergleichsmaßstab, nicht die Implementierung).
const BY_SCAN: &str = "SELECT COUNT(*) FROM urls WHERE NOT (in_tables = 0 AND \
    (discovered_via = 'robots_txt' OR id IN (SELECT url_id FROM sitemaps WHERE url_id IS NOT NULL)))";

// spec: UI-061
// spec: CRAWL-130
// spec: CRAWL-098
#[test]
fn encountered_counts_hidden_urls_but_no_robots_txt_or_sitemap_file_rows() {
    let dir = tempfile::tempdir().unwrap();
    let file = file(dir.path());
    crawl(&file);
    assert_eq!(scalar(&file, "SELECT COUNT(*) FROM urls"), 12);
    // 12 Zeilen − 2 verborgene robots.txt − 1 verborgene Sitemap-Datei
    assert_eq!(os_store::counters::encountered(file.conn()).unwrap(), 9);
    assert_eq!(scalar(&file, BY_SCAN), 9);
}

// spec: UI-061
// spec: CRAWL-130
// spec: UI-147 [P1]
#[test]
fn overview_status_bar_and_crawl_list_use_the_same_encountered_count() {
    let dir = tempfile::tempdir().unwrap();
    let file = file(dir.path());
    crawl(&file);
    let summary = os_store::queries::app_support::overview_summary(file.conn()).unwrap();
    let value = |name: &str| summary.iter().find(|(k, _)| k == name).unwrap().1;
    assert_eq!(summary[0].0, "Total URLs Encountered");
    assert_eq!(value("Total URLs Encountered"), 9);
    assert_eq!(value("Total URLs Displayed"), 5);
    assert!(value("Total URLs Displayed") <= value("Total URLs Encountered"));
    // gecrawlt (CRAWL-130): ohne robots.txt-Abrufe und Sitemap-Dateien, ohne gesperrte URLs
    assert_eq!(value("Total URLs Crawled"), 5);
    assert!(value("Total URLs Crawled") <= value("Total URLs Encountered"));
    // Statusleiste eines nicht von dieser App gesteuerten Crawls
    let counts = os_store::queries::session_counts(file.conn()).unwrap();
    assert_eq!(counts.discovered, 9);
    let path = file.path().to_path_buf();
    drop(file);
    // Crawl-Liste (`CrawlSummaryDto.urls`)
    assert_eq!(
        os_store::registry::read_listing(&path).unwrap().url_count,
        9
    );
}

// spec: UI-147 [P1]
// spec: UI-061
#[test]
fn encountered_never_scans_the_url_rows() {
    let dir = tempfile::tempdir().unwrap();
    let file = file(dir.path());
    crawl(&file);
    let sql = format!(
        "EXPLAIN QUERY PLAN SELECT (SELECT COUNT(*) FROM urls) - (SELECT COUNT(*) FROM ({}))",
        os_store::counters::HIDDEN_AUXILIARY_IDS
    );
    let mut stmt = file.conn().prepare(&sql).unwrap();
    let plan: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(3))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    // `COUNT(*)` über einen Index, je Hilfsabruf eine Suche über Primärschlüssel bzw. `UNIQUE(url)`; gelaufen
    // wird nur über die kleinen Tabellen `robots_files` und `sitemaps`.
    for line in plan.iter().filter(|l| l.starts_with("SCAN ")) {
        assert!(
            line.contains("COVERING INDEX")
                || line == "SCAN CONSTANT ROW"
                || line == "SCAN f"
                || line == "SCAN s",
            "{line}\n{plan:#?}"
        );
    }
    assert!(
        plan.iter().any(|l| l.starts_with("SEARCH r USING INDEX")),
        "{plan:#?}"
    );
}
