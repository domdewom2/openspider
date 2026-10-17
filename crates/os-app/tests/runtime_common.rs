//! Gemeinsame Hilfen der Laufzeit-Tests (`#[path = "runtime_common.rs"] mod common;`). Als eigene Testdatei enthält
//! sie selbst keine Tests.
#![allow(dead_code)]
// Testcode legt Crawl-Dateien an und hält Sperren: die Ausnahme von `clippy.toml` gilt hier, nicht in Quellcode.
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use os_app::dto::{CmdError, OpState, OpStatus};
use os_app::{App, AppConfig, CallCtx};
use os_model::{CrawlConfig, CrawlId, CrawlMode, IdSource, ManualClock, SeededIds, Timestamp};
use os_store::{CrawlFile, CrawlInit, StorageProfile};

/// Anwendung unter `dir` (alle Daten dort, deterministische Kennungen).
pub fn new_app(dir: &Path) -> App {
    let config = AppConfig::new(dir)
        .with_ids(Arc::new(SeededIds::new(7)))
        .with_clock(Arc::new(ManualClock::new(created_at())));
    App::new(config).expect("App::new")
}

pub fn created_at() -> Timestamp {
    Timestamp::from_unix_millis(1_760_000_000_000)
}

pub fn crawl_id(seed: u64) -> CrawlId {
    SeededIds::new(seed).new_crawl_id(created_at())
}

/// Legt eine Crawl-Datei an und gibt ihren Pfad zurück (die Anlegeverbindung wird geschlossen).
pub fn make_crawl(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    let init = CrawlInit {
        crawl_id: crawl_id(1),
        mode: CrawlMode::Spider,
        start_url: Some("https://site.test/".into()),
        list_source: None,
        project: "Projekt".into(),
        name: "Testcrawl".into(),
        config: CrawlConfig::default(),
        created_at: created_at(),
    };
    CrawlFile::create(&path, &init, StorageProfile::Balanced).expect("Crawl anlegen");
    path
}

pub fn ctx() -> CallCtx {
    CallCtx::for_window("main")
}

pub fn code(e: &CmdError) -> String {
    format!("{:?}", e.code)
}

/// Wartet (höchstens `max`), bis `cond` wahr ist.
pub async fn eventually(max: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < max {
        if cond() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    cond()
}

pub fn state_of(statuses: &[OpStatus], op_id: &str) -> Option<OpState> {
    statuses.iter().find(|s| s.op_id == op_id).map(|s| s.state)
}

/// Eine SQL-Anweisung, die sehr lange läuft (rekursive Zählung), bis sie unterbrochen wird.
pub const SLOW_SQL: &str =
    "WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c WHERE x < 4000000000) SELECT count(*) FROM c";

/// Die Abbruchzeit-Zusage (UI-141): Median unter 100 ms; einzelne Ausreißer auf ausgelasteten CI-Läufern sind
/// tolerierbar, nicht aber über eine Sekunde.
pub fn assert_cancel_latency(mut samples: Vec<Duration>) {
    samples.sort();
    let median = samples[samples.len() / 2];
    let worst = *samples.last().unwrap();
    assert!(
        median < Duration::from_millis(100),
        "Abbruch dauerte im Median {median:?} ({samples:?})"
    );
    assert!(
        worst < Duration::from_secs(1),
        "Ausreißer {worst:?} ({samples:?})"
    );
}
