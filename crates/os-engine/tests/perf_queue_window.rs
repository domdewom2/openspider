//! RAM-Fenster der Warteschlange im vollen Crawl (K-78 b, PERF-003): Mit einem kleinen Fenster steht fast die ganze
//! Warteschlange nur in der Tabelle `queue` der Crawl-Datei, und der Crawl verhält sich trotzdem genau wie ohne
//! Auslagerung: gleiche Abrufreihenfolge (Breitensuche, CRAWL-005), gleiche Datei, kein Abruf doppelt, keiner verloren
//! (PERF-005). Pausieren und Fortsetzen (CRAWL-126) lädt nur das Fenster und kommt ohne den Index `links_target` aus
//! (K-74); die Ressourcen-Erkennung bleibt erhalten.
//!
//! Die Fenstergröße ist prozessweit überschreibbar (`queue::set_window_override`); die Tests dieser Datei laufen
//! deshalb nacheinander.

#[path = "lifecycle_support.rs"]
mod support;

use std::collections::BTreeMap;

use os_engine::queue::set_window_override;
use os_model::{CrawlState, EndReason};
use os_testsite::{SiteOptions, SyntheticConfig, TestSite};
use support::*;

/// Die Tests teilen sich die Überschreibung der Fenstergröße.
static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Kleines Fenster: bei dieser Site (zehn Links je Seite) steht die Warteschlange bald viele Male darüber.
const WINDOW: usize = 10;

fn synthetic() -> SyntheticConfig {
    SyntheticConfig {
        pages: 300,
        links_per_page: 10,
        seed: 17,
        status_301_share: 0.02,
        status_404_share: 0.02,
        status_5xx_share: 0.01,
        ..SyntheticConfig::default()
    }
}

async fn site() -> TestSite {
    site_of(synthetic()).await
}

async fn site_of(cfg: SyntheticConfig) -> TestSite {
    TestSite::start_synthetic(
        cfg,
        SiteOptions {
            port: 0,
            log_requests: true,
        },
    )
    .await
    .unwrap()
}

fn counters() -> BTreeMap<&'static str, u64> {
    os_engine::diag::snapshot().into_iter().collect()
}

/// Reihenfolge der Abrufe am Server (ohne robots.txt).
fn request_order(site: &TestSite) -> Vec<String> {
    site.requests()
        .into_iter()
        .filter(|r| r.path_and_query != "/robots.txt")
        .map(|r| r.path_and_query)
        .collect()
}

struct Run {
    order: Vec<String>,
    snapshot: Vec<String>,
    duplicates: Vec<(String, usize)>,
    queue_rows: i64,
    crawled: u64,
    peak_queued: u64,
    counters: BTreeMap<&'static str, u64>,
}

/// Ein Crawl mit einem Thread; `peak_queued` = größte Warteschlange (alle 2 ms abgetastet).
async fn crawl_once(window: Option<usize>) -> Run {
    set_window_override(window);
    os_engine::diag::reset();
    let site = site().await;
    let work = tempfile::tempdir().unwrap();
    let path = crawl_file_in(&work, "crawl.ospider");
    let handle = start(config_for(&site, 1), &path, deps()).await;
    let watch = handle.watch();
    let mut peak_queued = 0u64;
    let mut run = Box::pin(run_to_end(handle));
    let summary = loop {
        tokio::select! {
            s = &mut run => break s,
            _ = tokio::time::sleep(std::time::Duration::from_millis(2)) => {
                peak_queued = peak_queued.max(watch.borrow().queued);
            }
        }
    };
    set_window_override(None);
    assert_eq!(summary.end_reason, Some(EndReason::QueueEmpty));
    let file = open_ro(&path);
    Run {
        order: request_order(&site),
        snapshot: snapshot(&path),
        duplicates: duplicate_requests(&site),
        queue_rows: scalar(&file, "SELECT COUNT(*) FROM queue"),
        crawled: summary.crawled,
        peak_queued,
        counters: counters(),
    }
}

// spec: CRAWL-005, PERF-005, PERF-003
#[tokio::test]
async fn a_small_window_changes_neither_the_request_order_nor_the_result_and_bounds_the_ram() {
    let _serial = SERIAL.lock().await;
    let plain = crawl_once(None).await;
    let windowed = crawl_once(Some(WINDOW)).await;
    let c = &windowed.counters;
    assert!(windowed.crawled > 200, "{} Abrufe", windowed.crawled);
    // CRAWL-005: Breitensuche und Reihenfolge, Abruf für Abruf dieselbe Reihenfolge am Server (1 Thread);
    // dieselbe Datei: Kennungen, Zustände, Status, Tiefen (A6: gleiche Eingabe, gleiches Ergebnis)
    assert_eq!(windowed.order, plain.order);
    assert_eq!(windowed.snapshot, plain.snapshot);
    // PERF-005: kein Abruf doppelt, keiner verloren
    assert_eq!(windowed.duplicates, vec![], "kein Abruf doppelt");
    assert_eq!(
        windowed.order.len(),
        plain.order.len(),
        "kein Abruf verloren"
    );
    assert_eq!(windowed.crawled, plain.crawled);
    assert_eq!(
        windowed.queue_rows, 0,
        "die Warteschlange der Datei ist leer"
    );
    // es wurde wirklich ausgelagert und nachgeladen (die Zähler wurden vor dem Lauf zurückgesetzt)
    assert!(
        c["q_spilled"] > 0 && c["q_refills"] > 0 && c["q_loaded"] > 0,
        "{c:?}"
    );
    assert!(c["mem_queue_disk"] >= 50, "{c:?}");
    // M1-N22 (PERF-002): Dauer, Zeilen und Wartezeit je Nachladevorgang stehen in den Zählern; die Reihen je zehn
    // Nachladevorgänge addieren sich zu den Gesamtwerten
    assert_eq!(c["q_refill_jobs"], c["q_refills"], "{c:?}");
    assert!(c["q_refill_rows"] >= c["q_loaded"], "{c:?}");
    assert!(c["q_refill_ns"] >= c["q_refill_wait_ns"], "{c:?}");
    assert!(c["q_refill_wait_ns"] >= c["q_refill_wait_max_ns"], "{c:?}");
    let sum = |prefix: &str| {
        (0..16)
            .map(|i| c[format!("{prefix}_r{i:02}").as_str()])
            .sum::<u64>()
    };
    assert_eq!(sum("q_refill_n"), c["q_refill_jobs"], "{c:?}");
    assert_eq!(sum("q_refill_wait_ns"), c["q_refill_wait_ns"], "{c:?}");
    assert_eq!(sum("q_refill_rows"), c["q_refill_rows"], "{c:?}");
    // Der RAM-Anteil bleibt klein gegen die Warteschlange (die genaue Grenze prüft der deterministische Test in
    // `queue.rs`; hier hängt sie vom Takt des Writers ab).
    assert!(
        c["mem_queue_items"] * 2 < windowed.peak_queued,
        "RAM-Anteil {} bei Warteschlange {}",
        c["mem_queue_items"],
        windowed.peak_queued
    );
    assert!(
        windowed.peak_queued >= 8 * WINDOW as u64,
        "Warteschlange nur {} lang",
        windowed.peak_queued
    );
}

// spec: CRAWL-126
#[tokio::test]
async fn resuming_loads_only_the_window_and_needs_no_link_target_index() {
    let _serial = SERIAL.lock().await;
    set_window_override(Some(WINDOW));
    // Referenz: ununterbrochen
    let ref_site = site().await;
    let ref_work = tempfile::tempdir().unwrap();
    let ref_path = crawl_file_in(&ref_work, "ref.ospider");
    let reference = run_to_end(start(config_for(&ref_site, 1), &ref_path, deps()).await).await;
    assert_eq!(reference.end_reason, Some(EndReason::QueueEmpty));

    // Pause nach 100 URLs: die Warteschlange ist dann um ein Vielfaches größer als das Fenster
    let site = site().await;
    let work = tempfile::tempdir().unwrap();
    let path = crawl_file_in(&work, "crawl.ospider");
    let handle = start(config_for(&site, 1), &path, deps()).await;
    until_crawled(&handle, 100).await;
    handle.pause();
    let paused = run_to_end(handle).await;
    assert_eq!(paused.state, CrawlState::Paused);
    {
        // Der Index `links_target` fehlt während eines Crawls (K-74): hier ausdrücklich entfernen.
        let file = os_store::CrawlFile::open(&path, os_store::OpenMode::ReadWrite).unwrap();
        file.conn()
            .execute_batch("DROP INDEX IF EXISTS links_target")
            .unwrap();
        assert_eq!(
            scalar(
                &file,
                "SELECT COUNT(*) FROM sqlite_master WHERE name = 'links_target'"
            ),
            0
        );
        assert!(
            scalar(&file, "SELECT COUNT(*) FROM queue") > 4 * WINDOW as i64,
            "Warteschlange größer als das Fenster"
        );
    }

    let before = counters();
    let cfg = resume_config(&path, &site, |_| {});
    let resumed = run_to_end(resume(cfg, &path, deps()).await).await;
    let after = counters();
    set_window_override(None);
    assert_eq!(resumed.state, CrawlState::Completed);
    assert_eq!(resumed.end_reason, Some(EndReason::QueueEmpty));
    assert_eq!(resumed.crawled, reference.crawled);
    // Gesamtergebnis wie der ununterbrochene Crawl, kein Abruf doppelt
    assert_eq!(snapshot(&path), snapshot(&ref_path));
    assert_eq!(duplicate_requests(&site), vec![]);
    assert_eq!(site.requests().len(), ref_site.requests().len());
    // der Rest der Warteschlange kam aus der Datei
    assert!(after["q_refills"] > before["q_refills"], "{after:?}");
    assert!(
        after["q_loaded"] - before["q_loaded"] > 4 * WINDOW as u64,
        "{after:?}"
    );
}

/// Ohne Prüfung externer Ziele: Der Abruf von `https://cdn.other.test/…` gegen den Testserver (nur HTTP) liefe in die
/// Zeitgrenze des Verbindungsaufbaus (10 s).
fn internal_only(site: &TestSite) -> os_model::CrawlConfig {
    let mut cfg = config_for(site, 1);
    cfg.spider.external.crawl = false;
    cfg
}

/// `Accept` je Pfad (ohne robots.txt): Seiten und Ressourcen werden mit verschiedenen Werten abgerufen.
fn accepts(site: &TestSite) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for r in site.requests() {
        if r.path_and_query == "/robots.txt" {
            continue;
        }
        let accept = r
            .headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case("accept"))
            .map(|(_, v)| v.clone())
            .unwrap_or_default();
        out.entry(r.path_and_query).or_default().push(accept);
    }
    out
}

// spec: CRAWL-126
#[tokio::test]
async fn queued_resources_are_still_fetched_as_resources_after_a_resume_without_the_index() {
    let _serial = SERIAL.lock().await;
    // Fenster 2: schon die Warteschlange nach der Startseite ist größer als 2 x 2
    set_window_override(Some(2));
    let ref_site = TestSite::start("crawl-resources").await.unwrap();
    let ref_work = tempfile::tempdir().unwrap();
    let ref_path = crawl_file_in(&ref_work, "ref.ospider");
    run_to_end(start(internal_only(&ref_site), &ref_path, deps()).await).await;

    let site = TestSite::start("crawl-resources").await.unwrap();
    let work = tempfile::tempdir().unwrap();
    let path = crawl_file_in(&work, "crawl.ospider");
    // gedrosselt, damit die Pause greift, bevor die kleine Fixture fertig ist
    let mut slow = internal_only(&site);
    slow.speed.max_urls_per_sec = Some(20.0);
    let handle = start(slow, &path, deps()).await;
    until_crawled(&handle, 1).await;
    handle.pause();
    let paused = run_to_end(handle).await;
    assert_eq!(paused.state, CrawlState::Paused);
    {
        let file = os_store::CrawlFile::open(&path, os_store::OpenMode::ReadWrite).unwrap();
        file.conn()
            .execute_batch("DROP INDEX IF EXISTS links_target")
            .unwrap();
        assert!(
            scalar(&file, "SELECT COUNT(*) FROM queue") > 4,
            "Warteschlange nach der Pause zu klein für das Fenster"
        );
    }
    let cfg = resume_config(&path, &site, |c| c.speed.max_urls_per_sec = None);
    let resumed = run_to_end(resume(cfg, &path, deps()).await).await;
    set_window_override(None);
    assert_eq!(resumed.state, CrawlState::Completed);
    assert_eq!(snapshot(&path), snapshot(&ref_path));
    let (got, want) = (accepts(&site), accepts(&ref_site));
    assert!(
        want.values().flatten().any(|a| a.starts_with("*/*"))
            && want.values().flatten().any(|a| a.contains("text/html")),
        "die Fixture hat Seiten und Ressourcen: {want:?}"
    );
    assert_eq!(got, want, "Seiten und Ressourcen mit demselben Accept");
}

/// Crawl-Datei im Zustand „Pausiert“ mit 300 eingeplanten URLs (Tiefe 1 bis 3, `seq` = Kennung): zehn davon
/// (Kennungen 10 bis 19) sind Ziele von Bild-Links, sechs (20 bis 25) extern, zwei (5 und 6, Tiefe 3) Ziele einer
/// Weiterleitung. Der Index `links_target` fehlt.
fn crafted_file(path: &std::path::Path) {
    let init = os_store::CrawlInit {
        crawl_id: deps().ids.new_crawl_id(deps().clock.now()),
        mode: os_model::CrawlMode::Spider,
        start_url: Some("http://site.test/".to_string()),
        list_source: None,
        project: "window".to_string(),
        name: "window".to_string(),
        config: os_model::CrawlConfig::default(),
        created_at: deps().clock.now(),
    };
    let file =
        os_store::CrawlFile::create(path, &init, os_store::StorageProfile::Balanced).unwrap();
    file.conn()
        .execute_batch(
            "BEGIN;
             WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c WHERE x < 300)
             INSERT INTO urls (id, url, address, origin, is_internal, in_tables, state, discovered_via, crawl_depth, folder_depth, url_length)
               SELECT x, 'http://site.test/q/' || x, 'http://site.test/q/' || x, 'http://site.test', x NOT BETWEEN 20 AND 25, 1, 'queued',
                      CASE WHEN x IN (5, 6) THEN 'redirect' ELSE 'link' END,
                      CASE WHEN x IN (5, 6) THEN 3 ELSE 1 + (x % 3) END, 1, 25 FROM c;
             WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c WHERE x < 300)
             INSERT INTO queue (url_id, seq, depth, priority) SELECT x, x, CASE WHEN x IN (5, 6) THEN 3 ELSE 1 + (x % 3) END, 0 FROM c;
             INSERT INTO urls (id, url, address, origin, is_internal, in_tables, state, discovered_via, crawl_depth, folder_depth, url_length, redirect_target_id)
               VALUES (301, 'http://site.test/r/1', 'http://site.test/r/1', 'http://site.test', 1, 1, 'done', 'link', 1, 1, 20, 5),
                      (302, 'http://site.test/r/2', 'http://site.test/r/2', 'http://site.test', 1, 1, 'done', 'link', 1, 1, 20, 6);
             INSERT INTO links (source_id, target_id, target_raw, link_type, position, follow)
               SELECT 301, x, 'x', 'Image', x, 1 FROM (WITH RECURSIVE c(x) AS (SELECT 10 UNION ALL SELECT x + 1 FROM c WHERE x < 19) SELECT x FROM c);
             UPDATE crawl_meta SET value = 'Paused' WHERE key = 'state';
             DROP INDEX IF EXISTS links_target;
             COMMIT;",
        )
        .unwrap();
}

// spec: CRAWL-126, CRAWL-005
#[tokio::test]
async fn a_resumed_large_queue_runs_in_file_order_with_purposes_and_redirect_targets() {
    use os_fetch::FetchPurpose;
    let _serial = SERIAL.lock().await;
    set_window_override(Some(8));
    let work = tempfile::tempdir().unwrap();
    let path = crawl_file_in(&work, "gross.ospider");
    crafted_file(&path);
    let expected: Vec<String> = {
        let file = open_ro(&path);
        let mut stmt = file
            .conn()
            .prepare(
                "SELECT u.url FROM queue q JOIN urls u ON u.id = q.url_id ORDER BY q.priority, q.depth, q.seq",
            )
            .unwrap();
        stmt.query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert_eq!(expected.len(), 300);

    let sink = os_engine::testing::MemorySink::new();
    let mut d = deps();
    d.sink_factory = Some(sink.factory());
    let fetcher = os_engine::testing::SimFetcher::new(os_engine::testing::SimSite::new());
    let mut cfg = os_model::CrawlConfig::default();
    cfg.speed.threads = 1;
    let before = counters();
    let handle =
        os_engine::resume_crawl(std::sync::Arc::clone(&fetcher), path.clone(), Some(cfg), d)
            .await
            .unwrap();
    let summary = run_to_end(handle).await;
    let after = counters();
    set_window_override(None);
    assert_eq!(summary.end_reason, Some(EndReason::QueueEmpty));
    let fetched = fetcher.page_requests();
    // Abruf für Abruf in der Reihenfolge der Tabelle (Tiefe, dann Entdeckung), auch die Weiterleitungsziele 5 und 6
    // (sie bleiben im RAM und kommen an ihrer Stelle dran)
    assert_eq!(
        fetched.iter().map(|r| r.url.clone()).collect::<Vec<_>>(),
        expected
    );
    // Zweck: Bilder als Ressource, die sechs externen URLs als externe Prüfung, der Rest als Seite
    let purpose_of = |n: i64| {
        fetched
            .iter()
            .find(|r| r.url == format!("http://site.test/q/{n}"))
            .unwrap()
            .purpose
    };
    assert!((10..=19).all(|n| purpose_of(n) == FetchPurpose::Resource));
    assert!((20..=25).all(|n| purpose_of(n) == FetchPurpose::External));
    assert!([1, 2, 3, 7, 100, 299, 5, 6]
        .iter()
        .all(|n| purpose_of(*n) == FetchPurpose::Page));
    // Der Rest kam aus der Datei
    assert!(after["q_refills"] > before["q_refills"], "{after:?}");
    assert!(
        after["q_loaded"] - before["q_loaded"] > 200,
        "geladen: {}",
        after["q_loaded"] - before["q_loaded"]
    );
}
