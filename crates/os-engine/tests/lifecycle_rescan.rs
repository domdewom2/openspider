//! „Neu crawlen“ und „Entfernen“ einzelner URLs (CRAWL-131).

#[path = "lifecycle_support.rs"]
mod support;

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use os_engine::{recrawl_urls, remove_urls, EngineDeps};
use os_model::{CrawlState, UrlId};
use os_store::{CrawlFile, OpenMode};
use support::*;

/// Kopie von `crawl-site-100` mit der umschaltbaren Seite `/wechsel.html` (verlinkt von der Startseite) und den
/// Zielseiten `/neu-a.html` … `/neu-c.html`.
fn switch_site() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(
        &os_testsite::fixtures_root().join("crawl-site-100"),
        dir.path(),
    );
    let index = std::fs::read_to_string(dir.path().join("index.html")).unwrap();
    std::fs::write(
        dir.path().join("index.html"),
        index.replace("</body>", "<a href=\"/wechsel.html\">Wechsel</a></body>"),
    )
    .unwrap();
    for n in ["a", "b", "c"] {
        std::fs::write(
            dir.path().join(format!("neu-{n}.html")),
            format!("<!doctype html><title>Neu {n}</title><h1>Neu {n}</h1>"),
        )
        .unwrap();
    }
    set_switch(dir.path(), true);
    dir
}

/// `true` = erster Zustand (Links auf neu-a, neu-b und page-1), `false` = zweiter (nur neu-c).
fn set_switch(dir: &Path, first: bool) {
    let links = if first {
        "<a href=\"/neu-a.html\">A</a><a href=\"/neu-b.html\">B</a><a href=\"/page-1.html\">1</a>"
    } else {
        "<a href=\"/neu-c.html\">C</a>"
    };
    std::fs::write(
        dir.join("wechsel.html"),
        format!("<!doctype html><title>Wechsel</title><h1>Wechsel</h1>{links}"),
    )
    .unwrap();
}

fn id_of(file: &CrawlFile, path: &str) -> UrlId {
    UrlId(
        file.conn()
            .query_row("SELECT id FROM urls WHERE url = ?1", [url(path)], |r| {
                r.get(0)
            })
            .unwrap_or_else(|e| panic!("{path}: {e}")),
    )
}

fn counters_are_consistent(path: &Path) {
    let file = CrawlFile::open(path, OpenMode::ReadOnly).unwrap();
    let bad = os_store::counters::verify_all(file.conn()).unwrap();
    assert!(
        bad.is_empty(),
        "Zähler weichen von der Neuberechnung ab: {bad:?}"
    );
}

struct Crawled {
    dir: tempfile::TempDir,
    work: tempfile::TempDir,
    path: std::path::PathBuf,
}

async fn baseline() -> Crawled {
    let dir = switch_site();
    let site = start_site(dir.path()).await;
    let work = tempfile::tempdir().unwrap();
    let path = crawl_file_in(&work, "crawl.ospider");
    let summary = run_to_end(start(config_for(&site, 3), &path, deps()).await).await;
    assert_eq!(summary.state, CrawlState::Completed);
    // die Analyse gilt als abgeschlossen (so, wie sie nach dem Crawl-Ende hinterlassen würde)
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    file.set_meta("analysis_state", "completed").unwrap();
    file.set_meta("analysis_stale", "0").unwrap();
    drop(file);
    Crawled { dir, work, path }
}

/// Deps mit einem Hook, der zählt, ob die Engine eine Analyse auslösen wollte.
fn counting_deps() -> (EngineDeps, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut d = deps();
    let c = Arc::clone(&calls);
    d.on_finished = Some(Arc::new(move |_| {
        c.fetch_add(1, Ordering::SeqCst);
    }));
    (d, calls)
}

// spec: CRAWL-131
#[tokio::test]
async fn recrawl_replaces_status_content_and_outlinks_and_corrects_the_inlink_counters() {
    let c = baseline().await;
    let file = CrawlFile::open(&c.path, OpenMode::ReadOnly).unwrap();
    let wechsel = id_of(&file, "/wechsel.html");
    let (neu_a, page1) = (id_of(&file, "/neu-a.html"), id_of(&file, "/page-1.html"));
    let inl = |file: &CrawlFile, id: UrlId| {
        scalar(
            file,
            &format!("SELECT inlinks FROM url_counters WHERE url_id = {}", id.0),
        )
    };
    let (a_before, p1_before) = (inl(&file, neu_a), inl(&file, page1));
    assert_eq!(
        scalar(
            &file,
            &format!(
                "SELECT outlinks FROM url_counters WHERE url_id = {}",
                wechsel.0
            )
        ),
        3
    );
    let seq_before = scalar(
        &file,
        "SELECT CAST(value AS INTEGER) FROM crawl_meta WHERE key = 'change_seq'",
    );
    drop(file);
    counters_are_consistent(&c.path);

    // die Seite wechselt: neue Links, die alten entfallen
    set_switch(c.dir.path(), false);
    let site = start_site(c.dir.path()).await;
    let (d, hook_calls) = counting_deps();
    let cfg = resume_config(&c.path, &site, |_| {});
    let fetcher = http_fetcher(&cfg, &d);
    recrawl_urls(fetcher, &c.path, &[wechsel], d).await.unwrap();

    let file = CrawlFile::open(&c.path, OpenMode::ReadOnly).unwrap();
    assert_eq!(
        scalar(
            &file,
            &format!(
                "SELECT outlinks FROM url_counters WHERE url_id = {}",
                wechsel.0
            )
        ),
        1,
        "nur noch /neu-c"
    );
    assert_eq!(
        scalar(
            &file,
            &format!("SELECT COUNT(*) FROM links WHERE source_id = {}", wechsel.0)
        ),
        1
    );
    assert_eq!(
        inl(&file, neu_a),
        a_before - 1,
        "entfallener Outlink: Inlinks von /neu-a sinken"
    );
    assert_eq!(inl(&file, page1), p1_before - 1);
    // der neue Outlink im Bereich ist eingereiht (Tiefe = Tiefe von /wechsel + 1), noch nicht abgerufen
    let (state, depth, wechsel_depth): (String, i64, i64) = file
        .conn()
        .query_row(
            &format!(
                "SELECT n.state, n.crawl_depth, w.crawl_depth FROM urls n, urls w WHERE n.url = ?1 AND w.id = {}",
                wechsel.0
            ),
            [url("/neu-c.html")],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(state, "queued");
    assert_eq!(depth, wechsel_depth + 1);
    assert_eq!(scalar(&file, "SELECT COUNT(*) FROM queue"), 1);
    assert_eq!(
        site.requests()
            .iter()
            .filter(|r| r.path() != "/robots.txt")
            .count(),
        1,
        "nur die eine Seite wurde abgerufen"
    );
    // Änderungsfeed
    let seq_after = scalar(
        &file,
        "SELECT CAST(value AS INTEGER) FROM crawl_meta WHERE key = 'change_seq'",
    );
    assert!(seq_after > seq_before);
    let changed = os_store::queries::changed_since(file.conn(), seq_before as u64, 1000).unwrap();
    assert!(changed.contains(&wechsel) && changed.contains(&neu_a) && changed.contains(&page1));
    // Analyse veraltet, keine automatische Analyse
    assert_eq!(
        file.get_meta("analysis_stale").unwrap().as_deref(),
        Some("1")
    );
    assert_eq!(
        file.get_meta("analysis_state").unwrap().as_deref(),
        Some("stale")
    );
    assert_eq!(
        hook_calls.load(Ordering::SeqCst),
        0,
        "Neu crawlen löst keine Analyse aus"
    );
    assert_eq!(file.meta().unwrap().state, CrawlState::Completed);
    drop(file);
    counters_are_consistent(&c.path);
    let file = CrawlFile::open(&c.path, OpenMode::ReadOnly).unwrap();
    assert!(file.integrity_check().unwrap());
}

// spec: CRAWL-131
#[tokio::test]
async fn recrawl_of_a_page_that_now_answers_404_drops_its_links() {
    let c = baseline().await;
    let file = CrawlFile::open(&c.path, OpenMode::ReadOnly).unwrap();
    let wechsel = id_of(&file, "/wechsel.html");
    drop(file);
    std::fs::remove_file(c.dir.path().join("wechsel.html")).unwrap();
    let site = start_site(c.dir.path()).await;
    let d = deps();
    let cfg = resume_config(&c.path, &site, |_| {});
    let fetcher = http_fetcher(&cfg, &d);
    recrawl_urls(fetcher, &c.path, &[wechsel], d).await.unwrap();

    let file = CrawlFile::open(&c.path, OpenMode::ReadOnly).unwrap();
    let (code, outlinks, title): (i64, i64, Option<String>) = file
        .conn()
        .query_row(
            &format!(
                "SELECT u.status_code, c.outlinks, u.title_1 FROM urls u JOIN url_counters c ON c.url_id = u.id WHERE u.id = {}",
                wechsel.0
            ),
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!((code, outlinks, title), (404, 0, None));
    assert_eq!(
        scalar(
            &file,
            &format!("SELECT COUNT(*) FROM links WHERE source_id = {}", wechsel.0)
        ),
        0
    );
    drop(file);
    counters_are_consistent(&c.path);
}

// spec: CRAWL-131
#[tokio::test]
async fn a_selection_of_many_urls_is_recrawled_in_one_go() {
    let c = baseline().await;
    let file = CrawlFile::open(&c.path, OpenMode::ReadOnly).unwrap();
    let ids: Vec<UrlId> = (1..=60)
        .map(|n| id_of(&file, &format!("/page-{n}.html")))
        .collect();
    let before = scalar(&file, "SELECT COUNT(*) FROM urls");
    let seen_before = scalar(&file, "SELECT COUNT(*) FROM urls WHERE state = 'done'");
    drop(file);

    let site = start_site(c.dir.path()).await;
    let d = deps();
    let cfg = resume_config(&c.path, &site, |c| c.speed.threads = 4);
    let fetcher = http_fetcher(&cfg, &d);
    recrawl_urls(fetcher, &c.path, &ids, d).await.unwrap();

    assert_eq!(
        site.requests()
            .iter()
            .filter(|r| r.path() != "/robots.txt")
            .count(),
        60,
        "jede ausgewählte URL genau einmal"
    );
    let file = CrawlFile::open(&c.path, OpenMode::ReadOnly).unwrap();
    assert_eq!(
        scalar(&file, "SELECT COUNT(*) FROM urls"),
        before,
        "keine neuen URLs, keine Duplikate"
    );
    assert_eq!(
        scalar(&file, "SELECT COUNT(*) FROM urls WHERE state = 'done'"),
        seen_before
    );
    assert_eq!(scalar(&file, "SELECT COUNT(*) FROM queue"), 0);
    drop(file);
    counters_are_consistent(&c.path);
}

// spec: CRAWL-131
#[tokio::test]
async fn remove_deletes_the_url_and_its_outlinks_and_leaves_inlinks_without_a_target() {
    let c = baseline().await;
    let file = CrawlFile::open(&c.path, OpenMode::ReadOnly).unwrap();
    let page1 = id_of(&file, "/page-1.html");
    let child = id_of(&file, "/page-4.html"); // verlinkt von page-1, page-0 (/) nicht
    let root = id_of(&file, "/");
    let child_inlinks = scalar(
        &file,
        &format!(
            "SELECT inlinks FROM url_counters WHERE url_id = {}",
            child.0
        ),
    );
    let root_outlinks = scalar(
        &file,
        &format!(
            "SELECT outlinks FROM url_counters WHERE url_id = {}",
            root.0
        ),
    );
    let inlinks_to_page1 = scalar(
        &file,
        &format!("SELECT COUNT(*) FROM links WHERE target_id = {}", page1.0),
    );
    assert!(inlinks_to_page1 >= 2, "{inlinks_to_page1}");
    let seq_before = scalar(
        &file,
        "SELECT CAST(value AS INTEGER) FROM crawl_meta WHERE key = 'change_seq'",
    );
    let total_before = scalar(&file, "SELECT COUNT(*) FROM urls");
    drop(file);

    remove_urls(&c.path, &[page1]).unwrap();

    let file = CrawlFile::open(&c.path, OpenMode::ReadOnly).unwrap();
    assert_eq!(scalar(&file, "SELECT COUNT(*) FROM urls"), total_before - 1);
    assert_eq!(
        scalar(
            &file,
            &format!("SELECT COUNT(*) FROM urls WHERE id = {}", page1.0)
        ),
        0
    );
    assert_eq!(
        scalar(
            &file,
            &format!("SELECT COUNT(*) FROM links WHERE source_id = {}", page1.0)
        ),
        0,
        "ausgehende Links weg"
    );
    // Inlinks bleiben als Links ohne Ziel (ohne Statuscode) stehen
    assert_eq!(
        scalar(
            &file,
            "SELECT COUNT(*) FROM links WHERE target_id IS NULL AND target_raw LIKE '%page-1.html'"
        ),
        inlinks_to_page1
    );
    // Zähler der Nachbarn sind neu berechnet: page-4 hat einen Inlink weniger, die Startseite einen Outlink weniger
    assert_eq!(
        scalar(
            &file,
            &format!(
                "SELECT inlinks FROM url_counters WHERE url_id = {}",
                child.0
            )
        ),
        child_inlinks - 1
    );
    assert_eq!(
        scalar(
            &file,
            &format!(
                "SELECT outlinks FROM url_counters WHERE url_id = {}",
                root.0
            )
        ),
        root_outlinks - 1
    );
    // Änderungsfeed: change_seq erhöht, reset_seq gleich (Löschungen sind sonst unsichtbar)
    let seq_after = scalar(
        &file,
        "SELECT CAST(value AS INTEGER) FROM crawl_meta WHERE key = 'change_seq'",
    );
    let reset = scalar(
        &file,
        "SELECT CAST(value AS INTEGER) FROM crawl_meta WHERE key = 'reset_seq'",
    );
    assert!(seq_after > seq_before);
    assert_eq!(reset, seq_after);
    assert_eq!(
        file.get_meta("analysis_stale").unwrap().as_deref(),
        Some("1")
    );
    assert_eq!(
        file.get_meta("analysis_state").unwrap().as_deref(),
        Some("stale")
    );
    assert!(file.integrity_check().unwrap());
    drop(file);
    counters_are_consistent(&c.path);
    let _ = &c.work;
}
