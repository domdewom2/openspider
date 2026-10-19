//! Zeit-, Raten- und Ablaufkriterien in simulierter Zeit (Regel 2.8): `#[tokio::test(start_paused = true)]`
//! mit `SimFetcher` (Website im Speicher, Verzögerung über `tokio::time::sleep`) und `MemorySink`.
//! Kein Test wartet echte Zeit ab.

use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use os_engine::testing::{sim_deps, MemorySink, SimFetcher, SimSite};
use os_engine::{start_crawl, CrawlHandle, CrawlRequest, CrawlSource, CrawlSummary};
use os_model::{CrawlConfig, CrawlState, EndReason, QueueOp, UrlId};

fn request(cfg: CrawlConfig, start: &str, dir: &tempfile::TempDir) -> CrawlRequest {
    CrawlRequest {
        config: cfg,
        source: CrawlSource::Spider {
            start_url: start.to_string(),
        },
        file: dir.path().join("crawl.ospider"),
        project: "sim".to_string(),
        name: "sim".to_string(),
    }
}

/// Simulierte Zeitkriterien dürfen nicht vom freien RAM des Testrechners abhängen (M2-E02 drosselt sonst Threads).
fn fixed_memory_probe() {
    os_engine::memory::set_probe(Some(Arc::new(|| os_engine::memory::Sample {
        physical_bytes: 16 << 30,
        available_bytes: 8 << 30,
        ..Default::default()
    })));
}

async fn start(
    site: SimSite,
    cfg: CrawlConfig,
    sink: &MemorySink,
) -> (Arc<SimFetcher>, CrawlHandle, tempfile::TempDir) {
    fixed_memory_probe();
    let fetcher = SimFetcher::new(site);
    let dir = tempfile::tempdir().unwrap();
    let handle = start_crawl(
        fetcher.clone(),
        request(cfg, "https://site.test/", &dir),
        sim_deps(sink),
    )
    .await
    .expect("Crawl startet");
    (fetcher, handle, dir)
}

fn secs(s: f64) -> Duration {
    Duration::from_secs_f64(s)
}

fn threads(n: u32) -> CrawlConfig {
    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = n;
    cfg
}

// ---------------------------------------------------------------------------------------------------
// CRAWL-112

// spec: CRAWL-112
#[tokio::test(start_paused = true)]
async fn five_threads_keep_at_least_four_requests_busy_on_average_and_finish_within_the_bound() {
    // 100 Seiten à 200 ms, 5 Threads: Dauer ≤ 100 × 0,2 / 5 × 1,3 = 5,2 s
    let sink = MemorySink::new();
    let site = SimSite::tree("site.test", 100).delay(Duration::from_millis(200));
    let (fetcher, handle, _dir) = start(site, threads(5), &sink).await;
    let summary = handle.wait().await.unwrap();
    assert_eq!(summary.state, CrawlState::Completed);

    let reqs = fetcher.page_requests();
    // Abrufe außer robots.txt: 100 Seiten + 10 Bilder + 3 CSS + 3 JS (Aufbau von `SimSite::tree`), jede URL einmal.
    assert_eq!(reqs.len(), 100 + 10 + 3 + 3, "{} Abrufe", reqs.len());
    let elapsed = reqs.iter().map(|r| r.end).max().unwrap();
    assert!(elapsed <= secs(5.2), "Dauer {elapsed:?} (Grenze 5,2 s)");
    let busy: Duration = reqs.iter().map(|r| r.end - r.start).sum();
    let mean = busy.as_secs_f64() / elapsed.as_secs_f64();
    assert!(mean >= 4.0, "mittlere Auslastung {mean:.2} (mindestens 4)");
    assert!(fetcher.max_concurrency() <= 5, "höchstens 5 gleichzeitig");
    assert_eq!(fetcher.max_concurrency(), 5);
}

// spec: CRAWL-112
#[tokio::test(start_paused = true)]
async fn one_thread_never_runs_two_requests_at_once() {
    let sink = MemorySink::new();
    let site = SimSite::tree("site.test", 40).delay(Duration::from_millis(50));
    let (fetcher, handle, _dir) = start(site, threads(1), &sink).await;
    handle.wait().await.unwrap();
    assert_eq!(fetcher.max_concurrency(), 1);
}

// spec: CRAWL-112
#[tokio::test(start_paused = true)]
async fn thread_changes_take_effect_within_two_seconds_without_a_restart() {
    let sink = MemorySink::new();
    let site = SimSite::tree("site.test", 300).delay(Duration::from_millis(200));
    let (fetcher, handle, _dir) = start(site, threads(1), &sink).await;

    tokio::time::sleep(secs(1.0)).await;
    let before = fetcher
        .requests()
        .iter()
        .map(|r| r.concurrent)
        .max()
        .unwrap();
    assert_eq!(before, 1, "ein Thread");

    // hoch auf 5
    handle.set_threads(5);
    tokio::time::sleep(secs(2.0)).await;
    let raised: Vec<_> = fetcher
        .requests()
        .into_iter()
        .filter(|r| r.start >= secs(1.0))
        .map(|r| r.concurrent)
        .collect();
    assert_eq!(
        raised.iter().copied().max(),
        Some(5),
        "binnen 2 s sind 5 Threads aktiv"
    );

    // herunter auf 2: laufende Abrufe dürfen enden (≤ 0,2 s), danach höchstens 2 gleichzeitig
    let t_change = secs(3.0);
    handle.set_threads(2);
    tokio::time::sleep(secs(2.0)).await;
    let lowered: Vec<_> = fetcher
        .requests()
        .into_iter()
        .filter(|r| r.start >= t_change + secs(0.3))
        .map(|r| r.concurrent)
        .collect();
    assert!(!lowered.is_empty());
    assert!(lowered.iter().all(|c| *c <= 2), "{lowered:?}");

    // Werte außerhalb des Bereichs werden begrenzt
    handle.set_threads(0);
    handle.set_threads(10_000);
    handle.stop();
    handle.wait().await.unwrap();
}

// ---------------------------------------------------------------------------------------------------
// CRAWL-005

// spec: CRAWL-005
#[tokio::test(start_paused = true)]
async fn with_several_threads_no_deeper_url_starts_while_a_known_shallower_one_waits() {
    // Wurzel → 20 Seiten (Tiefe 1), jede → ein Kind (Tiefe 2). Alle Tiefe-1-Abrufe müssen vor dem ersten
    // Tiefe-2-Abruf gestartet sein, auch mit 4 Threads.
    let mut site = SimSite::new().delay(Duration::from_millis(50));
    let mut root = String::from("<html><body>");
    for i in 1..=20 {
        root.push_str(&format!("<a href=\"/a{i}\">a{i}</a>"));
        site = site.page(
            &format!("https://site.test/a{i}"),
            &format!("<html><body><a href=\"/b{i}\">b</a></body></html>"),
        );
        site = site.page(
            &format!("https://site.test/b{i}"),
            "<html><body>Ende</body></html>",
        );
    }
    root.push_str("</body></html>");
    site = site.page("https://site.test/", &root);

    let sink = MemorySink::new();
    let (fetcher, handle, _dir) = start(site, threads(4), &sink).await;
    handle.wait().await.unwrap();
    let order: Vec<String> = fetcher.page_requests().into_iter().map(|r| r.url).collect();
    let last_a = order.iter().rposition(|u| u.contains("/a")).unwrap();
    let first_b = order.iter().position(|u| u.contains("/b")).unwrap();
    assert!(
        last_a < first_b,
        "ein Tiefe-2-Abruf startete vor dem letzten Tiefe-1-Abruf: {order:?}"
    );
    assert_eq!(order.len(), 41);
}

// spec: CRAWL-003
#[tokio::test(start_paused = true)]
async fn fifty_pages_linking_to_one_url_cause_exactly_one_request_for_it_with_twenty_threads() {
    let mut site = SimSite::new().delay(Duration::from_millis(30));
    let mut root = String::new();
    for i in 1..=50 {
        root.push_str(&format!("<a href=\"/p{i}\">p{i}</a>"));
        site = site.page(
            &format!("https://site.test/p{i}"),
            "<html><body><a href=\"/geteilt\">geteilt</a></body></html>",
        );
    }
    site = site.page(
        "https://site.test/",
        &format!("<html><body>{root}</body></html>"),
    );
    site = site.page(
        "https://site.test/geteilt",
        "<html><body>geteilt</body></html>",
    );
    let sink = MemorySink::new();
    let (fetcher, handle, _dir) = start(site, threads(20), &sink).await;
    handle.wait().await.unwrap();
    assert_eq!(fetcher.count("https://site.test/geteilt"), 1);
    assert_eq!(fetcher.page_requests().len(), 52);
    // jede URL genau eine Zeile
    let urls: Vec<String> = sink.new_urls().into_iter().map(|u| u.url).collect();
    let unique: HashSet<_> = urls.iter().collect();
    assert_eq!(urls.len(), unique.len());
}

// ---------------------------------------------------------------------------------------------------
// CRAWL-130

// spec: CRAWL-130
#[tokio::test(start_paused = true)]
async fn rate_after_twenty_seconds_is_between_seven_and_ten_per_second_with_one_thread() {
    let sink = MemorySink::new();
    let site = SimSite::tree("site.test", 700).delay(Duration::from_millis(100));
    let (_fetcher, handle, _dir) = start(site, threads(1), &sink).await;
    tokio::time::sleep(secs(20.15)).await;
    let p = handle.progress();
    assert_eq!(p.state, CrawlState::Running);
    assert!((7.0..=10.0).contains(&p.rate), "Rate {}", p.rate);
    assert_eq!(p.eta_s, None, "vor 30 s keine Restzeit");
    assert!(p.avg_rate > 0.0 && p.paused_s == 0);
    assert!(p.discovered > p.crawled);
    assert_eq!(p.avg_response_time, Some(0.1));
    assert_eq!(p.active_threads, 1);
    handle.stop();
    handle.wait().await.unwrap();
}

// spec: CRAWL-130
#[tokio::test(start_paused = true)]
async fn eta_appears_after_thirty_seconds_and_one_hundred_urls() {
    let sink = MemorySink::new();
    let site = SimSite::tree("site.test", 700).delay(Duration::from_millis(100));
    let (_fetcher, handle, _dir) = start(site, threads(1), &sink).await;
    tokio::time::sleep(secs(29.0)).await;
    assert_eq!(handle.progress().eta_s, None, "29 s");
    tokio::time::sleep(secs(2.5)).await;
    let p = handle.progress();
    assert!(p.crawled >= 100);
    let eta = p.eta_s.expect("nach 31,5 s und über 100 URLs");
    // Warteschlange / Rate
    let expected = p.queued as f64 / p.rate;
    assert!(
        (eta as f64 - expected).abs() <= 2.0,
        "eta {eta}, erwartet {expected:.1}"
    );
    assert!(p.percent > 0 && p.percent < 100);
    handle.stop();
    handle.wait().await.unwrap();

    // wenige URLs (400 ms je Abruf): auch nach 31 s noch keine Restzeit
    let sink = MemorySink::new();
    let site = SimSite::tree("site.test", 700).delay(Duration::from_millis(400));
    let (_f, handle, _d) = start(site, threads(1), &sink).await;
    tokio::time::sleep(secs(31.0)).await;
    let p = handle.progress();
    assert!(p.crawled < 100, "{}", p.crawled);
    assert_eq!(p.eta_s, None);
    handle.stop();
    handle.wait().await.unwrap();
}

// spec: CRAWL-130
#[tokio::test(start_paused = true)]
async fn state_runs_and_ends_completed_with_percent_one_hundred() {
    let sink = MemorySink::new();
    let site = SimSite::tree("site.test", 30).delay(Duration::from_millis(20));
    let (_fetcher, handle, _dir) = start(site, threads(3), &sink).await;
    let mut watch = handle.watch();
    assert_eq!(watch.borrow().state, CrawlState::Running);
    let summary: CrawlSummary = handle.wait().await.unwrap();
    watch.changed().await.ok();
    let last = watch.borrow().clone();
    assert_eq!(last.state, CrawlState::Completed);
    assert_eq!(last.end_reason, Some(EndReason::QueueEmpty));
    assert_eq!(last.percent, 100);
    assert_eq!(last.crawled, summary.crawled);
    assert_eq!(last.queued, 0);
}

// spec: CRAWL-130
#[tokio::test(start_paused = true)]
async fn a_set_url_limit_raises_the_percentage_beyond_the_queue_ratio() {
    let sink = MemorySink::new();
    let site = SimSite::tree("site.test", 300).delay(Duration::from_millis(100));
    let mut cfg = threads(1);
    cfg.limits.max_urls = 100;
    let (_fetcher, handle, _dir) = start(site, cfg, &sink).await;
    tokio::time::sleep(secs(5.05)).await;
    let p = handle.progress();
    // gecrawlt / Limit = 50 / 100 = 50 %, gecrawlt / (gecrawlt + Warteschlange) wäre viel kleiner
    assert!((48..=52).contains(&p.crawled), "{}", p.crawled);
    assert_eq!(p.percent as u64, p.crawled * 100 / 100);
    let summary = handle.wait().await.unwrap();
    assert_eq!(summary.end_reason, Some(EndReason::LimitReached));
    assert_eq!(summary.crawled, 100);
}

// ---------------------------------------------------------------------------------------------------
// Stopp, Pause, Abschluss-Hook

fn queue_after(sink: &MemorySink) -> HashSet<UrlId> {
    let mut queue = HashSet::new();
    for b in sink.batches() {
        for op in b.queue {
            match op {
                QueueOp::Push { id, .. } => {
                    queue.insert(id);
                }
                QueueOp::Pop { id } => {
                    queue.remove(&id);
                }
            }
        }
    }
    queue
}

fn meta_values(sink: &MemorySink, key: &str) -> Vec<String> {
    sink.batches()
        .into_iter()
        .flat_map(|b| b.meta.into_iter())
        .filter(|(k, _)| k == key)
        .map(|(_, v)| v)
        .collect()
}

// spec: CRAWL-026
#[tokio::test(start_paused = true)]
async fn stop_ends_the_crawl_keeps_the_queue_and_calls_the_hook() {
    let sink = MemorySink::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut deps = sim_deps(&sink);
    let counter = calls.clone();
    deps.on_finished = Some(Arc::new(move |s: &CrawlSummary| {
        assert_eq!(s.end_reason, Some(EndReason::Stopped));
        counter.fetch_add(1, Ordering::SeqCst);
    }));
    let fetcher =
        SimFetcher::new(SimSite::tree("site.test", 300).delay(Duration::from_millis(100)));
    let dir = tempfile::tempdir().unwrap();
    let handle = start_crawl(
        fetcher.clone(),
        request(threads(2), "https://site.test/", &dir),
        deps,
    )
    .await
    .unwrap();
    tokio::time::sleep(secs(3.0)).await;
    handle.stop();
    let summary = handle.wait().await.unwrap();
    assert_eq!(summary.state, CrawlState::Completed);
    assert_eq!(summary.end_reason, Some(EndReason::Stopped));
    assert_eq!(calls.load(Ordering::SeqCst), 1, "Hook einmal");
    assert_eq!(
        meta_values(&sink, "state").last().map(String::as_str),
        Some("Completed")
    );
    assert_eq!(
        meta_values(&sink, "end_reason").last().map(String::as_str),
        Some("stopped")
    );
    // abgeschlossene Seiten sind aus der Warteschlange, der Rest bleibt für das Fortsetzen
    let queue = queue_after(&sink);
    assert!(!queue.is_empty());
    let done: HashSet<_> = sink.pages().into_iter().map(|p| p.url_id).collect();
    assert!(queue.is_disjoint(&done));
    assert!(summary.crawled < 100);
}

// spec: CRAWL-026
#[tokio::test(start_paused = true)]
async fn pause_ends_the_run_as_paused_without_the_hook() {
    let sink = MemorySink::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut deps = sim_deps(&sink);
    let counter = calls.clone();
    deps.on_finished = Some(Arc::new(move |_s: &CrawlSummary| {
        counter.fetch_add(1, Ordering::SeqCst);
    }));
    let fetcher =
        SimFetcher::new(SimSite::tree("site.test", 300).delay(Duration::from_millis(100)));
    let dir = tempfile::tempdir().unwrap();
    let handle = start_crawl(
        fetcher.clone(),
        request(threads(2), "https://site.test/", &dir),
        deps,
    )
    .await
    .unwrap();
    tokio::time::sleep(secs(2.0)).await;
    let before = fetcher.requests().len();
    handle.pause();
    let summary = handle.wait().await.unwrap();
    assert_eq!(summary.state, CrawlState::Paused);
    assert_eq!(summary.end_reason, None);
    assert_eq!(calls.load(Ordering::SeqCst), 0, "Pause ruft den Hook nicht");
    assert_eq!(
        meta_values(&sink, "state").last().map(String::as_str),
        Some("Paused")
    );
    // keine neuen Abrufe nach der Pause, nur laufende werden gespeichert
    assert!(fetcher.requests().len() <= before + 2);
    assert!(meta_values(&sink, "ended_at").is_empty());
}

// spec: CRAWL-026
#[tokio::test(start_paused = true)]
async fn the_hook_runs_after_the_final_state_is_written_and_before_wait_returns() {
    let sink = MemorySink::new();
    let seen_state = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let mut deps = sim_deps(&sink);
    let (s2, k2) = (sink.clone(), seen_state.clone());
    deps.on_finished = Some(Arc::new(move |s: &CrawlSummary| {
        let state = s2
            .batches()
            .into_iter()
            .flat_map(|b| b.meta.into_iter())
            .filter(|(k, _)| k == "state")
            .map(|(_, v)| v)
            .next_back()
            .unwrap_or_default();
        k2.lock()
            .unwrap()
            .push(format!("{state}:{:?}", s.end_reason));
    }));
    let fetcher = SimFetcher::new(SimSite::tree("site.test", 12));
    let dir = tempfile::tempdir().unwrap();
    let handle = start_crawl(
        fetcher,
        request(threads(2), "https://site.test/", &dir),
        deps,
    )
    .await
    .unwrap();
    let summary = handle.wait().await.unwrap();
    assert_eq!(summary.end_reason, Some(EndReason::QueueEmpty));
    assert_eq!(
        *seen_state.lock().unwrap(),
        vec!["Completed:Some(QueueEmpty)".to_string()]
    );
}

// spec: CRAWL-026
#[tokio::test(start_paused = true)]
async fn heartbeat_is_written_every_two_seconds() {
    let sink = MemorySink::new();
    let site = SimSite::tree("site.test", 100).delay(Duration::from_secs(1));
    let (_fetcher, handle, _dir) = start(site, threads(1), &sink).await;
    tokio::time::sleep(secs(7.0)).await;
    handle.stop();
    handle.wait().await.unwrap();
    let beats = meta_values(&sink, "heartbeat_at");
    // Start + 3 Schläge (2, 4, 6 s) + Ende
    assert!(beats.len() >= 4, "{beats:?}");
}

// spec: CRAWL-130
#[tokio::test(start_paused = true)]
async fn start_writes_owner_state_and_the_start_url_first() {
    let sink = MemorySink::new();
    let (_fetcher, handle, _dir) = start(SimSite::tree("site.test", 3), threads(1), &sink).await;
    handle.wait().await.unwrap();
    let batches = sink.batches();
    let first = &batches[0];
    let keys: Vec<_> = first.meta.iter().map(|(k, _)| k.as_str()).collect();
    for k in [
        "state",
        "started_at",
        "owner_kind",
        "owner_pid",
        "heartbeat_at",
    ] {
        assert!(keys.contains(&k), "{k} fehlt in {keys:?}");
    }
    assert_eq!(first.new_urls[0].url, "https://site.test/");
    assert_eq!(first.new_urls[0].crawl_depth, Some(0));
}

// spec: CRAWL-127, CRAWL-130
// Infrastruktur (M2-F05a) für CRAWL-127/130; Fachprüfung M2-E02.
#[tokio::test(start_paused = true)]
async fn added_urls_reach_admission_and_progress_is_persisted_during_a_crawl() {
    let sink = MemorySink::new();
    let site = SimSite::new()
        .page("https://site.test/", "<title>fixture</title>")
        .delay(Duration::from_millis(2500));
    let (_, handle, _dir) = start(site, threads(1), &sink).await;
    assert!(matches!(
        handle
            .add_urls(vec!["https://site.test/added".into()])
            .await,
        Ok(os_engine::AddUrlsSummary { added: 1, .. })
    ));
    handle.wait().await.unwrap();
    let snapshots: Vec<_> = sink
        .batches()
        .into_iter()
        .flat_map(|b| b.meta)
        .filter(|(key, _)| key == "progress_snapshot")
        .collect();
    assert!(!snapshots.is_empty());
    for (_, json) in snapshots {
        let _: os_model::Progress = serde_json::from_str(&json).unwrap();
    }
}

// spec: CRAWL-011, CRAWL-027
// Infrastruktur (M2-F05a) für Antworttransport; Fachprüfung M2-E01/E03.
#[tokio::test(start_paused = true)]
async fn every_response_class_transports_attempts_and_discovery_provenance() {
    let sink = MemorySink::new();
    let site = SimSite::new()
        .page("https://site.test/", "<a href='/redirect'>r</a><a href='https://other.test/'>e</a><link rel='stylesheet' href='/style.css'>")
        .redirect("https://site.test/redirect", 301, "/end")
        .page("https://site.test/end", "<title>end</title>")
        .asset("https://site.test/style.css", "text/css", "body {}")
        .asset("https://other.test/", "text/plain", "external");
    struct AttemptFixture(Arc<SimFetcher>);
    impl os_fetch::Fetch for AttemptFixture {
        fn fetch(
            &self,
            req: os_fetch::FetchRequest,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = os_fetch::FetchResult> + Send + '_>>
        {
            Box::pin(async move {
                let mut result = os_fetch::Fetch::fetch(&*self.0, req).await;
                let attempts = if result.url.path() == "/end" { 1 } else { 3 };
                if let os_fetch::FetchOutcome::Response(response) = &mut result.outcome {
                    response.attempts = attempts;
                }
                result
            })
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let handle = start_crawl(
        Arc::new(AttemptFixture(SimFetcher::new(site))),
        request(threads(1), "https://site.test/", &dir),
        sim_deps(&sink),
    )
    .await
    .unwrap();
    handle.wait().await.unwrap();
    let batches = sink.batches();
    let attempts = batches
        .iter()
        .flat_map(|b| &b.ext)
        .filter(|e| e.key.as_str() == "http.attempts")
        .map(|e| (e.url, e.value.clone()))
        .collect::<std::collections::HashMap<_, _>>();
    let robots: HashSet<_> = sink
        .new_urls()
        .iter()
        .filter(|u| u.url.ends_with("/robots.txt"))
        .map(|u| u.id)
        .collect();
    for page in sink
        .pages()
        .into_iter()
        .filter(|p| p.fetch.status_code > 0 && !robots.contains(&p.url_id))
    {
        let url = sink
            .new_urls()
            .into_iter()
            .find(|u| u.id == page.url_id)
            .unwrap();
        if url.url.ends_with("/end") {
            assert!(
                !attempts.contains_key(&page.url_id),
                "single attempt is implicit"
            );
        } else {
            assert_eq!(
                attempts.get(&page.url_id),
                Some(&os_model::Value::Int(3)),
                "{}",
                url.url
            );
        }
    }
    let edges: Vec<_> = batches.iter().flat_map(|b| &b.discoveries).collect();
    assert!(edges
        .iter()
        .any(|e| e.source.is_some() && e.link_type == Some(os_model::LinkType::Hyperlink)));
    let seqs: HashSet<_> = edges.iter().map(|e| e.seq).collect();
    assert_eq!(seqs.len(), edges.len());
}
