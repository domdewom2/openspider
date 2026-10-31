//! Referenzhardware und RSS-Messung: M2-Q03, kein Leistungsversprechen des normalen CI-Laufs.
#[path = "near_dup_support/baseline.rs"]
mod baseline;
#[path = "framework_common.rs"]
mod common;
#[path = "near_dup_support/transaction.rs"]
mod transaction;
use common::*;
use os_analyze::{
    analysis::{steps::near_duplicates, CancelToken},
    minhash,
};
use os_model::{CrawlConfig, StepState};
fn prepare(size: usize) -> (tempfile::TempDir, os_store::CrawlFile) {
    // Optional reusable fixture; timing still covers only the analysis, as before.
    let dir = tempfile::tempdir().unwrap();
    let reuse = std::env::var_os("OPENSPIDER_NEAR_DUP_FIXTURE").map(std::path::PathBuf::from);
    let path = reuse.as_deref().unwrap_or(dir.path());
    std::fs::create_dir_all(path).unwrap();
    let db = path.join(format!("scale-{size}.ospider"));
    let exists = db.exists();
    let file = if exists {
        os_store::CrawlFile::open(&db, os_store::OpenMode::ReadWrite).unwrap()
    } else {
        create(path, &format!("scale-{size}.ospider"), 7).1
    };
    if !exists {
        file.conn().execute_batch("BEGIN").unwrap();
        for id in 1..=size {
            let text = (0..500)
                .map(|n| format!("page{id}word{n}"))
                .collect::<Vec<_>>()
                .join(" ");
            insert(
                &file,
                &[row(id as i64, &format!("{id}.html"))
                    .text("content_class", "HTML")
                    .int("status_code", 200)],
            );
            file.conn()
                .execute(
                    "INSERT INTO url_ext(url_id,key,v_text) VALUES (?1,'content.text',?2)",
                    rusqlite::params![id as i64, text],
                )
                .unwrap();
            let sig = minhash::signature(&minhash::shingles(&text));
            file.conn()
                .execute(
                    "INSERT INTO minhash VALUES (?1,?2)",
                    rusqlite::params![id as i64, os_store::encode_minhash(&sig)],
                )
                .unwrap();
        }
        file.conn().execute_batch("COMMIT").unwrap();
    }
    let count: i64 = file
        .conn()
        .query_row("SELECT COUNT(*) FROM minhash", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        count, size as i64,
        "incomplete reusable fixture: remove its directory and rebuild"
    );
    (dir, file)
}

fn scale(size: usize, budget: std::time::Duration) {
    let (_dir, file) = prepare(size);
    let mut cfg = CrawlConfig::default();
    cfg.duplicates.near_enabled = true;
    let start = std::time::Instant::now();
    let cancel = CancelToken::new();
    let run = if std::env::var_os("OPENSPIDER_NEAR_DUP_BASELINE").is_some() {
        baseline::run
    } else if std::env::var_os("OPENSPIDER_NEAR_DUP_TRANSACTION_ONLY").is_some() {
        transaction::run
    } else {
        near_duplicates::run
    };
    assert_eq!(run(&file, &cfg, &cancel).unwrap(), StepState::Done);
    eprintln!(
        "near_duplicates pages={size} elapsed_s={}",
        start.elapsed().as_secs_f64()
    );
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        let kib: u64 = status
            .lines()
            .find(|line| line.starts_with("VmHWM:"))
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        eprintln!("near_duplicates process_peak_rss_kib={kib}");
        assert!(kib < 2_000_000_000 / 1024);
    }
    let pairs: i64 = file
        .conn()
        .query_row("SELECT COUNT(*) FROM near_duplicates", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        pairs, 0,
        "page-specific shingles have zero pairwise Jaccard similarity"
    );
    assert!(start.elapsed() < budget);
    assert_eq!(cancel.progress(), 1.0);
}
// spec: ONPAGE-144
#[test]
#[ignore = "Benchmark: 100000 Seiten à 500 Wörter, Referenzhardware M2-Q03"]
fn hundred_thousand_pages_under_sixty_seconds() {
    scale(100_000, std::time::Duration::from_secs(60));
}
// spec: PERF-007
#[test]
#[ignore = "Benchmark: 1 Mio. Seiten, RSS extern messen (<2 GB), Referenzhardware M2-Q03"]
fn million_pages_under_ten_minutes() {
    scale(1_000_000, std::time::Duration::from_secs(600));
}

// spec: PERF-007
#[test]
#[ignore = "Benchmark: fixture preparation only, no analysis or performance assertion"]
fn n16_prepare_million_page_fixture() {
    let _fixture = prepare(1_000_000);
    eprintln!("n16 fixture prepared; analysis NOT run; no performance result");
}
