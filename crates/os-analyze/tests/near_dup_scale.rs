//! Referenzhardware und RSS-Messung: M2-Q03, kein Leistungsversprechen des normalen CI-Laufs.
#[path = "near_dup_support/baseline.rs"]
mod baseline;
#[path = "framework_common.rs"]
mod common;
#[path = "near_dup_support/profile.rs"]
mod profile;
use common::*;
use os_analyze::{
    analysis::{steps::near_duplicates, CancelToken},
    minhash,
};
use os_model::{CrawlConfig, StepState};
fn prepare(size: usize, near: bool) -> (tempfile::TempDir, os_store::CrawlFile) {
    // Optional reusable fixture; timing still covers only the analysis, as before.
    let dir = tempfile::tempdir().unwrap();
    let reuse = std::env::var_os("OPENSPIDER_NEAR_DUP_FIXTURE").map(std::path::PathBuf::from);
    let path = reuse.as_deref().unwrap_or(dir.path());
    std::fs::create_dir_all(path).unwrap();
    let name = if near {
        format!("scale-near-v1-{size}.ospider")
    } else {
        format!("scale-{size}.ospider")
    };
    let db = path.join(&name);
    let exists = db.exists();
    let file = if exists {
        os_store::CrawlFile::open(&db, os_store::OpenMode::ReadWrite).unwrap()
    } else {
        create(path, &name, 7).1
    };
    if !exists {
        file.conn().execute_batch("BEGIN").unwrap();
        for id in 1..=size {
            // Exactly 5% in groups of four. Mutate one edge word per member:
            // 497 three-word shingles remain shared (Jaccard >= 99%).
            let grouped = near && id <= size / 20;
            let group = (id - 1) / 4;
            let text = (0..500)
                .map(|n| {
                    if grouped && n > 0 {
                        format!("article{group}word{n}")
                    } else {
                        format!("page{id}word{n}")
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");
            insert(
                &file,
                &[row(id as i64, &format!("{id}.html"))
                    .text("content_class", "HTML")
                    .int("status_code", 200)
                    .text("hash_md5", &format!("{id:032x}"))],
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

fn scale(size: usize, near: bool, budget: std::time::Duration) {
    let (_dir, file) = prepare(size, near);
    let mut cfg = CrawlConfig::default();
    cfg.duplicates.near_enabled = true;
    let start = std::time::Instant::now();
    let cancel = CancelToken::new();
    let run = if std::env::var_os("OPENSPIDER_NEAR_DUP_BASELINE").is_some() {
        baseline::run
    } else {
        near_duplicates::run
    };
    assert_eq!(
        profile::capture(|| run(&file, &cfg, &cancel)).unwrap(),
        StepState::Done
    );
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
        pairs,
        if near { (size / 20 / 4 * 12) as i64 } else { 0 },
        "each four-page article contributes twelve directed pairs; other pages are disjoint"
    );
    if near {
        let (minimum, maximum): (f64, f64) = file
            .conn()
            .query_row(
                "SELECT MIN(similarity),MAX(similarity) FROM near_duplicates",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        let expected = (497_f64 / 499_f64).to_bits();
        assert_eq!(minimum.to_bits(), expected);
        assert_eq!(maximum.to_bits(), expected);
    }
    assert!(start.elapsed() < budget);
    assert_eq!(cancel.progress(), 1.0);
}
// spec: ONPAGE-144
#[test]
#[ignore = "Benchmark: 100000 Seiten à 500 Wörter, Referenzhardware M2-Q03"]
fn hundred_thousand_pages_under_sixty_seconds() {
    scale(100_000, false, std::time::Duration::from_secs(60));
}
// spec: PERF-007
#[test]
#[ignore = "Benchmark: 1 Mio. Seiten, RSS extern messen (<2 GB), Referenzhardware M2-Q03"]
fn million_pages_under_ten_minutes() {
    scale(1_000_000, false, std::time::Duration::from_secs(600));
}

// spec: PERF-007
#[test]
#[ignore = "Benchmark: fixture preparation only, no analysis or performance assertion"]
fn n16_prepare_million_page_fixture() {
    let _fixture = prepare(1_000_000, false);
    eprintln!("n16 fixture prepared; analysis NOT run; no performance result");
}

// spec: PERF-007
#[test]
#[ignore = "Benchmark: 100k pages, 5% near duplicates, RSS <2 GB"]
fn hundred_thousand_pages_with_near_duplicates_under_ten_minutes() {
    scale(100_000, true, std::time::Duration::from_secs(600));
}

// spec: PERF-007
#[test]
#[ignore = "Benchmark: 1M pages, 5% near duplicates, RSS <2 GB, reference hardware M2-Q03"]
fn million_pages_with_near_duplicates_under_ten_minutes() {
    scale(1_000_000, true, std::time::Duration::from_secs(600));
}

// spec: PERF-007
#[test]
fn n16_near_fixture_exercises_comparison_and_directed_pair_count() {
    // 400 pages include five groups of four; 60 directed results.
    scale(400, true, std::time::Duration::from_secs(600));
}
