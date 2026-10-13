//! Determinismus (CRAWL-124): mit fixierter Uhr, festem Seed, einem Thread und deterministischer Fixture
//! erzeugen zwei Crawls nach dem abschließenden Checkpoint byte-identische Crawl-Dateien.

#[path = "lifecycle_support.rs"]
mod support;

use std::sync::Arc;
use std::time::Duration;

use os_engine::testing::{coalescing_sink_factory, test_deps};
use os_engine::EngineDeps;
use os_model::{CrawlConfig, FixedClock, SeededIds, Timestamp};
use support::*;

/// Abhängigkeiten mit stehender Uhr (`FixedClock` mit Schritt 0) und festem Seed.
fn fixed_deps() -> EngineDeps {
    let mut d = test_deps();
    let start = Timestamp::from_db("2026-03-04T05:06:07.000Z").unwrap();
    d.clock = Arc::new(FixedClock::new(start, Duration::ZERO));
    d.ids = Arc::new(SeededIds::new(7));
    d
}

/// Wie `fixed_deps`, mit der Prüf-Senke: alle Batches eines Laufs als ein Batch (siehe `CoalescingSink`).
fn checked_deps() -> EngineDeps {
    let mut d = fixed_deps();
    d.sink_factory = Some(coalescing_sink_factory(d.clock.clone()));
    d
}

fn deterministic_config(site: &os_testsite::TestSite) -> CrawlConfig {
    let mut cfg = config_for(site, 1);
    // die Antwortzeit kommt von der echten Uhr des Servers und ist kein Teil der Determinismus-Zusage
    cfg.extraction.response_time = false;
    cfg
}

// spec: CRAWL-124
#[tokio::test]
async fn two_crawls_with_a_fixed_clock_a_seed_and_one_thread_are_byte_identical() {
    let dir = slow_site_dir(0);
    // dieselbe Testsite für beide Läufe: gleicher Port, damit auch die gespeicherte Konfiguration übereinstimmt
    let site = start_site(dir.path()).await;
    let work = tempfile::tempdir().unwrap();
    let (a, b) = (
        crawl_file_in(&work, "a.ospider"),
        crawl_file_in(&work, "b.ospider"),
    );
    for path in [&a, &b] {
        let summary =
            run_to_end(start(deterministic_config(&site), path, checked_deps()).await).await;
        assert_eq!(summary.state, os_model::CrawlState::Completed);
    }
    let (bytes_a, bytes_b) = (std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
    assert!(
        bytes_a.len() > 100_000,
        "eine echte Crawl-Datei: {} Bytes",
        bytes_a.len()
    );
    assert_eq!(bytes_a.len(), bytes_b.len());
    assert!(bytes_a == bytes_b, "die Dateien unterscheiden sich");
    // keine WAL-Reste: alles steht in der Hauptdatei
    for suffix in ["-wal", "-shm"] {
        let mut p = a.as_os_str().to_owned();
        p.push(suffix);
        assert!(
            std::fs::metadata(&p).map_or(true, |m| m.len() == 0),
            "{suffix} nicht leer"
        );
    }
    // Besitzer-Prozess ist am Ende geleert, die Kennung stammt aus dem Seed
    let file = open_ro(&a);
    assert_eq!(file.get_meta("owner_pid").unwrap().as_deref(), Some(""));
    assert_eq!(
        file.meta().unwrap().crawl_id,
        open_ro(&b).meta().unwrap().crawl_id
    );
}

// spec: CRAWL-124
#[tokio::test]
async fn with_the_standard_writer_only_the_batch_sequence_depends_on_timing() {
    let dir = slow_site_dir(0);
    let site = start_site(dir.path()).await;
    let work = tempfile::tempdir().unwrap();
    let (a, b) = (
        crawl_file_in(&work, "a.ospider"),
        crawl_file_in(&work, "b.ospider"),
    );
    for path in [&a, &b] {
        run_to_end(start(deterministic_config(&site), path, fixed_deps()).await).await;
    }
    // Inhalt wie in der Datei, aber ohne die Batch-Sequenz des Änderungsfeeds
    let dump = |path: &std::path::Path| -> Vec<String> {
        let file = open_ro(path);
        let mut out = Vec::new();
        for table in [
            "crawl_meta",
            "urls",
            "links",
            "image_instances",
            "canonicals",
            "directives",
            "refresh",
            "hreflang",
            "pagination",
            "http_headers",
            "robots_files",
            "sitemaps",
            "queue",
            "url_flags",
            "crawl_events",
            "crawl_counters",
        ] {
            let cols: Vec<String> = file
                .conn()
                .prepare(&format!("PRAGMA table_info({table})"))
                .unwrap()
                .query_map([], |r| r.get::<_, String>(1))
                .unwrap()
                .map(Result::unwrap)
                .filter(|c| c != "change_seq")
                .collect();
            let select = cols
                .iter()
                .map(|c| format!("quote({c})"))
                .collect::<Vec<_>>()
                .join(" || '|' || ");
            let mut stmt = file
                .conn()
                .prepare(&format!("SELECT {select} FROM {table} ORDER BY 1"))
                .unwrap();
            let rows = stmt.query_map([], |r| r.get::<_, String>(0)).unwrap();
            for row in rows {
                let line = row.unwrap();
                // der Wert von `crawl_meta.change_seq` ist die Batch-Zahl selbst
                if !line.starts_with("'change_seq'|") {
                    out.push(format!("{table}: {line}"));
                }
            }
        }
        out
    };
    let (da, db) = (dump(&a), dump(&b));
    assert!(da.len() > 500, "{} Zeilen", da.len());
    assert_eq!(da, db);
}
