//! Absturz mitten im Crawl (CRAWL-125, CRAWL-128, PERF-005-Vorlage): Ein Kindprozess (`examples/crawl_child.rs`)
//! crawlt eine synthetische Site und wird hart beendet (SIGKILL bzw. `TerminateProcess`). Danach ist die Datei
//! konsistent und „Unterbrochen“, und das Fortsetzen ergibt dieselbe URL-Menge mit denselben Statuscodes wie ein
//! ununterbrochener Crawl.

#[path = "lifecycle_support.rs"]
mod support;

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use os_model::time::ManualClock;
use os_model::Clock;
use os_model::{CrawlState, EndReason};
use os_testsite::{SiteOptions, SyntheticConfig, TestSite};
use support::*;

const THREADS: u32 = 4;

/// Die synthetische Site: 1 500 Seiten, 10 Links je Seite, einige Weiterleitungen und Fehler, jede Antwort
/// 10 ms langsam (damit der Abbruch mitten im Crawl liegt).
fn synthetic() -> SyntheticConfig {
    SyntheticConfig {
        pages: 1_500,
        links_per_page: 10,
        seed: 42,
        status_301_share: 0.02,
        status_404_share: 0.02,
        status_5xx_share: 0.01,
        delay_ms: 10,
        slow_share: 1.0,
        ..SyntheticConfig::default()
    }
}

async fn site() -> TestSite {
    TestSite::start_synthetic(
        synthetic(),
        SiteOptions {
            port: 0,
            log_requests: true,
        },
    )
    .await
    .unwrap()
}

/// Pfad des Beispielprogramms `crawl_child`; fehlt es (z. B. bei `cargo test --test lifecycle_crash`), wird es
/// gebaut.
fn child_binary() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    let profile_dir = exe.parent().unwrap().parent().unwrap();
    let path = profile_dir
        .join("examples")
        .join(format!("crawl_child{}", std::env::consts::EXE_SUFFIX));
    if !path.exists() {
        let status = Command::new(env!("CARGO"))
            .args([
                "build",
                "--example",
                "crawl_child",
                "-p",
                "os-engine",
                "--locked",
            ])
            .status()
            .expect("cargo startet");
        assert!(status.success(), "crawl_child lässt sich nicht bauen");
    }
    assert!(path.exists(), "{} fehlt", path.display());
    path
}

/// Zeilen der Standardausgabe des Kindes, aus einem eigenen Thread gelesen.
fn lines_of(child: &mut std::process::Child) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel();
    let stdout = child.stdout.take().unwrap();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    rx
}

fn crawled_of(line: &str) -> Option<u64> {
    line.strip_prefix("PROGRESS crawled=")?
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

// spec: CRAWL-125, CRAWL-128
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_killed_crawl_is_incomplete_and_consistent_and_resumes_to_the_reference_result() {
    // Referenz: ununterbrochen, im eigenen Prozess
    let ref_site = site().await;
    let ref_work = tempfile::tempdir().unwrap();
    let ref_path = crawl_file_in(&ref_work, "ref.ospider");
    let ref_summary =
        run_to_end(start(config_for(&ref_site, THREADS), &ref_path, deps()).await).await;
    assert_eq!(ref_summary.end_reason, Some(EndReason::QueueEmpty));
    assert!(ref_summary.crawled >= 1_000, "{}", ref_summary.crawled);

    // Kindprozess crawlt dieselbe Site (andere Instanz, anderer Port, gleicher Host `site.test`)
    let site = site().await;
    let work = tempfile::tempdir().unwrap();
    let path = crawl_file_in(&work, "crawl.ospider");
    let mut child = Command::new(child_binary())
        .arg("--file")
        .arg(&path)
        .args(["--start", &start_url()])
        .args(["--resolve", &format!("*.test={}", site.addr())])
        .args(["--threads", &THREADS.to_string()])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("crawl_child startet");
    let lines = lines_of(&mut child);

    // lesen, bis genug gecrawlt ist, und den Fortschritt mit Zeitstempeln festhalten
    let mut progress: Vec<(Instant, u64)> = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let line = lines
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("das Kind meldet Fortschritt");
        if let Some(n) = crawled_of(&line) {
            progress.push((Instant::now(), n));
            if n >= 700 {
                break;
            }
        }
    }
    let killed_at = Instant::now();
    child.kill().expect("hart beenden");
    let status = child.wait().unwrap();
    assert!(
        !status.success(),
        "ein harter Abbruch endet nicht mit Exit 0: {status:?}"
    );

    // Datei: konsistent, Zustand „Unterbrochen“ (der Heartbeat bleibt aus), nichts doppelt
    let file = open_ro(&path);
    assert!(file.integrity_check().unwrap(), "PRAGMA integrity_check");
    assert_eq!(
        file.meta().unwrap().state,
        CrawlState::Running,
        "kein geordnetes Ende"
    );
    let heartbeat = file.meta().unwrap().heartbeat_at.expect("Heartbeat");
    let later = ManualClock::new(heartbeat);
    later.advance(Duration::from_secs(60));
    assert_eq!(
        file.effective_state(&later).unwrap(),
        CrawlState::Incomplete
    );
    assert_eq!(
        scalar(&file, "SELECT COUNT(*) - COUNT(DISTINCT url) FROM urls"),
        0,
        "keine URL doppelt in urls"
    );
    // alle bis 2 s vor dem Abbruch gecrawlten URLs sind in der Datei
    let in_file = scalar(
        &file,
        "SELECT COUNT(*) FROM urls WHERE state = 'done' AND discovered_via != 'robots_txt'",
    ) as u64;
    if let Some((_, reported)) = progress
        .iter()
        .rev()
        .find(|(t, _)| killed_at.duration_since(*t) >= Duration::from_secs(2))
    {
        assert!(
            in_file >= *reported,
            "{in_file} in der Datei, {reported} vor 2 s gemeldet"
        );
    }
    assert!(in_file >= 100, "{in_file}");
    assert_eq!(
        scalar(
            &file,
            "SELECT COUNT(*) FROM urls WHERE state = 'queued' AND id NOT IN (SELECT url_id FROM queue)"
        ),
        0,
        "Warteschlange und Zustand stimmen überein"
    );
    drop(file);

    // Fortsetzen
    let cfg = resume_config(&path, &site, |_| {});
    // Der Heartbeat stammt von der Systemuhr des Kindes: das Fortsetzen läuft „eine Minute später“.
    let resumed = run_to_end(resume(cfg, &path, deps_from(later.now())).await).await;
    assert_eq!(resumed.state, CrawlState::Completed);
    assert_eq!(resumed.end_reason, Some(EndReason::QueueEmpty));
    assert_eq!(
        statuses(&path),
        statuses(&ref_path),
        "URL-Menge und Status je URL wie im ununterbrochenen Crawl"
    );
    let file = open_ro(&path);
    assert!(file.integrity_check().unwrap());
    assert_eq!(
        scalar(&file, "SELECT COUNT(*) - COUNT(DISTINCT url) FROM urls"),
        0
    );
    assert_eq!(scalar(&file, "SELECT COUNT(*) FROM queue"), 0);

    // wiederholte Abrufe: nur für URLs, die beim Abbruch in Bearbeitung oder im offenen Batch waren
    let counts = site.request_counts();
    let repeated: u64 = counts.values().map(|c| c.saturating_sub(1)).sum();
    let bound = u64::from(THREADS) + 200;
    eprintln!(
        "lifecycle_crash: {} Abrufe, davon {repeated} wiederholt (Grenze {bound})",
        counts.values().sum::<u64>()
    );
    assert!(
        repeated <= bound,
        "{repeated} wiederholte Abrufe (Grenze {bound})"
    );
}
