//! Referenzhardware und RSS-Messung: M2-Q03, kein Leistungsversprechen des normalen CI-Laufs.
#[path = "framework_common.rs"]
mod common;
use common::*;
use os_analyze::{
    analysis::{steps::near_duplicates, CancelToken},
    minhash,
};
use os_model::{CrawlConfig, StepState};
fn scale(size: usize, budget: std::time::Duration) {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = create(dir.path(), "scale.ospider", 7);
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
    let mut cfg = CrawlConfig::default();
    cfg.duplicates.near_enabled = true;
    let start = std::time::Instant::now();
    let cancel = CancelToken::new();
    assert_eq!(
        near_duplicates::run(&file, &cfg, &cancel).unwrap(),
        StepState::Done
    );
    eprintln!(
        "near_duplicates pages={size} elapsed_s={}",
        start.elapsed().as_secs_f64()
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
