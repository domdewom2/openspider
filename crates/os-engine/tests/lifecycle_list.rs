//! Listenmodus (CRAWL-027 Eingabequellen und Zähler, CRAWL-028 Verhalten).

#[path = "lifecycle_support.rs"]
mod support;

use std::path::Path;

use os_engine::list_input::{self, ListOutcome};
use os_engine::{start_crawl, CrawlHandle, CrawlRequest, CrawlSource};
use os_model::{CrawlConfig, CrawlMode, CrawlState, EndReason, RobotsMode};
use os_testsite::TestSite;
use support::*;

fn fixture(name: &str) -> std::path::PathBuf {
    os_testsite::fixtures_root().join(name)
}

fn items(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|s| s.to_string()).collect()
}

async fn start_list(cfg: CrawlConfig, file: &Path, items: Vec<String>, label: &str) -> CrawlHandle {
    let deps = deps();
    let fetcher = http_fetcher(&cfg, &deps);
    start_crawl(
        fetcher,
        CrawlRequest {
            config: cfg,
            source: CrawlSource::List {
                items,
                label: label.to_string(),
            },
            file: file.to_path_buf(),
            project: "liste".to_string(),
            name: "liste".to_string(),
        },
        deps,
    )
    .await
    .unwrap()
}

fn list_cfg(site: &TestSite) -> CrawlConfig {
    let mut cfg = list_input::list_config(CrawlConfig::default());
    cfg.speed.threads = 1;
    cfg.network.resolve = vec![format!("*.test={}", site.addr())];
    cfg
}

// ---------------------------------------------------------------------------------------------------
// CRAWL-027: Eingabequellen

// spec: CRAWL-027
#[test]
fn text_files_yield_all_http_strings_and_count_lines_without_a_url() {
    let p = list_input::read_list(&fixture("crawl-list-input").join("urls.txt")).unwrap();
    assert_eq!(
        p.urls,
        vec![
            "http://site.test/a",
            "https://site.test/b",
            "http://site.test/c",
            "HTTPS://SITE.test/d",
            "http://site.test/e?x=1&y=2"
        ],
        "zwei URLs in einer Zeile, HTTPS:// in Großschrift erkannt"
    );
    assert_eq!(
        p.ignored_lines, 3,
        "zwei Kommentarzeilen und `www.site.test/ohne-schema` (ohne Schema, Q27)"
    );
}

// spec: CRAWL-027
#[test]
fn csv_and_both_excel_sheets_are_searched_in_every_cell() {
    let dir = fixture("crawl-list-input");
    let csv = list_input::read_list(&dir.join("urls.csv")).unwrap();
    assert_eq!(
        csv.urls,
        vec![
            "http://site.test/a",
            "http://site.test/b",
            "http://site.test/c"
        ],
        "URLs in Spalte 3, ein Komma in einer anderen Zelle stört nicht"
    );
    assert_eq!(csv.ignored_lines, 1, "die Kopfzeile ohne URL");

    let xlsx = list_input::read_list(&dir.join("urls.xlsx")).unwrap();
    assert_eq!(
        xlsx.urls,
        vec![
            "http://site.test/a",
            "http://site.test/b",
            "http://site.test/c",
            "http://site.test/d",
            "https://site.test/e"
        ],
        "alle Zellen beider Blätter"
    );
    assert_eq!(xlsx.ignored_lines, 2, "zwei Kopfzeilen ohne URL (je Blatt)");
}

// spec: CRAWL-027
#[test]
fn sitemaps_and_indexes_including_gzip_are_read_by_their_loc_entries() {
    let dir = fixture("crawl-list-input");
    let index = list_input::read_list(&dir.join("sitemap-index.xml")).unwrap();
    assert_eq!(
        index.urls,
        vec![
            "http://site.test/sitemaps/s1.xml",
            "http://site.test/sitemaps/s2.xml.gz"
        ],
        "ein Index liefert die Kind-Sitemaps"
    );
    let s1 = list_input::read_list(&dir.join("sitemaps").join("s1.xml")).unwrap();
    assert_eq!(s1.urls, vec!["http://site.test/a", "http://site.test/b"]);
    let s2 = list_input::read_list(&dir.join("sitemaps").join("s2.xml.gz")).unwrap();
    assert_eq!(
        s2.urls,
        vec!["http://site.test/c", "http://site.test/d"],
        "gzip wird an den Magic-Bytes erkannt"
    );
}

// spec: CRAWL-027
#[test]
fn pasting_and_typing_the_same_text_give_the_same_counts() {
    let text = std::fs::read_to_string(fixture("crawl-list-input").join("dupes.txt")).unwrap();
    let cfg = CrawlConfig::default();
    let pasted = list_input::plan(&list_input::text_items(&text), &cfg).unwrap();
    let typed_lines: Vec<String> = text.lines().map(str::to_string).collect();
    let typed = list_input::plan(&typed_lines, &cfg).unwrap();
    assert_eq!(pasted.counts, typed.counts);
    assert_eq!(pasted.counts.read, 10);
    // parse_text liefert dieselben URLs
    assert_eq!(list_input::parse_text(&text).urls.len(), 10);
}

// spec: CRAWL-027
#[test]
fn dupes_fixture_with_an_exclude_shows_the_documented_counts() {
    let items = list_input::read_items(&fixture("crawl-list-input").join("dupes.txt")).unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.scope.exclude = vec![".*/privat/.*".to_string()];
    let plan = list_input::plan(&items, &cfg).unwrap();
    let c = plan.counts;
    assert_eq!(
        (c.read, c.duplicates, c.excluded, c.to_crawl),
        (10, 3, 1, 6),
        "Gelesen 10, Duplikate 3, Exclude 1, Zu crawlen 6"
    );
    assert_eq!((c.invalid, c.not_included, c.ignored_lines), (0, 0, 0));
    // Duplikate nutzen die normalisierte Form: `HTTP://SITE.test/a` ist ein Duplikat von `http://site.test/a`
    let dup = plan
        .entries
        .iter()
        .find(|e| e.original == "HTTP://SITE.test/a")
        .unwrap();
    assert_eq!(dup.outcome, ListOutcome::Duplicate);

    // Include: nur /a und /b
    let mut cfg = CrawlConfig::default();
    cfg.scope.include = vec![".*/(a|b)".to_string()];
    let plan = list_input::plan(&items, &cfg).unwrap();
    assert_eq!(plan.counts.not_included, 5);
    assert_eq!(
        plan.counts.to_crawl,
        plan.counts.read - plan.counts.duplicates - 5
    );
}

// spec: CRAWL-027
#[test]
fn the_duplicate_check_uses_the_normalised_form() {
    let items = items(&[
        "HTTP://SITE.test/a",
        "http://site.test:80/a",
        "http://site.test/a#anker",
        "http://site.test/b",
    ]);
    let plan = list_input::plan(&items, &CrawlConfig::default()).unwrap();
    assert_eq!(plan.counts.read, 4);
    assert_eq!(
        plan.counts.duplicates, 2,
        "Groß-/Kleinschreibung, Standardport und Fragment"
    );
    assert_eq!(plan.counts.to_crawl, 2);
}

// spec: CRAWL-027
#[test]
fn a_list_of_ten_thousand_lines_is_planned_and_every_line_is_accounted_for() {
    let lines: Vec<String> = (0..10_000)
        .map(|i| format!("http://site.test/seite/{}", i % 9_000))
        .collect();
    let plan = list_input::plan(&lines, &CrawlConfig::default()).unwrap();
    assert_eq!(plan.counts.read, 10_000);
    assert_eq!(plan.counts.duplicates, 1_000);
    assert_eq!(plan.counts.to_crawl, 9_000);
    assert_eq!(plan.entries.len(), 10_000);
}

// spec: CRAWL-027
#[test]
#[ignore = "Benchmark (P28): 100 000 Zeilen in unter 10 s einlesen und planen (Wanduhr, Referenzmaschine)"]
fn a_list_of_one_hundred_thousand_urls_is_read_in_under_ten_seconds() {
    let text: String = (0..100_000)
        .map(|i| format!("https://site.test/seite/{i}?x={}\n", i % 7))
        .collect();
    let started = std::time::Instant::now();
    let parse = list_input::parse_text(&text);
    let plan = list_input::plan(&list_input::text_items(&text), &CrawlConfig::default()).unwrap();
    let took = started.elapsed();
    assert_eq!(parse.urls.len(), 100_000);
    assert_eq!(plan.counts.to_crawl, 100_000);
    assert!(took < std::time::Duration::from_secs(10), "{took:?}");
}

// ---------------------------------------------------------------------------------------------------
// CRAWL-027 / CRAWL-028 im Crawl

// spec: CRAWL-027
#[tokio::test]
async fn a_list_crawl_requests_each_listed_url_once_and_fills_the_list_input_table() {
    let site = TestSite::start("crawl-list-input").await.unwrap();
    let work = tempfile::tempdir().unwrap();
    let path = crawl_file_in(&work, "liste.ospider");
    let mut cfg = list_cfg(&site);
    cfg.scope.exclude = vec![".*/privat/.*".to_string()];
    let all = list_input::read_items(&fixture("crawl-list-input").join("dupes.txt")).unwrap();
    let mut with_noise = all.clone();
    with_noise.insert(2, "kein Link in dieser Zeile".to_string());
    with_noise.push("http://".to_string());
    let summary = run_to_end(start_list(cfg, &path, with_noise, "dupes.txt").await).await;
    assert_eq!(summary.state, CrawlState::Completed);
    assert_eq!(summary.end_reason, Some(EndReason::QueueEmpty));

    let file = open_ro(&path);
    let meta = file.meta().unwrap();
    assert_eq!(meta.mode, CrawlMode::List);
    assert_eq!(meta.start_url, None);
    assert_eq!(
        file.get_meta("list_source").unwrap().as_deref(),
        Some("dupes.txt")
    );
    // Tabelle list_input: eine Zeile je Eintrag in Eingabereihenfolge
    let rows: Vec<(i64, String, String)> = {
        let mut stmt = file
            .conn()
            .prepare("SELECT position, original, outcome FROM list_input ORDER BY position")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert_eq!(
        rows.len(),
        12,
        "10 URLs + 1 Zeile ohne URL + 1 ungültige Adresse"
    );
    assert_eq!(
        rows[0],
        (1, "http://site.test/a".to_string(), "ok".to_string())
    );
    assert_eq!(rows[2].2, "ignored_line");
    let outcome_of = |o: &str| rows.iter().filter(|r| r.2 == o).count();
    assert_eq!(outcome_of("ok"), 6);
    assert_eq!(outcome_of("duplicate"), 3);
    assert_eq!(outcome_of("excluded"), 1);
    assert_eq!(outcome_of("invalid"), 1);
    assert_eq!(outcome_of("ignored_line"), 1);
    // Zähler für den Dialog stehen in crawl_counters
    let counter = |k: &str| {
        scalar(
            &file,
            &format!("SELECT value FROM crawl_counters WHERE key = '{k}'"),
        )
    };
    assert_eq!(
        (
            counter("list_read"),
            counter("list_duplicates"),
            counter("list_excluded"),
            counter("list_to_crawl")
        ),
        (11, 3, 1, 6)
    );
    // genau die 6 Listen-URLs wurden gecrawlt, jede einmal, alle in Tiefe 0
    assert_eq!(
        scalar(&file, "SELECT COUNT(*) FROM urls WHERE state = 'done'"),
        6
    );
    assert_eq!(
        scalar(&file, "SELECT COUNT(*) FROM urls WHERE crawl_depth = 0 AND discovered_via = 'list_input' AND state = 'done'"),
        6
    );
    assert_eq!(
        scalar(&file, "SELECT COUNT(*) FROM urls WHERE url LIKE '%/privat/%' AND state = 'discovered' AND not_crawled_reason = 'excluded'"),
        1
    );
    assert_eq!(duplicate_requests(&site), vec![]);
    assert!(
        site.requests().iter().all(|r| r.path() != "/robots.txt"),
        "Listenmodus: robots.txt ignorieren"
    );
    assert_eq!(site.requests().len(), 6);
}

// spec: CRAWL-027, CRAWL-141
#[tokio::test]
async fn sitemap_addresses_in_the_list_are_fetched_and_their_entries_become_the_list() {
    let site = TestSite::start("crawl-list-input").await.unwrap();
    let work = tempfile::tempdir().unwrap();
    let path = crawl_file_in(&work, "liste.ospider");
    let cfg = list_cfg(&site);
    let summary = run_to_end(
        start_list(
            cfg,
            &path,
            items(&["http://site.test/sitemap-index.xml"]),
            "sitemap",
        )
        .await,
    )
    .await;
    assert_eq!(summary.end_reason, Some(EndReason::QueueEmpty));
    let file = open_ro(&path);
    // Index + zwei Kind-Sitemaps, alle über die Liste gefunden
    assert_eq!(scalar(&file, "SELECT COUNT(*) FROM sitemaps"), 3);
    assert_eq!(
        scalar(
            &file,
            "SELECT COUNT(*) FROM sitemaps WHERE found_via = 'list'"
        ),
        3
    );
    assert_eq!(scalar(&file, "SELECT COUNT(*) FROM sitemap_entries"), 4);
    // a, b, c, d sind die Listen-URLs (Tiefe 0, gecrawlt); die Sitemap-Dateien erscheinen nicht in den Tabellen
    assert_eq!(
        scalar(&file, "SELECT COUNT(*) FROM urls WHERE discovered_via = 'list_input' AND crawl_depth = 0 AND state = 'done' AND in_tables = 1"),
        4
    );
    assert_eq!(
        scalar(
            &file,
            "SELECT COUNT(*) FROM urls u JOIN sitemaps s ON s.url_id = u.id WHERE u.in_tables = 1"
        ),
        0
    );
    let outcome: String = file
        .conn()
        .query_row(
            "SELECT outcome FROM list_input WHERE position = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(outcome, "sitemap");
    assert_eq!(duplicate_requests(&site), vec![]);
}

// ---------------------------------------------------------------------------------------------------
// CRAWL-028: Verhalten

fn behavior_items() -> Vec<String> {
    list_input::text_items(
        &std::fs::read_to_string(fixture("crawl-list-behavior").join("list.txt")).unwrap(),
    )
}

fn requested(site: &TestSite) -> Vec<String> {
    site.requests()
        .iter()
        .map(|r| format!("{}{}", r.host, r.path_and_query))
        .collect()
}

// spec: CRAWL-028
#[tokio::test]
async fn by_default_only_the_listed_urls_are_requested_and_robots_txt_is_ignored() {
    let site = TestSite::start("crawl-list-behavior").await.unwrap();
    let work = tempfile::tempdir().unwrap();
    let path = crawl_file_in(&work, "liste.ospider");
    let mut items = behavior_items();
    items.push("http://site.test/a".to_string()); // doppelt: einmal gecrawlt
    let summary = run_to_end(start_list(list_cfg(&site), &path, items, "list.txt").await).await;
    assert_eq!(summary.end_reason, Some(EndReason::QueueEmpty));
    let mut got = requested(&site);
    got.sort();
    assert_eq!(
        got,
        vec![
            "other.test/b".to_string(),
            "site.test/a".to_string(),
            "site.test/r301".to_string(),
            "site.test/robots-blocked".to_string(),
        ],
        "nur die Listen-URLs; weder robots.txt noch Kinder, Bild oder Weiterleitungsziel"
    );
    let file = open_ro(&path);
    // /robots-blocked wird trotz Disallow abgerufen (robots.txt ignoriert)
    assert_eq!(
        scalar(
            &file,
            "SELECT status_code FROM urls WHERE url = 'http://site.test/robots-blocked'"
        ),
        200
    );
    // Links aus den Seiten sind gespeichert, aber nicht angefragt: Grund Tiefenlimit im Listenmodus
    for target in [
        "http://site.test/a-kind",
        "http://site.test/bild.png",
        "http://site.test/ziel",
    ] {
        let (state, reason, tables): (String, Option<String>, i64) = file
            .conn()
            .query_row(
                "SELECT state, not_crawled_reason, in_tables FROM urls WHERE url = ?1",
                [target],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap_or_else(|e| panic!("{target}: {e}"));
        assert_eq!(
            (state.as_str(), reason.as_deref(), tables),
            ("discovered", Some("list_mode_depth"), 0),
            "{target}"
        );
    }
    assert_eq!(
        scalar(&file, "SELECT COUNT(*) FROM links WHERE source_id = (SELECT id FROM urls WHERE url = 'http://site.test/a')"),
        2,
        "Outlinks (Kind und Bild) stehen in der Datei"
    );
    // Alle Listen-URLs haben Tiefe 0; jeder Host der Liste ist intern, auch other.test
    assert_eq!(
        scalar(
            &file,
            "SELECT COUNT(*) FROM urls WHERE discovered_via = 'list_input' AND crawl_depth = 0"
        ),
        4
    );
    assert_eq!(
        scalar(
            &file,
            "SELECT is_internal FROM urls WHERE url = 'http://other.test/b'"
        ),
        1
    );
    assert_eq!(
        scalar(
            &file,
            "SELECT in_tables FROM urls WHERE url = 'http://other.test/b'"
        ),
        1
    );
    // die Weiterleitung selbst wird erfasst, ihr Ziel nicht verfolgt
    assert_eq!(
        scalar(
            &file,
            "SELECT status_code FROM urls WHERE url = 'http://site.test/r301'"
        ),
        301
    );
    assert_eq!(duplicate_requests(&site), vec![]);
}

// spec: CRAWL-028
#[tokio::test]
async fn with_depth_one_the_directly_linked_urls_are_checked_too() {
    let site = TestSite::start("crawl-list-behavior").await.unwrap();
    let work = tempfile::tempdir().unwrap();
    let path = crawl_file_in(&work, "liste.ospider");
    let mut cfg = list_cfg(&site);
    cfg.limits.max_depth = Some(1);
    let summary = run_to_end(start_list(cfg, &path, behavior_items(), "list.txt").await).await;
    assert_eq!(summary.end_reason, Some(EndReason::QueueEmpty));
    let mut got = requested(&site);
    got.sort();
    assert_eq!(
        got,
        vec![
            "other.test/b".to_string(),
            "other.test/b-kind".to_string(),
            "site.test/a".to_string(),
            "site.test/a-kind".to_string(),
            "site.test/bild.png".to_string(),
            "site.test/r301".to_string(),
            "site.test/robots-blocked".to_string(),
            "site.test/ziel".to_string(),
        ],
        "Tiefe 1: Kinder, Bild und Weiterleitungsziel kommen dazu"
    );
}

// spec: CRAWL-028
#[tokio::test]
async fn choosing_respect_reports_blocked_list_urls_as_blocked_by_robots() {
    let site = TestSite::start("crawl-list-behavior").await.unwrap();
    let work = tempfile::tempdir().unwrap();
    let path = crawl_file_in(&work, "liste.ospider");
    let mut cfg = list_cfg(&site);
    cfg.robots.mode = RobotsMode::Respect;
    run_to_end(start_list(cfg, &path, behavior_items(), "list.txt").await).await;
    let file = open_ro(&path);
    let (code, status, blocked, line): (i64, String, i64, Option<String>) = file
        .conn()
        .query_row(
            "SELECT status_code, status, robots_blocked, robots_matched_line FROM urls WHERE url = 'http://site.test/robots-blocked'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(
        (code, status.as_str(), blocked),
        (0, "Blocked by Robots.txt", 1)
    );
    assert!(line.is_some_and(|l| l.contains("/robots-blocked")));
    assert!(!requested(&site).contains(&"site.test/robots-blocked".to_string()));
    // erlaubte Listen-URLs laufen normal
    assert_eq!(
        scalar(
            &file,
            "SELECT status_code FROM urls WHERE url = 'http://site.test/a'"
        ),
        200
    );
}

// spec: CRAWL-028, CRAWL-126
#[tokio::test]
async fn a_paused_list_crawl_resumes_with_all_hosts_still_internal() {
    let dir = slow_site_dir(3);
    let site = start_site(dir.path()).await;
    let urls: Vec<String> = (1..=40)
        .map(|n| format!("http://site.test/page-{n}.html"))
        .collect();
    let work = tempfile::tempdir().unwrap();

    let ref_path = crawl_file_in(&work, "ref.ospider");
    run_to_end(start_list(list_cfg(&site), &ref_path, urls.clone(), "ref").await).await;

    let path = crawl_file_in(&work, "liste.ospider");
    let handle = start_list(list_cfg(&site), &path, urls, "liste").await;
    until_crawled(&handle, 10).await;
    handle.pause();
    let paused = run_to_end(handle).await;
    assert_eq!(paused.state, CrawlState::Paused);
    assert!(paused.crawled < 40);

    let cfg = resume_config(&path, &site, |_| {});
    let resumed = run_to_end(resume(cfg, &path, deps()).await).await;
    assert_eq!(resumed.state, CrawlState::Completed);
    assert_eq!(resumed.crawled, 40);
    assert_eq!(statuses(&path), statuses(&ref_path));
    assert_eq!(open_ro(&path).meta().unwrap().mode, CrawlMode::List);
}
