//! Pause und Fortsetzen (CRAWL-126, CRAWL-066 „Limit nach Pause“, CRAWL-128): Zustand in der Datei, Ergebnis nach
//! Neustart gleich dem ununterbrochenen Crawl, Konfigurationsänderungen in der Pause.

#[path = "lifecycle_support.rs"]
mod support;

use std::sync::Arc;
use std::time::Duration;

use os_engine::lifecycle::{apply_paused_config, check_config_change};
use os_engine::testing::{sim_deps, MemorySink, SimFetcher, SimSite};
use os_engine::{resume_crawl, start_crawl, CrawlRequest, CrawlSource};
use os_model::{CrawlConfig, CrawlState, EndReason, RenderingMode};
use support::*;

fn secs(s: f64) -> Duration {
    Duration::from_secs_f64(s)
}

// ---------------------------------------------------------------------------------------------------
// Pause in simulierter Zeit (Regel 2.8)

// spec: CRAWL-126
#[tokio::test(start_paused = true)]
async fn after_pause_no_request_starts_and_the_running_ones_are_stored() {
    let sink = MemorySink::new();
    let fetcher =
        SimFetcher::new(SimSite::tree("site.test", 300).delay(Duration::from_millis(200)));
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = 5;
    let handle = start_crawl(
        fetcher.clone(),
        CrawlRequest {
            config: cfg,
            source: CrawlSource::Spider {
                start_url: "https://site.test/".to_string(),
            },
            file: dir.path().join("crawl.ospider"),
            project: "sim".to_string(),
            name: "sim".to_string(),
        },
        sim_deps(&sink),
    )
    .await
    .unwrap();
    tokio::time::sleep(secs(3.0)).await;
    let paused_at = secs(3.0);
    handle.pause();
    let summary = handle.wait().await.unwrap();
    assert_eq!(summary.state, CrawlState::Paused);

    let reqs = fetcher.page_requests();
    let late: Vec<_> = reqs
        .iter()
        .filter(|r| r.start > paused_at + secs(1.0))
        .collect();
    assert!(
        late.is_empty(),
        "nach 1 s darf keine Anfrage mehr starten: {late:?}"
    );
    // höchstens Threads-viele Anfragen liefen noch (und wurden zu Ende geführt)
    let running: Vec<_> = reqs.iter().filter(|r| r.end > paused_at).collect();
    assert!(running.len() <= 5, "{} laufende Anfragen", running.len());
    assert_eq!(fetcher.inflight(), 0, "alle laufenden Abrufe sind beendet");
    // jede beendete Anfrage steht gespeichert im Writer: Seiten + robots.txt-Zeile
    let stored = sink.pages().len();
    assert_eq!(
        stored,
        reqs.len() + 1,
        "alle {} Abrufe gespeichert",
        reqs.len()
    );
    // Meta: pausiert, Zeitpunkt der Pause vermerkt, kein Besitzer mehr
    let last_meta = |key: &str| {
        sink.batches()
            .into_iter()
            .flat_map(|b| b.meta.into_iter())
            .filter(|(k, _)| k == key)
            .map(|(_, v)| v)
            .next_back()
    };
    assert_eq!(last_meta("state").as_deref(), Some("Paused"));
    assert!(last_meta("paused_at").is_some_and(|v| !v.is_empty()));
    assert_eq!(last_meta("owner_pid").as_deref(), Some(""));
    assert_eq!(last_meta("ended_at"), None, "eine Pause ist kein Ende");
}

// ---------------------------------------------------------------------------------------------------
// Pause gegen die echte Datei

// spec: CRAWL-126
#[tokio::test]
async fn a_paused_file_holds_state_queue_pause_time_and_no_owner() {
    let dir = slow_site_dir(5);
    let site = start_site(dir.path()).await;
    let work = tempfile::tempdir().unwrap();
    let path = crawl_file_in(&work, "crawl.ospider");
    let handle = start(config_for(&site, 1), &path, deps()).await;
    until_crawled(&handle, 20).await;
    handle.pause();
    let summary = run_to_end(handle).await;
    assert_eq!(summary.state, CrawlState::Paused);
    assert_eq!(summary.end_reason, None);

    let file = open_ro(&path);
    let meta = file.meta().unwrap();
    assert_eq!(meta.state, CrawlState::Paused);
    assert_eq!(meta.ended_at, None);
    assert!(file
        .get_meta("paused_at")
        .unwrap()
        .is_some_and(|v| !v.is_empty()));
    assert_eq!(file.get_meta("owner_pid").unwrap().as_deref(), Some(""));
    assert_eq!(
        file.effective_state(&*deps().clock).unwrap(),
        CrawlState::Paused,
        "Pausiert, nicht Unterbrochen"
    );
    assert!(scalar(&file, "SELECT COUNT(*) FROM queue") > 0);
    assert_eq!(
        scalar(
            &file,
            "SELECT COUNT(*) FROM urls WHERE state = 'queued' AND id NOT IN (SELECT url_id FROM queue)"
        ),
        0,
        "jede eingeplante URL steht in der Warteschlange"
    );
}

// spec: CRAWL-126, CRAWL-125
#[tokio::test]
async fn resuming_after_a_restart_equals_the_uninterrupted_crawl_without_repeating_a_request() {
    // Referenz: ununterbrochen
    let ref_dir = slow_site_dir(2);
    let ref_site = start_site(ref_dir.path()).await;
    let ref_work = tempfile::tempdir().unwrap();
    let ref_path = crawl_file_in(&ref_work, "ref.ospider");
    let summary = run_to_end(start(config_for(&ref_site, 1), &ref_path, deps()).await).await;
    assert_eq!(summary.end_reason, Some(EndReason::QueueEmpty));

    // Pause nach 30 URLs, „Neustart“ (neue Engine, neuer Abrufer, neue Abhängigkeiten), Fortsetzen
    let dir = slow_site_dir(2);
    let site = start_site(dir.path()).await;
    let work = tempfile::tempdir().unwrap();
    let path = crawl_file_in(&work, "crawl.ospider");
    let handle = start(config_for(&site, 1), &path, deps()).await;
    until_crawled(&handle, 30).await;
    handle.pause();
    let paused = run_to_end(handle).await;
    assert_eq!(paused.state, CrawlState::Paused);
    assert!(paused.crawled >= 30 && paused.crawled < summary.crawled);

    let cfg = resume_config(&path, &site, |_| {});
    let resumed = run_to_end(resume(cfg, &path, deps()).await).await;
    assert_eq!(resumed.state, CrawlState::Completed);
    assert_eq!(resumed.end_reason, Some(EndReason::QueueEmpty));
    assert_eq!(resumed.crawl_id, paused.crawl_id, "dieselbe Crawl-Kennung");
    assert_eq!(resumed.crawled, summary.crawled, "Zähler läuft weiter");

    // Gesamtergebnis: identisch zum ununterbrochenen Crawl (URL-Menge, Status, Tiefe, Zähler)
    assert_eq!(snapshot(&path), snapshot(&ref_path));
    // keine URL – auch nicht robots.txt – doppelt im Request-Log
    assert_eq!(
        duplicate_requests(&site),
        vec![],
        "Pause und Fortsetzen wiederholen keinen Abruf"
    );
    // die Gesamtzahl der Abrufe entspricht dem ununterbrochenen Crawl
    assert_eq!(site.requests().len(), ref_site.requests().len());
    let file = open_ro(&path);
    assert_eq!(file.get_meta("owner_pid").unwrap().as_deref(), Some(""));
    assert_eq!(file.meta().unwrap().state, CrawlState::Completed);
    assert!(file.integrity_check().unwrap());
}

// spec: CRAWL-126
#[tokio::test]
async fn a_copied_file_resumes_on_another_machine_with_the_same_result() {
    let ref_dir = slow_site_dir(2);
    let ref_site = start_site(ref_dir.path()).await;
    let ref_work = tempfile::tempdir().unwrap();
    let ref_path = crawl_file_in(&ref_work, "ref.ospider");
    run_to_end(start(config_for(&ref_site, 1), &ref_path, deps()).await).await;

    let dir = slow_site_dir(2);
    let site = start_site(dir.path()).await;
    let work = tempfile::tempdir().unwrap();
    let path = crawl_file_in(&work, "crawl.ospider");
    let handle = start(config_for(&site, 1), &path, deps()).await;
    until_crawled(&handle, 40).await;
    handle.pause();
    run_to_end(handle).await;
    drop(site);

    // „anderer Rechner“: Datei kopieren, andere Testsite (anderer Port, andere Verzeichnisinstanz)
    let other_work = tempfile::tempdir().unwrap();
    let copy = crawl_file_in(&other_work, "kopie.ospider");
    open_ro(&path).save_as(&copy).unwrap();
    let other_dir = slow_site_dir(2);
    let other_site = start_site(other_dir.path()).await;
    let cfg = resume_config(&copy, &other_site, |_| {});
    let resumed = run_to_end(resume(cfg, &copy, deps()).await).await;
    assert_eq!(resumed.state, CrawlState::Completed);
    assert_eq!(snapshot(&copy), snapshot(&ref_path));
    assert_eq!(duplicate_requests(&other_site), vec![]);
}

// spec: CRAWL-126
#[tokio::test]
async fn an_exclude_added_during_the_pause_drops_matching_queue_entries_at_once() {
    let dir = slow_site_dir(2);
    let site = start_site(dir.path()).await;
    let work = tempfile::tempdir().unwrap();
    let path = crawl_file_in(&work, "crawl.ospider");
    let handle = start(config_for(&site, 1), &path, deps()).await;
    until_crawled(&handle, 12).await;
    handle.pause();
    run_to_end(handle).await;

    let before = scalar(&open_ro(&path), "SELECT COUNT(*) FROM queue");
    let excluded_queued = scalar(
        &open_ro(&path),
        "SELECT COUNT(*) FROM queue q JOIN urls u ON u.id = q.url_id WHERE u.url LIKE '%/page-1__.html' OR u.url GLOB '*/page-[1-9][0-9].html'",
    );
    assert!(
        before > 0 && excluded_queued > 0,
        "{before}/{excluded_queued}"
    );

    let mut cfg = resume_config(&path, &site, |c| {
        c.scope.exclude = vec![".*/page-[1-9][0-9]\\.html".to_string()];
    });
    let report = apply_paused_config(&path, &cfg, &deps()).unwrap();
    assert_eq!(report.queue_removed as i64, excluded_queued);
    assert_eq!(report.queue_remaining as i64, before - excluded_queued);
    let file = open_ro(&path);
    assert_eq!(
        scalar(&file, "SELECT COUNT(*) FROM queue") as u64,
        report.queue_remaining
    );
    assert_eq!(
        scalar(
            &file,
            "SELECT COUNT(*) FROM urls WHERE not_crawled_reason = 'excluded' AND state = 'discovered' AND in_tables = 0"
        ),
        excluded_queued
    );
    drop(file);

    // fortsetzen: die ausgeschlossenen Seiten werden nie mehr angefragt
    let seen_before = site.requests().len();
    cfg.network.resolve = vec![format!("*.test={}", site.addr())];
    let resumed = run_to_end(resume(cfg, &path, deps()).await).await;
    assert_eq!(resumed.state, CrawlState::Completed);
    let after: Vec<String> = site
        .requests()
        .into_iter()
        .skip(seen_before)
        .map(|r| r.path_and_query)
        .collect();
    assert!(!after.is_empty());
    for p in after.iter().filter(|p| p.starts_with("/page-")) {
        let n: u32 = p
            .trim_start_matches("/page-")
            .trim_end_matches(".html")
            .parse()
            .unwrap();
        assert!(n < 10, "{p} hätte ausgeschlossen sein müssen");
    }
    let file = open_ro(&path);
    assert_eq!(scalar(&file, "SELECT COUNT(*) FROM queue"), 0);
    assert!(
        scalar(
            &file,
            "SELECT COUNT(*) FROM urls WHERE url GLOB '*/page-[1-9][0-9].html' AND state = 'discovered' AND not_crawled_reason = 'excluded'"
        ) >= excluded_queued,
        "die verworfenen Einträge bleiben als ausgeschlossen vermerkt"
    );
    assert_eq!(duplicate_requests(&site), vec![]);
}

// spec: CRAWL-066, CRAWL-126
#[tokio::test]
async fn the_limit_can_be_raised_after_a_pause_and_lowered_below_the_crawled_count() {
    let dir = slow_site_dir(1);
    let site = start_site(dir.path()).await;
    let work = tempfile::tempdir().unwrap();
    let path = crawl_file_in(&work, "crawl.ospider");

    // Limit 10: genau 10 Abrufe (ohne robots.txt), danach Ende
    let mut cfg = config_for(&site, 1);
    cfg.limits.max_urls = 10;
    let first = run_to_end(start(cfg, &path, deps()).await).await;
    assert_eq!(first.end_reason, Some(EndReason::LimitReached));
    assert_eq!(first.crawled, 10);
    let non_robots = |site: &os_testsite::TestSite| {
        site.requests()
            .iter()
            .filter(|r| r.path() != "/robots.txt")
            .count()
    };
    assert_eq!(non_robots(&site), 10);

    // Limit auf 25 erhöhen und fortsetzen
    let cfg = resume_config(&path, &site, |c| c.limits.max_urls = 25);
    let second = run_to_end(resume(cfg, &path, deps()).await).await;
    assert_eq!(second.state, CrawlState::Completed);
    assert_eq!(second.end_reason, Some(EndReason::LimitReached));
    assert_eq!(second.crawled, 25);
    assert_eq!(
        non_robots(&site),
        25,
        "genau 15 neue Abrufe, keine Wiederholung"
    );
    assert_eq!(duplicate_requests(&site), vec![]);

    // Limit unter die gecrawlte Anzahl senken: Fortsetzen endet sofort mit „Limit erreicht“, ohne Abruf
    let cfg = resume_config(&path, &site, |c| c.limits.max_urls = 5);
    let third = run_to_end(resume(cfg, &path, deps()).await).await;
    assert_eq!(third.end_reason, Some(EndReason::LimitReached));
    assert_eq!(third.crawled, 25);
    assert_eq!(non_robots(&site), 25, "kein weiterer Abruf");

    // ohne Limit bis zum Ende
    let cfg = resume_config(&path, &site, |c| c.limits.max_urls = 5_000_000);
    let done = run_to_end(resume(cfg, &path, deps()).await).await;
    assert_eq!(done.end_reason, Some(EndReason::QueueEmpty));
    assert_eq!(duplicate_requests(&site), vec![]);
}

// spec: CRAWL-126
#[tokio::test]
async fn fewer_threads_after_the_pause_never_exceed_the_new_number_of_parallel_requests() {
    let dir = slow_site_dir(10);
    let site = start_site(dir.path()).await;
    let work = tempfile::tempdir().unwrap();
    let path = crawl_file_in(&work, "crawl.ospider");
    let handle = start(config_for(&site, 5), &path, deps()).await;
    until_crawled(&handle, 25).await;
    handle.pause();
    run_to_end(handle).await;

    site.reset_request_log();
    let cfg = resume_config(&path, &site, |c| c.speed.threads = 1);
    let resumed = run_to_end(resume(cfg, &path, deps()).await).await;
    assert_eq!(resumed.state, CrawlState::Completed);
    assert!(
        site.max_concurrency() <= 1,
        "der Server sah {} gleichzeitige Anfragen",
        site.max_concurrency()
    );
}

// spec: CRAWL-126
#[test]
fn mode_scope_and_rendering_cannot_change_while_paused() {
    let base = CrawlConfig::default();
    let mut scope = base.clone();
    scope.scope.subfolder_only = true;
    let mut sub = base.clone();
    sub.scope.all_subdomains = true;
    let mut rendering = base.clone();
    rendering.rendering.mode = RenderingMode::JavaScript;
    let mut mode = base.clone();
    mode.mode = os_model::CrawlMode::List;
    for (label, changed) in [
        ("Crawl-Bereich", scope),
        ("Crawl-Bereich", sub),
        ("Rendering-Modus", rendering),
        ("Modus", mode),
    ] {
        let err = check_config_change(&base, &changed)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains(label) && err.contains("neuer Crawl nötig"),
            "{err}"
        );
    }
    // erlaubt: Limits, Regeln, Threads, Erfassungsschalter, User-Agent
    let mut ok = base.clone();
    ok.limits.max_urls = 7;
    ok.scope.exclude = vec![".*".to_string()];
    ok.speed.threads = 2;
    ok.extraction.hash = false;
    ok.user_agent.http = Some("Test/1.0".to_string());
    check_config_change(&base, &ok).unwrap();
}

// spec: CRAWL-126
#[tokio::test]
async fn forcing_a_forbidden_change_through_resume_is_refused_and_leaves_the_file_unchanged() {
    let dir = slow_site_dir(2);
    let site = start_site(dir.path()).await;
    let work = tempfile::tempdir().unwrap();
    let path = crawl_file_in(&work, "crawl.ospider");
    let handle = start(config_for(&site, 1), &path, deps()).await;
    until_crawled(&handle, 10).await;
    handle.pause();
    run_to_end(handle).await;
    let before = open_ro(&path).config().unwrap();

    let cfg = resume_config(&path, &site, |c| {
        c.rendering.mode = RenderingMode::JavaScript
    });
    let fetcher = http_fetcher(&cfg, &deps());
    let err = match resume_crawl(fetcher, path.clone(), Some(cfg), deps()).await {
        Ok(_) => panic!("Rendering-Wechsel hätte abgewiesen werden müssen"),
        Err(e) => e.to_string(),
    };
    assert!(
        err.contains("Rendering-Modus") && err.contains("neuer Crawl nötig"),
        "{err}"
    );
    assert_eq!(open_ro(&path).config().unwrap(), before);
    assert_eq!(open_ro(&path).meta().unwrap().state, CrawlState::Paused);
}

// spec: CRAWL-126, CRAWL-128
#[tokio::test]
async fn only_paused_interrupted_and_unfinished_crawls_can_be_resumed() {
    // Abgeschlossen mit leerer Warteschlange: nichts fortzusetzen
    let dir = slow_site_dir(0);
    let site = start_site(dir.path()).await;
    let work = tempfile::tempdir().unwrap();
    let path = crawl_file_in(&work, "crawl.ospider");
    run_to_end(start(config_for(&site, 3), &path, deps()).await).await;
    let cfg = resume_config(&path, &site, |_| {});
    let fetcher = http_fetcher(&cfg, &deps());
    let err = match resume_crawl(
        Arc::clone(&fetcher),
        path.clone(),
        Some(cfg.clone()),
        deps(),
    )
    .await
    {
        Ok(_) => panic!("abgeschlossen mit leerer Warteschlange"),
        Err(e) => e.to_string(),
    };
    assert!(
        err.contains("abgeschlossen") && err.contains("Warteschlange leer"),
        "{err}"
    );

    // Fehlgeschlagen
    {
        let file = os_store::CrawlFile::open(&path, os_store::OpenMode::ReadWrite).unwrap();
        file.set_meta("state", "Failed").unwrap();
    }
    let err = match resume_crawl(
        Arc::clone(&fetcher),
        path.clone(),
        Some(cfg.clone()),
        deps(),
    )
    .await
    {
        Ok(_) => panic!("fehlgeschlagen"),
        Err(e) => e.to_string(),
    };
    assert!(err.contains("fehlgeschlagen"), "{err}");

    // Läuft noch (Heartbeat frisch): abgewiesen mit Hinweis auf den Besitzer
    {
        let file = os_store::CrawlFile::open(&path, os_store::OpenMode::ReadWrite).unwrap();
        file.set_meta("state", "Running").unwrap();
        file.set_meta("owner_pid", "4242").unwrap();
        file.set_meta("heartbeat_at", &deps().clock.now().to_db())
            .unwrap();
    }
    let err = match resume_crawl(fetcher, path.clone(), Some(cfg), deps()).await {
        Ok(_) => panic!("läuft noch"),
        Err(e) => e.to_string(),
    };
    assert!(err.contains("läuft noch") && err.contains("4242"), "{err}");
}

// spec: CRAWL-126
#[tokio::test]
async fn the_time_between_pause_and_resume_counts_as_paused_time_and_the_counters_continue() {
    let dir = slow_site_dir(2);
    let site = start_site(dir.path()).await;
    let work = tempfile::tempdir().unwrap();
    let path = crawl_file_in(&work, "crawl.ospider");
    let handle = start(config_for(&site, 1), &path, deps()).await;
    until_crawled(&handle, 15).await;
    handle.pause();
    let paused = run_to_end(handle).await;
    let file = open_ro(&path);
    let paused_at =
        os_model::Timestamp::from_db(&file.get_meta("paused_at").unwrap().unwrap()).unwrap();
    drop(file);

    // 45 s später fortsetzen: die Wartezeit ist Pausenzeit, der Zähler läuft weiter
    let later = deps_from(paused_at.plus(Duration::from_secs(45)));
    let cfg = resume_config(&path, &site, |_| {});
    let handle = resume(cfg, &path, later).await;
    let p = handle.progress();
    assert_eq!(p.crawled, paused.crawled, "gecrawlt läuft weiter");
    assert!(p.paused_s >= 45, "paused_s = {}", p.paused_s);
    assert!(p.elapsed_s >= 45);
    handle.pause();
    run_to_end(handle).await;
    let file = open_ro(&path);
    let total: i64 = file
        .get_meta("paused_total_ms")
        .unwrap()
        .unwrap()
        .parse()
        .unwrap();
    assert!(total >= 45_000, "{total}");

    // noch einmal pausieren und 20 s später fortsetzen: die Pausenzeiten addieren sich
    let paused_again =
        os_model::Timestamp::from_db(&file.get_meta("paused_at").unwrap().unwrap()).unwrap();
    drop(file);
    let cfg = resume_config(&path, &site, |_| {});
    let handle = resume(
        cfg,
        &path,
        deps_from(paused_again.plus(Duration::from_secs(20))),
    )
    .await;
    assert!(
        handle.progress().paused_s >= 65,
        "{}",
        handle.progress().paused_s
    );
    run_to_end(handle).await;
    let file = open_ro(&path);
    assert!(
        file.get_meta("paused_total_ms")
            .unwrap()
            .unwrap()
            .parse::<i64>()
            .unwrap()
            >= 65_000
    );
    assert_eq!(
        file.get_meta("paused_at").unwrap().as_deref(),
        Some(""),
        "nach dem Fortsetzen ist nichts mehr pausiert"
    );
}

// spec: CRAWL-126
#[tokio::test]
#[ignore = "Benchmark (P28): Pausieren mit 1 Mio. Warteschlangeneinträgen in unter 3 s (Wanduhr, Referenzmaschine)"]
async fn pausing_with_a_million_queue_entries_takes_under_three_seconds() {
    let work = tempfile::tempdir().unwrap();
    let path = crawl_file_in(&work, "gross.ospider");
    // Crawl-Datei mit 1 000 000 eingeplanten URLs (Zustand „Pausiert“), direkt in SQL erzeugt
    {
        let site_cfg = CrawlConfig::default();
        let init = os_store::CrawlInit {
            crawl_id: deps().ids.new_crawl_id(deps().clock.now()),
            mode: os_model::CrawlMode::Spider,
            start_url: Some("http://site.test/".to_string()),
            list_source: None,
            project: "bench".to_string(),
            name: "bench".to_string(),
            config: site_cfg,
            created_at: deps().clock.now(),
        };
        let file =
            os_store::CrawlFile::create(&path, &init, os_store::StorageProfile::Balanced).unwrap();
        file.conn()
            .execute_batch(
                "BEGIN;
                 WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c WHERE x < 1000000)
                 INSERT INTO urls (id, url, address, origin, is_internal, in_tables, state, discovered_via, crawl_depth, folder_depth, url_length)
                   SELECT x, 'http://site.test/q/' || x, 'http://site.test/q/' || x, 'http://site.test', 1, 1, 'queued', 'link', 1, 1, 25 FROM c;
                 WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c WHERE x < 1000000)
                 INSERT INTO queue (url_id, seq, depth, priority) SELECT x, x, 1, 0 FROM c;
                 UPDATE crawl_meta SET value = 'Paused' WHERE key = 'state';
                 COMMIT;",
            )
            .unwrap();
    }
    let sink = os_engine::testing::MemorySink::new();
    let mut d = deps();
    d.sink_factory = Some(sink.factory());
    let fetcher = os_engine::testing::SimFetcher::new(
        os_engine::testing::SimSite::new().delay(Duration::from_millis(50)),
    );
    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = 5;
    let started = std::time::Instant::now();
    let handle = resume_crawl(fetcher, path.clone(), Some(cfg), d)
        .await
        .unwrap();
    let restore = started.elapsed();
    tokio::time::sleep(Duration::from_millis(300)).await;
    let before_pause = std::time::Instant::now();
    handle.pause();
    let summary = handle.wait().await.unwrap();
    let pause = before_pause.elapsed();
    assert_eq!(summary.state, CrawlState::Paused);
    eprintln!("Fortsetzen (Aufbau aus der Datei): {restore:?}, Pausieren: {pause:?}");
    assert!(
        pause < Duration::from_secs(3),
        "Pausieren dauerte {pause:?}"
    );
}
