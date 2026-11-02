//! M2-N26: latency of `sidebar.issues` (dashboard card "Top-Probleme" and sidebar tab Issues) on a
//! finished crawl of the size reported from the beta test (about 4 000 internal URLs, half HTML, half
//! images, 1 600 external URLs, crawl analysis at the end).
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "session_common.rs"]
mod common;
use common::*;
use os_app::CallCtx;
use serde_json::json;
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const HTML: usize = 2_050;
const IMAGES: usize = 1_950;
const EXTERNAL: usize = 1_635;
/// 1×1 PNG.
const PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
];

/// Static site: `HTML` pages with ten internal links, one image and one external link each.
fn site(dir: &Path) {
    std::fs::create_dir_all(dir.join("p")).unwrap();
    std::fs::create_dir_all(dir.join("img")).unwrap();
    std::fs::write(dir.join("s.css"), "body{margin:0}").unwrap();
    std::fs::write(dir.join("a.js"), "void 0;").unwrap();
    for i in 0..IMAGES {
        std::fs::write(dir.join("img").join(format!("{i}.png")), PNG).unwrap();
    }
    let page = |i: usize| {
        let mut links = String::new();
        for k in 1..=10 {
            links.push_str(&format!(
                "<a href=\"/p/{}.html\">Seite {k}</a>\n",
                (i * 7 + k * 13) % HTML
            ));
        }
        // A few duplicate titles, missing descriptions and long titles, as on a real site.
        let title = if i.is_multiple_of(9) {
            "Produkt".to_owned()
        } else {
            format!("Produktseite Nummer {i} mit einem etwas längeren Titel für die Suche")
        };
        let desc = if i.is_multiple_of(5) {
            String::new()
        } else {
            format!("<meta name=\"description\" content=\"Beschreibung {i}\">")
        };
        format!(
            "<!doctype html><html lang=\"de\"><head><title>{title}</title>{desc}<link rel=\"stylesheet\" href=\"/s.css\"><script src=\"/a.js\"></script></head>\
             <body><h1>Überschrift {i}</h1><h2>Teil</h2><p>{}</p><img src=\"/img/{}.png\"{}>{links}\
             <a href=\"http://localhost:{{{{PORT2}}}}/ext/{}.html\">extern</a></body></html>",
            "Inhalt der Seite mit Text. ".repeat(20 + i % 40),
            i % IMAGES,
            if i.is_multiple_of(3) { "" } else { " alt=\"Bild\"" },
            i % EXTERNAL,
        )
    };
    for i in 0..HTML {
        std::fs::write(dir.join("p").join(format!("{i}.html")), page(i)).unwrap();
    }
    let mut index = String::from("<!doctype html><html><head><title>Start</title></head><body>");
    for i in (0..HTML).step_by(20) {
        index.push_str(&format!("<a href=\"/p/{i}.html\">{i}</a>"));
    }
    index.push_str("</body></html>");
    std::fs::write(dir.join("index.html"), index).unwrap();
}

#[derive(Default, Debug)]
struct Timeline {
    /// (seconds since start, issues version) of every version change.
    versions: Vec<(f64, u64)>,
    crawl_done: Option<f64>,
    analysis_done: Option<f64>,
}

async fn issues(app: &os_app::App) -> (Duration, usize) {
    let before = Instant::now();
    let rows = call(app, "sidebar.issues", json!({})).await;
    (before.elapsed(), rows.as_array().unwrap().len())
}

// spec: UI-145
// spec: UI-147 [P1]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Benchmark: M2-N26 Messung sidebar.issues nach Crawl-Ende (cargo test -p os-app --test n26_issues_latency -- --ignored --nocapture)"]
async fn issues_latency_after_a_finished_crawl_of_beta_size() {
    if let Ok(filter) = std::env::var("N26_TRACE") {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
            .with_ansi(false)
            .with_writer(std::io::stderr)
            .try_init();
    }
    let root = tempfile::tempdir().unwrap();
    let www = root.path().join("www");
    site(&www);
    let server = os_testsite::TestSite::start_dir(&www, 0).await.unwrap();
    let home = root.path().join("home");
    let app = app(&home);
    let mut config = cfg();
    config.analysis.auto_at_end = true;
    config.speed.threads = 8;
    config.save_default(&app.config().paths.settings).unwrap();
    let start = Instant::now();
    let started = call(
        &app,
        "crawl.start",
        serde_json::to_value(request(server.url("/"))).unwrap(),
    )
    .await;
    let path = std::path::PathBuf::from(started["file"].as_str().unwrap());

    // Frontend model of the dashboard card: query key contains `issuesVersion`/`countsVersion`; a new key
    // aborts the running request (gcTime 0) and starts a new one. `loaded` = first answer for the newest key.
    let timeline = Arc::new(Mutex::new(Timeline::default()));
    let mut current: Option<(
        u64,
        os_model::CancelToken,
        tokio::task::JoinHandle<Option<f64>>,
    )> = None;
    let mut answered: Vec<(f64, u64, f64)> = Vec::new();
    let mut aborted = 0usize;
    let mut last_version = u64::MAX;
    let mut quiet_since: Option<Instant> = None;
    // Independent probe: one direct `sidebar.issues` after another from the crawl end on.
    let probe_log = Arc::new(Mutex::new(Vec::<(f64, f64, String)>::new()));
    let mut probe: Option<tokio::task::JoinHandle<()>> = None;
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    loop {
        if probe.is_none() && timeline.lock().unwrap().crawl_done.is_some() {
            let (worker, log, stop) = (app.clone(), probe_log.clone(), stop.clone());
            probe = Some(tokio::spawn(async move {
                while !stop.load(std::sync::atomic::Ordering::SeqCst) {
                    let at = start.elapsed().as_secs_f64();
                    let t0 = Instant::now();
                    let r = worker
                        .dispatch(CallCtx::for_window("main"), "sidebar.issues", json!({}))
                        .await;
                    let status = worker
                        .dispatch(
                            CallCtx::for_window("main"),
                            "sidebar.countStatus",
                            json!(null),
                        )
                        .await;
                    log.lock().unwrap().push((
                        at,
                        t0.elapsed().as_secs_f64(),
                        format!("{} {:?}", r.is_ok(), status.ok()),
                    ));
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
            }));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
        let now = start.elapsed().as_secs_f64();
        if let Some(tick) = app.assemble_tick() {
            let mut t = timeline.lock().unwrap();
            if tick.issues_version != last_version {
                t.versions.push((now, tick.issues_version));
            }
            if let Some(c) = &tick.crawl {
                if c.phase == os_app::dto::CrawlPhase::Completed && t.crawl_done.is_none() {
                    t.crawl_done = Some(now);
                }
            }
            if let Some(a) = &tick.analysis {
                if !a.running
                    && t.crawl_done.is_some()
                    && t.analysis_done.is_none()
                    && !a.steps.is_empty()
                {
                    t.analysis_done = Some(now);
                }
            }
            if tick.issues_version != last_version {
                last_version = tick.issues_version;
                quiet_since = Some(Instant::now());
                if let Some((_, cancel, task)) = current.take() {
                    if !task.is_finished() {
                        cancel.cancel();
                        aborted += 1;
                    }
                }
                let cancel = os_model::CancelToken::new();
                let ctx = CallCtx {
                    window: window(),
                    cancel: cancel.clone(),
                };
                let worker = app.clone();
                let begun = now;
                let task = tokio::spawn(async move {
                    let t0 = Instant::now();
                    worker
                        .dispatch(ctx, "sidebar.issues", json!({}))
                        .await
                        .ok()
                        .map(|_| begun + t0.elapsed().as_secs_f64())
                });
                current = Some((last_version, cancel, task));
            }
        }
        if current.as_ref().is_some_and(|c| c.2.is_finished()) {
            let (version, _, task) = current.take().unwrap();
            if let Some(done) = task.await.unwrap() {
                answered.push((now, version, done));
            }
        }
        let t = timeline.lock().unwrap();
        if t.analysis_done.is_some()
            && current.is_none()
            && quiet_since.is_some_and(|q| q.elapsed() > Duration::from_secs(5))
        {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(900),
            "Crawl/Analyse zu langsam: {t:?}"
        );
    }
    stop.store(true, std::sync::atomic::Ordering::SeqCst);
    if let Some(p) = probe {
        p.await.unwrap();
    }
    let probes = std::mem::take(&mut *probe_log.lock().unwrap());
    for (at, d, s) in probes.iter().filter(|p| p.1 > 0.1).take(10) {
        eprintln!("N26 Probe ab {at:.2} s: sidebar.issues {d:.3} s, countStatus {s}");
    }
    eprintln!(
        "N26 Proben gesamt {}, davon über 100 ms {}",
        probes.len(),
        probes.iter().filter(|p| p.1 > 0.1).count()
    );
    let t = std::mem::take(&mut *timeline.lock().unwrap());
    let crawl_done = t.crawl_done.unwrap();
    let analysis_done = t.analysis_done.unwrap();
    let last_change = t.versions.last().unwrap().0;
    let shown = answered
        .iter()
        .find(|(_, v, _)| *v == last_version)
        .map(|a| a.2)
        .unwrap();
    eprintln!("N26 crawl: fertig nach {crawl_done:.1} s, Analyse fertig nach {analysis_done:.1} s");
    eprintln!(
        "N26 Versionen: {} Wechsel, davon {} nach Crawl-Ende, letzte nach {last_change:.1} s; abgebrochene Abfragen {aborted}, beantwortete {}",
        t.versions.len(),
        t.versions.iter().filter(|v| v.0 >= crawl_done).count(),
        answered.len()
    );
    eprintln!("N26 Frontend-Modell: Top-Probleme erstmals mit Daten nach {:?} s (Crawl-Ende {crawl_done:.1} s)", answered.first().map(|a| a.2));
    eprintln!("N26 Frontend-Modell: Top-Probleme für die letzte Version nach {shown:.1} s ({:.1} s nach Analyse-Ende, {:.1} s nach letzter Änderung)", shown - analysis_done, shown - last_change);
    let gaps: Vec<f64> = t
        .versions
        .windows(2)
        .filter(|w| w[1].0 >= crawl_done)
        .map(|w| w[1].0 - w[0].0)
        .collect();
    eprintln!("N26 Abstände der Versionswechsel nach Crawl-Ende (s): {gaps:.1?}");

    for i in 0..3 {
        let (d, n) = issues(&app).await;
        eprintln!("N26 warm #{i}: sidebar.issues {d:?} ({n} Zeilen)");
    }
    let before = Instant::now();
    call(&app, "sidebar.overview", json!({})).await;
    eprintln!("N26 warm: sidebar.overview {:?}", before.elapsed());
    os_app::session::close(&app, &window(), false);
    drop(app);

    // Cold: new process, open the saved crawl (pieces from view_cache).
    let reopened = common::app(&home);
    let before = Instant::now();
    open(&reopened, &path).await;
    eprintln!("N26 kalt: crawl.open {:?}", before.elapsed());
    let (d, n) = issues(&reopened).await;
    eprintln!("N26 kalt (view_cache): sidebar.issues {d:?} ({n} Zeilen)");
    os_app::session::close(&reopened, &window(), false);
    drop(reopened);

    // Without stored pieces, same path: the in-process fact memory (keyed by path) still answers.
    let file = os_store::CrawlFile::open(&path, os_store::OpenMode::ReadWrite).unwrap();
    os_store::view_cache::clear(&file).unwrap();
    drop(file);
    let reopened = common::app(&home);
    open(&reopened, &path).await;
    let (d, n) = issues(&reopened).await;
    eprintln!(
        "N26 ohne view_cache, gleicher Pfad (Prozessspeicher): sidebar.issues {d:?} ({n} Zeilen)"
    );
    let (d, _) = issues(&reopened).await;
    eprintln!("N26 danach warm: sidebar.issues {d:?}");
    let urls = reopened
        .read_pool()
        .run(key(&reopened), |f, _| {
            Ok(f.conn().query_row(
                "SELECT COUNT(*), SUM(is_internal), SUM(is_internal AND content_class='HTML'), SUM(is_internal AND content_class<>'HTML') FROM urls",
                [],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?)),
            )?)
        })
        .await
        .unwrap();
    eprintln!("N26 Datei: URLs gesamt/intern/intern HTML/intern übrige = {urls:?}");
    os_app::session::close(&reopened, &window(), false);
    drop(reopened);

    // Really cold: the in-process fact memory is keyed by path, so count a copy without view_cache.
    let copy = root.path().join("copy.ospider");
    std::fs::copy(&path, &copy).unwrap();
    let file = os_store::CrawlFile::open(&copy, os_store::OpenMode::ReadWrite).unwrap();
    os_store::view_cache::clear(&file).unwrap();
    let cfg = config.clone();
    let tabs = os_analyze::tabs::configured_for_file(&cfg, &file);
    let cancel = os_model::CancelToken::new();
    let all = Instant::now();
    let mut slow = Vec::new();
    for tab in tabs.iter() {
        let before = Instant::now();
        let p = os_analyze::views::overview_progress(&file, &cfg, &cancel, 1).unwrap();
        slow.push((before.elapsed(), tab.id, p.done));
    }
    eprintln!(
        "N26 Vollzählung je Tab (Kopie ohne Cache) gesamt {:?}",
        all.elapsed()
    );
    slow.sort_by_key(|s| std::cmp::Reverse(s.0));
    for (d, tab, done) in slow.iter().take(8) {
        eprintln!("N26   {d:?} (Schritt {done}, Tab-Reihenfolge {tab})");
    }
    // Per filter of the slowest tabs: plain COUNT with the tab's plan, and the query plan.
    for (_, tab_id, _) in slow.iter().take(2) {
        let tab = tabs.iter().find(|t| t.id == *tab_id).unwrap();
        for f in &tab.filters {
            let q = os_analyze::views::ViewQuery::new(tab.id, f.id);
            let Ok(plan) = os_analyze::views::plan_in(&tabs, &q, &cfg) else {
                continue;
            };
            let sql = format!(
                "SELECT COUNT(*) FROM {} WHERE ({}) AND ({})",
                os_analyze::views::URLS_FROM,
                plan.population_sql,
                plan.filter_sql
            );
            let bind = |stmt: &mut rusqlite::Statement<'_>| {
                for (name, value) in &plan.params {
                    if let Ok(Some(i)) = stmt.parameter_index(name) {
                        let v: rusqlite::types::Value = match value {
                            os_model::Value::Int(n) => (*n).into(),
                            os_model::Value::Float(x) => (*x).into(),
                            os_model::Value::Text(t) => t.clone().into(),
                            os_model::Value::Bool(b) => i64::from(*b).into(),
                            os_model::Value::Null => rusqlite::types::Value::Null,
                        };
                        stmt.raw_bind_parameter(i, v).unwrap();
                    }
                }
            };
            let before = Instant::now();
            let mut stmt = file.conn().prepare(&sql).unwrap();
            bind(&mut stmt);
            let n: i64 = stmt.raw_query().next().unwrap().unwrap().get(0).unwrap();
            let d = before.elapsed();
            if d > Duration::from_millis(50) {
                eprintln!(
                    "N26     {tab_id} / {}: {d:?} (n={n})\n        SQL: {sql}",
                    f.id
                );
                let mut stmt = file
                    .conn()
                    .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
                    .unwrap();
                bind(&mut stmt);
                let mut rows = stmt.raw_query();
                while let Some(r) = rows.next().unwrap() {
                    eprintln!("        PLAN {}", r.get::<_, String>(3).unwrap());
                }
            } else {
                eprintln!("N26     {tab_id} / {}: {d:?} (n={n})", f.id);
            }
        }
    }
}
