//! Benchmarks des Writers (nicht Teil des normalen Testlaufs; P28 führt sie aus):
//!
//! ```text
//! cargo test -p os-store --release --test bench_writer -- --ignored --nocapture
//! ```
//!
//! * 100 000 synthetische Seiten mit je 30 Links in unter 60 s (DoD M1-P07), Bericht je Profil. Die Links
//!   sind seitentypisch verteilt: 10 gemeinsame Ziele (Navigation/Fußzeile) und 20 Nachbarseiten.
//!   Zusätzlich ein Bericht (ohne Schranke) für gleichverteilte Zufallsziele – der ungünstigste Fall, in dem
//!   jede Seite 30 verschiedene Zeilen von `urls` und des Link-Index berührt.
//! * Inlinks-Paging bei 1 Mio. Inlinks auf eine URL (Seitenabfrage ohne Vollmaterialisierung).

mod store_common;

use std::time::{Duration, Instant};

use os_model::WriteBatch;
use os_store::queries::{self, LinkSortKey, PageReq};
use os_store::{counters, CrawlFile, CrawlSink, OpenMode, StorageProfile};
use store_common::*;

const PAGES: i64 = 100_000;
const LINKS_PER_PAGE: i64 = 30;
const PAGES_PER_BATCH: i64 = 200;
const HUBS: i64 = 10;

#[derive(Clone, Copy, Debug)]
enum Targets {
    /// 10 gemeinsame Ziele und 20 Nachbarseiten.
    SiteLike,
    /// Gleichverteilt über alle Seiten.
    Uniform,
}

fn target(kind: Targets, id: i64, k: i64) -> i64 {
    match kind {
        Targets::SiteLike if k < HUBS => k + 1,
        Targets::SiteLike => (id + k - HUBS) % PAGES + 1,
        Targets::Uniform => (id * 31 + k * 977) % PAGES + 1,
    }
}

fn run(profile: StorageProfile, kind: Targets) -> Duration {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "bench.ospider", profile);
    let mut sink = sink(&path, profile);

    let start = Instant::now();
    if matches!(kind, Targets::Uniform) {
        // Alle Ziele müssen vor den Links existieren.
        let mut first = 1;
        while first <= PAGES {
            let last = (first + 4_999).min(PAGES);
            sink.write(batch_with_urls((first..=last).map(|i| (i, true))))
                .unwrap();
            first = last + 1;
        }
    }
    let mut first = 1;
    while first <= PAGES {
        let last = (first + PAGES_PER_BATCH - 1).min(PAGES);
        let mut batch: WriteBatch = WriteBatch::default();
        if matches!(kind, Targets::SiteLike) {
            // Wie die Engine: Ziele werden mit der verlinkenden Seite entdeckt.
            let known = if first == 1 {
                0
            } else {
                first - 1 + LINKS_PER_PAGE
            };
            let upto = (last + LINKS_PER_PAGE).min(PAGES);
            batch = batch_with_urls((known + 1..=upto).map(|i| (i, true)));
        }
        for id in first..=last {
            let links = (0..LINKS_PER_PAGE)
                .map(|k| hyperlink(target(kind, id, k), u32::try_from(k).unwrap()))
                .collect();
            batch.pages.push(page(id, links));
        }
        sink.write(batch).unwrap();
        first = last + 1;
    }
    sink.finish().unwrap();
    let elapsed = start.elapsed();

    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    assert_eq!(count(&file, "SELECT COUNT(*) FROM urls"), PAGES);
    assert_eq!(
        count(&file, "SELECT COUNT(*) FROM links"),
        PAGES * LINKS_PER_PAGE
    );
    // Stichprobe der Zähler gegen die Neuberechnung.
    for id in [1, 5, 777, 50_000, PAGES] {
        assert_eq!(
            counters::stored(file.conn(), id).unwrap(),
            counters::recompute(file.conn(), id).unwrap(),
            "URL {id}"
        );
    }
    println!(
        "Writer {profile:?}/{kind:?}: {PAGES} Seiten x {LINKS_PER_PAGE} Links in {:.1} s ({:.0} Seiten/s, {:.0} Links/s), Datei {} MiB",
        elapsed.as_secs_f64(),
        PAGES as f64 / elapsed.as_secs_f64(),
        (PAGES * LINKS_PER_PAGE) as f64 / elapsed.as_secs_f64(),
        std::fs::metadata(&path).unwrap().len() / (1024 * 1024),
    );
    elapsed
}

#[test]
#[ignore = "Benchmark (P28): 100 000 Seiten x 30 Links"]
fn writer_100k_pages_with_30_links_under_60_seconds() {
    for profile in [
        StorageProfile::Balanced,
        StorageProfile::MaxSpeed,
        StorageProfile::MinMemory,
    ] {
        let elapsed = run(profile, Targets::SiteLike);
        assert!(elapsed.as_secs() < 60, "{profile:?}: {elapsed:?}");
    }
}

#[test]
#[ignore = "Benchmark (P28): 100 000 Seiten x 30 Links, gleichverteilte Ziele (Bericht)"]
fn writer_100k_pages_uniform_targets_report() {
    run(StorageProfile::Balanced, Targets::Uniform);
}

#[test]
#[ignore = "Benchmark (P28): 1 Mio. Inlinks auf eine URL"]
fn inlinks_paging_with_one_million_inlinks() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "inl.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);
    // 100 000 Quellseiten verlinken die Startseite je zehnmal.
    let sources = 100_000i64;
    sink.write(batch_with_urls([(1, true)])).unwrap();
    let mut first = 2;
    while first <= sources + 1 {
        let last = (first + PAGES_PER_BATCH - 1).min(sources + 1);
        let mut batch = batch_with_urls((first..=last).map(|i| (i, true)));
        for id in first..=last {
            let links = (0..10).map(|k| hyperlink(1, k)).collect();
            batch.pages.push(page(id, links));
        }
        sink.write(batch).unwrap();
        first = last + 1;
    }
    sink.finish().unwrap();
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();

    let measure = |req: PageReq| {
        let t = Instant::now();
        let p = queries::inlinks(file.conn(), os_model::UrlId(1), req).unwrap();
        assert_eq!(p.total, 1_000_000);
        assert_eq!(p.items.len(), 50);
        t.elapsed().as_secs_f64() * 1000.0
    };
    let first_page = measure(PageReq::new(0, 50));
    let deep_page = measure(PageReq::new(500_000, 50));
    let mut by_anchor = PageReq::new(500_000, 50);
    by_anchor.sort = vec![LinkSortKey {
        column: "Anchor".into(),
        desc: true,
    }];
    let deep_sorted = measure(by_anchor);
    println!(
        "Inlinks 1 Mio.: erste Seite {first_page:.0} ms, Offset 500000 {deep_page:.0} ms, Offset 500000 nach Anchor {deep_sorted:.0} ms"
    );
}

// ---------------------------------------------------------------------------------------------------------
// M1-P32: 1 Mio. Seiten x 30 gleichverteilte Linkziele in zehn Abschnitten (PERF-002, K-72/K-74/K-75)
// ---------------------------------------------------------------------------------------------------------

/// Seiten je Transaktion wie `os_engine::writer_thread::BATCH_PAGES_BACKLOG` (os-store hängt nicht an os-engine).
const BACKLOG_PAGES: i64 = 2_000;
/// Abschnitte zu je 10 % der Seiten.
const SEGMENTS: usize = 10;

/// SplitMix64: gleichverteilte, vom Zufall der Reihenfolge unabhängige Linkziele (die Formel des älteren
/// `Targets::Uniform` ist eine Folge mit festem Abstand und trifft je Batch dicht beieinanderliegende Zeilen).
fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

fn spread_target(pages: i64, id: i64, k: i64) -> i64 {
    let h = mix(((id as u64) << 8) | (k as u64));
    i64::try_from(h % (pages as u64)).unwrap() + 1
}

/// Höchster Speicherverbrauch des Prozesses in Bytes (0, wenn nicht ermittelbar).
fn peak_rss_bytes() -> u64 {
    #[cfg(unix)]
    {
        // SAFETY: `getrusage` schreibt in eine ausreichend große, nullinitialisierte Struktur.
        let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
        if unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) } != 0 {
            return 0;
        }
        let max = u64::try_from(usage.ru_maxrss).unwrap_or(0);
        // macOS meldet Bytes, Linux Kibibytes.
        if cfg!(target_os = "macos") {
            max
        } else {
            max * 1024
        }
    }
    #[cfg(not(unix))]
    {
        0
    }
}

fn diag_map(snapshot: Vec<(&'static str, u64)>) -> serde_json::Value {
    serde_json::Value::Object(
        snapshot
            .into_iter()
            .map(|(k, v)| (k.to_string(), serde_json::Value::from(v)))
            .collect(),
    )
}

// spec: PERF-001, PERF-002, PERF-003
#[test]
#[ignore = "Benchmark (P32): 1 Mio. Seiten x 30 gleichverteilte Linkziele, zehn Abschnitte (OS_BENCH_PAGES, OS_BENCH_OUT, OS_BENCH_DIR, OS_BENCH_AUTOCHECKPOINT)"]
fn writer_1m_uniform_segments() {
    let pages: i64 = std::env::var("OS_BENCH_PAGES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1_000_000);
    let per_segment = pages / SEGMENTS as i64;
    let profile = StorageProfile::Balanced;
    let dir = tempfile::tempdir_in(
        std::env::var_os("OS_BENCH_DIR").unwrap_or_else(|| std::env::temp_dir().into()),
    )
    .unwrap();
    let path = new_crawl(dir.path(), "segments.ospider", profile);
    let mut sink = sink(&path, profile);
    // A/B zum Hintergrund-Checkpoint (K-76): Mit OS_BENCH_AUTOCHECKPOINT=<Seiten> setzt der Benchmark den Wert von
    // außen; der Sink lässt ihn unangetastet, es läuft kein Hintergrund-Thread, SQLite checkpointet im COMMIT.
    if let Some(pages) = std::env::var("OS_BENCH_AUTOCHECKPOINT")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
    {
        sink.file()
            .conn()
            .pragma_update(None, "wal_autocheckpoint", pages)
            .unwrap();
    }

    // Alle URLs vorab als NewUrl bekannt (wie in der Vorprüfung des Planers).
    let t_urls = Instant::now();
    let mut first = 1;
    while first <= pages {
        let last = (first + 4_999).min(pages);
        sink.write(batch_with_urls((first..=last).map(|i| (i, true))))
            .unwrap();
        first = last + 1;
    }
    let urls_insert_s = t_urls.elapsed().as_secs_f64();

    os_store::sink::diag::reset();
    let wall = Instant::now();
    let mut segment_s = [0f64; SEGMENTS];
    let mut segment_pages = [0i64; SEGMENTS];
    let mut segment_diag = Vec::new();
    let mut commit_max_ns = 0u64;
    let mut checkpoint_max_ns = 0u64;
    let mut written_in_segment = 0i64;
    let mut segment = 0usize;
    let mut first = 1;
    while first <= pages {
        let last = (first + BACKLOG_PAGES - 1).min(pages);
        let mut batch = WriteBatch::default();
        for id in first..=last {
            let links = (0..LINKS_PER_PAGE)
                .map(|k| hyperlink(spread_target(pages, id, k), u32::try_from(k).unwrap()))
                .collect();
            batch.pages.push(page(id, links));
        }
        let t = Instant::now();
        sink.write(batch).unwrap();
        let spent = t.elapsed().as_secs_f64();
        let seg = segment.min(SEGMENTS - 1);
        segment_s[seg] += spent;
        segment_pages[seg] += last - first + 1;
        written_in_segment += last - first + 1;
        first = last + 1;
        if written_in_segment >= per_segment && segment < SEGMENTS - 1 || first > pages {
            let snap = os_store::sink::diag::snapshot();
            let get = |name: &str| snap.iter().find(|(k, _)| *k == name).map_or(0, |(_, v)| *v);
            commit_max_ns = commit_max_ns.max(get("commit_max_ns"));
            checkpoint_max_ns = checkpoint_max_ns.max(get("checkpoint_max_ns"));
            println!(
                "Abschnitt {:>2}/{SEGMENTS}: {:>8.1} s ({:>6.0} Seiten/s), groesster Commit {:.2} s, groesster Checkpoint {:.2} s",
                seg + 1,
                segment_s[seg],
                written_in_segment as f64 / segment_s[seg],
                get("commit_max_ns") as f64 / 1e9,
                get("checkpoint_max_ns") as f64 / 1e9,
            );
            segment_diag.push(diag_map(snap));
            os_store::sink::diag::reset();
            written_in_segment = 0;
            segment += 1;
        }
    }
    let pages_s: f64 = segment_s.iter().sum();
    let wall_s = wall.elapsed().as_secs_f64();

    // Crawl-Ende: `state = completed` im letzten Batch (ab K-74 baut der Writer hier `links_target`).
    let mut last = WriteBatch::default();
    last.meta.push(("state".into(), "Completed".into()));
    let t_end = Instant::now();
    sink.write(last).unwrap();
    let end_batch_s = t_end.elapsed().as_secs_f64();
    let t_finish = Instant::now();
    sink.finish().unwrap();
    let finish_s = t_finish.elapsed().as_secs_f64();
    drop(sink);

    // Durchsatz je Abschnitt im Verhältnis zum Mittel (die Abschnitte enden an Batch-Grenzen und können bei Seitenzahlen,
    // die kein Vielfaches von 20 000 sind, verschieden viele Seiten haben).
    let mean_rate = pages as f64 / pages_s;
    let rel: Vec<f64> = segment_s
        .iter()
        .zip(segment_pages)
        .map(|(s, n)| {
            if *s > 0.0 {
                (n as f64 / s) / mean_rate
            } else {
                1.0
            }
        })
        .collect();
    let file_bytes = std::fs::metadata(&path).unwrap().len();
    let rss = peak_rss_bytes();
    println!(
        "writer_1m_uniform_segments: {pages} Seiten x {LINKS_PER_PAGE} Links; URLs anlegen {urls_insert_s:.1} s; \
         Seiten schreiben {pages_s:.1} s (Wanduhr {wall_s:.1} s), {:.0} Seiten/s; Abschnitt min/max zum Mittel {:.0} % / {:.0} %; \
         groesster Commit {:.2} s; groesster Checkpoint {:.2} s; Endbatch (Indexaufbau) {end_batch_s:.1} s; finish {finish_s:.1} s; \
         Datei {} MiB; Spitzen-RSS {} MiB",
        pages as f64 / pages_s,
        rel.iter().cloned().fold(f64::MAX, f64::min) * 100.0,
        rel.iter().cloned().fold(0.0, f64::max) * 100.0,
        commit_max_ns as f64 / 1e9,
        checkpoint_max_ns as f64 / 1e9,
        file_bytes / (1024 * 1024),
        rss / (1024 * 1024),
    );

    if let Some(out) = std::env::var_os("OS_BENCH_OUT") {
        let json = serde_json::json!({
            "benchmark": "writer_1m_uniform_segments",
            "schema_version": os_store::SCHEMA_VERSION,
            "pages": pages,
            "links_per_page": LINKS_PER_PAGE,
            "pages_per_transaction": BACKLOG_PAGES,
            "profile": "balanced",
            "urls_insert_s": urls_insert_s,
            "pages_write_s": pages_s,
            "wall_s": wall_s,
            "pages_per_s": pages as f64 / pages_s,
            "segment_s": segment_s,
            "segment_pages": segment_pages,
            "segment_rel_to_mean": rel,
            "segment_diag": segment_diag,
            "commit_max_s": commit_max_ns as f64 / 1e9,
            "checkpoint_max_s": checkpoint_max_ns as f64 / 1e9,
            "end_batch_s": end_batch_s,
            "finish_s": finish_s,
            "file_bytes": file_bytes,
            "peak_rss_bytes": rss,
        });
        std::fs::write(out, serde_json::to_string_pretty(&json).unwrap()).unwrap();
    }

    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    assert_eq!(count(&file, "SELECT COUNT(*) FROM urls"), pages);
    assert_eq!(
        count(&file, "SELECT COUNT(*) FROM links"),
        pages * LINKS_PER_PAGE
    );
    for id in [1, 5, 777, pages / 2, pages] {
        assert_eq!(
            counters::stored(file.conn(), id).unwrap(),
            counters::recompute(file.conn(), id).unwrap(),
            "URL {id}"
        );
    }
    // PERF-003: kein einzelner Commit oder Checkpoint des Writers nähert sich dem Stillstand von 30 s;
    // PERF-001: der Spitzen-RSS des Writers bleibt weit unter 1,5 GB.
    assert!(
        commit_max_ns.max(checkpoint_max_ns) < 30_000_000_000,
        "Commit/Checkpoint über 30 s"
    );
    assert!(
        rss < 1_500_000_000,
        "Spitzen-RSS {} MiB",
        rss / (1024 * 1024)
    );
    // Zusicherung ab Schema v2 (K-74/K-75): jeder Abschnitt zwischen 70 % und 130 % des Mittels.
    if os_store::SCHEMA_VERSION >= 2 && pages >= 1_000_000 {
        for (i, r) in rel.iter().enumerate() {
            assert!(
                (0.7..=1.3).contains(r),
                "Abschnitt {} liegt bei {:.0} % des Mittels",
                i + 1,
                r * 100.0
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// M1-P32: Abfragen nach Linkziel ohne und mit Index `links_target` (K-74)
// ---------------------------------------------------------------------------------------------------------

// spec: PERF-002
#[test]
#[ignore = "Benchmark (P32): links_page In ohne Index bei 250 000 Seiten x 30 Links, Hochrechnung 1 Mio. (OS_BENCH_PAGES, OS_BENCH_DIR)"]
fn inlinks_without_index_250k_pages() {
    let pages: i64 = std::env::var("OS_BENCH_PAGES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(250_000);
    let profile = StorageProfile::Balanced;
    let dir = tempfile::tempdir_in(
        std::env::var_os("OS_BENCH_DIR").unwrap_or_else(|| std::env::temp_dir().into()),
    )
    .unwrap();
    let path = new_crawl(dir.path(), "inlinks.ospider", profile);
    let mut sink = sink(&path, profile);
    let mut first = 1;
    while first <= pages {
        let last = (first + 4_999).min(pages);
        sink.write(batch_with_urls((first..=last).map(|i| (i, true))))
            .unwrap();
        first = last + 1;
    }
    let mut first = 1;
    while first <= pages {
        let last = (first + BACKLOG_PAGES - 1).min(pages);
        let mut batch = WriteBatch::default();
        for id in first..=last {
            let links = (0..LINKS_PER_PAGE)
                .map(|k| hyperlink(spread_target(pages, id, k), u32::try_from(k).unwrap()))
                .collect();
            batch.pages.push(page(id, links));
        }
        sink.write(batch).unwrap();
        first = last + 1;
    }
    sink.finish().unwrap();
    drop(sink);

    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    assert!(!os_store::link_index::present(file.conn()).unwrap());
    let targets = [1, pages / 3, pages / 2, pages - 1];
    let time_inlinks = |file: &CrawlFile| -> Vec<f64> {
        targets
            .iter()
            .map(|&t| {
                let start = Instant::now();
                let page =
                    queries::inlinks(file.conn(), os_model::UrlId(t), PageReq::new(0, 50)).unwrap();
                assert!(page.total > 0);
                start.elapsed().as_secs_f64()
            })
            .collect()
    };
    let _ = time_inlinks(&file); // Kaltstart des Seiten-Caches nicht mitzählen
    let without = time_inlinks(&file);
    drop(file);

    let rw = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    let start = Instant::now();
    assert!(os_store::link_index::ensure(rw.conn()).unwrap());
    let build_s = start.elapsed().as_secs_f64();
    drop(rw);
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    let with = time_inlinks(&file);
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    println!(
        "inlinks_without_index_250k_pages: {pages} Seiten x {LINKS_PER_PAGE} Links; links_page In (50 Zeilen) ohne Index {:.2} s (Mittel aus {} Zielen, einzeln {:?}), \
         mit Index {:.4} s; Indexaufbau {build_s:.1} s; Hochrechnung 1 Mio. Seiten (x{:.1}): ohne Index {:.1} s, Aufbau {:.0} s",
        mean(&without),
        targets.len(),
        without.iter().map(|s| (s * 100.0).round() / 100.0).collect::<Vec<_>>(),
        mean(&with),
        1_000_000.0 / pages as f64,
        mean(&without) * 1_000_000.0 / pages as f64,
        build_s * 1_000_000.0 / pages as f64,
    );
    if let Some(out) = std::env::var_os("OS_BENCH_OUT") {
        let json = serde_json::json!({
            "benchmark": "inlinks_without_index_250k_pages",
            "pages": pages,
            "links": pages * LINKS_PER_PAGE,
            "inlinks_page_without_index_s": without,
            "inlinks_page_without_index_mean_s": mean(&without),
            "inlinks_page_with_index_s": with,
            "index_build_s": build_s,
            "extrapolated_1m_without_index_s": mean(&without) * 1_000_000.0 / pages as f64,
            "extrapolated_1m_index_build_s": build_s * 1_000_000.0 / pages as f64,
        });
        std::fs::write(out, serde_json::to_string_pretty(&json).unwrap()).unwrap();
    }
}
