//! Lesen-dann-Schreiben-Transaktionen der Analyse neben einer gehaltenen Schreibsperre (CRAWL-126, OS-Festlegung
//! vom 01.11.2026): Die WAL-Rückführung nach Pause/Ende hält die Schreibsperre bis zu 100 ms je Versuch. Eine
//! verzögerte Transaktion bekäme beim Hochstufen sofort `SQLITE_BUSY`; die betroffenen Stellen warten jetzt mit
//! `BEGIN IMMEDIATE` im Busy-Timeout.
#[path = "aggregates_fixture.rs"]
mod fixture;

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use os_analyze::aggregates::status::{analysis_status, on_config_changed};
use os_analyze::analysis::{self, persist, CancelToken};
use os_analyze::state::step_config_hash;
use os_model::{
    time::{IdSource, SeededIds},
    AnalysisStep, CrawlConfig, CrawlMode, StepState, Timestamp,
};
use os_store::{CrawlFile, CrawlInit, StorageProfile};

/// So lange hält die zweite Verbindung die Schreibsperre.
const HOLD: Duration = Duration::from_millis(200);

/// Eine zweite Verbindung nimmt die Schreibsperre (wie die WAL-Rückführung, während sie auf Leser wartet) und gibt
/// sie nach [`HOLD`] frei; kehrt zurück, sobald die Sperre gehalten wird.
fn hold_write_lock(path: &Path) -> std::thread::JoinHandle<()> {
    let (tx, rx) = mpsc::channel();
    let path: PathBuf = path.to_path_buf();
    let handle = std::thread::spawn(move || {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("BEGIN IMMEDIATE").unwrap();
        tx.send(()).unwrap();
        std::thread::sleep(HOLD);
        conn.execute_batch("ROLLBACK").unwrap();
    });
    rx.recv().unwrap();
    handle
}

fn images_fixture(dir: &Path) -> (CrawlFile, CrawlConfig) {
    let mut cfg = CrawlConfig::default();
    cfg.images.parse_inline_css_backgrounds = true;
    let at = Timestamp::from_db("2026-10-18T00:00:00.000Z").unwrap();
    let file = CrawlFile::create(
        &dir.join("images.ospider"),
        &CrawlInit {
            crawl_id: SeededIds::new(3).new_crawl_id(at),
            mode: CrawlMode::Spider,
            start_url: Some("https://site.test/".into()),
            list_source: None,
            project: "Images".into(),
            name: "Fixture".into(),
            config: cfg.clone(),
            created_at: at,
        },
        StorageProfile::Balanced,
    )
    .unwrap();
    file.conn()
        .execute_batch(include_str!(
            "../../../tests/fixtures/onpage-m2-images/analysis/facts.sql"
        ))
        .unwrap();
    file.conn().busy_timeout(Duration::from_secs(5)).unwrap();
    (file, cfg)
}

fn backgrounds(file: &CrawlFile) -> i64 {
    file.conn()
        .query_row(
            "SELECT COUNT(*) FROM url_flags WHERE flag='image_background'",
            [],
            |r| r.get(0),
        )
        .unwrap()
}

// spec: CRAWL-126
// spec: CRAWL-141
// spec: ONPAGE-178
#[test]
fn image_step_publishes_after_a_held_write_lock_instead_of_failing_busy() {
    let dir = tempfile::tempdir().unwrap();
    let (file, cfg) = images_fixture(dir.path());
    assert_eq!(backgrounds(&file), 0);
    let lock = hold_write_lock(file.path());
    let t = Instant::now();
    let result = analysis::steps::images::run(&file, &cfg, &CancelToken::new());
    let waited = t.elapsed();
    lock.join().unwrap();
    assert!(
        matches!(result, Ok(StepState::Done)),
        "{result:?} after {waited:?}"
    );
    assert!(waited >= HOLD / 2, "waited only {waited:?} for the lock");
    assert_eq!(backgrounds(&file), 2, "bg.png and dynamic.png");
    let scratch: i64 = file
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM temp.sqlite_master WHERE name LIKE 'd03_%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(scratch, 0, "scratch tables are removed after the step");
}

// spec: CRAWL-126
// spec: UI-082
#[test]
fn config_change_marks_steps_stale_after_a_held_write_lock_instead_of_failing_busy() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture::create(dir.path());
    fixture::depth_fixture(&file);
    file.conn().busy_timeout(Duration::from_secs(5)).unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.analysis.near_duplicates = true;
    for &step in AnalysisStep::ALL {
        persist::write_step(
            &file,
            step,
            StepState::Done,
            None,
            Some(&step_config_hash(step, &cfg)),
        )
        .unwrap();
    }
    persist::write_run_state(&file, persist::RunState::Completed, None).unwrap();
    let mut new = cfg.clone();
    new.duplicates.near_threshold_pct = 83;
    let lock = hold_write_lock(file.path());
    let t = Instant::now();
    let result = on_config_changed(&file, &cfg, &new);
    let waited = t.elapsed();
    lock.join().unwrap();
    assert!(result.is_ok(), "{result:?} after {waited:?}");
    assert!(waited >= HOLD / 2, "waited only {waited:?} for the lock");
    assert_eq!(
        analysis_status(&file, &new).stale,
        vec![AnalysisStep::NearDuplicates]
    );
}
