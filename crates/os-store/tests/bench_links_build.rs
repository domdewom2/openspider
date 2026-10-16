//! Benchmark (M1-N27, K-84): Aufbau des Index `links_target` auf einer Store-Datei mit 1 Mio. Seiten x 30 gleichverteilten
//! Links, ohne den Index, in verschiedenen Einstellungen (Hilfsthreads des Sortierers, Cache). Nur Bericht, läuft nicht im
//! normalen Testlauf (K-81).
//!
//! Zwei Schritte, jeder in einem eigenen Prozess (der Spitzen-RSS gilt je Prozess):
//!
//! ```text
//! # 1. Vorlage anlegen (dauert rund 5 Minuten bei 1 Mio. Seiten, Datei rund 6 GB)
//! OS_BENCH_MASTER=<ordner>/master.ospider OS_BENCH_PAGES=1000000 \
//!   cargo test -p os-store --release --test bench_links_build -- --ignored --nocapture create_master
//! # 2. je Einstellung eine Kopie der Vorlage, Aufbau messen
//! OS_BENCH_MASTER=<ordner>/master.ospider OS_BENCH_VARIANTS=2:4096,4:4096,4:16384 \
//!   cargo test -p os-store --release --test bench_links_build -- --ignored --nocapture build_variants
//! ```
//!
//! `OS_BENCH_VARIANTS` = Liste `threads:cache_kib` (Standard `2:4096`, der Stand vor N27). Jede Variante kopiert die Vorlage
//! neben sie (`<vorlage>.work`), setzt die PRAGMAs, baut den Index in einer Transaktion samt `COMMIT` und meldet Dauer,
//! Zuwachs des Spitzen-RSS dieses Prozesses und die Block-Ein-/Ausgabe (`getrusage`). Ein kalter Dateicache lässt sich
//! ohne `sudo purge` nicht erzwingen; die Zähler `inblock` zeigen, was wirklich von der Platte kam.

mod store_common;

use std::path::PathBuf;
use std::time::Instant;

use os_model::{LinkPosition, PathType, WriteBatch};
use os_store::{CrawlFile, CrawlSink, OpenMode, StorageProfile};
use store_common::*;

const LINKS_PER_PAGE: i64 = 30;
/// Seiten je Transaktion wie `os_engine::writer_thread::BATCH_PAGES_BACKLOG`.
const BACKLOG_PAGES: i64 = 2_000;

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

fn env_path(name: &str) -> PathBuf {
    std::env::var_os(name)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("{name} fehlt"))
}

/// CPU-Zeit (Benutzer, System) des Prozesses in Sekunden.
#[cfg(unix)]
fn cpu_seconds() -> (f64, f64) {
    // SAFETY: `getrusage` schreibt in eine ausreichend große, nullinitialisierte Struktur.
    let mut u: libc::rusage = unsafe { std::mem::zeroed() };
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut u) } != 0 {
        return (0.0, 0.0);
    }
    let t = |v: libc::timeval| v.tv_sec as f64 + v.tv_usec as f64 / 1e6;
    (t(u.ru_utime), t(u.ru_stime))
}

#[cfg(not(unix))]
fn cpu_seconds() -> (f64, f64) {
    (0.0, 0.0)
}

/// Zähler „Pageins“ aus `vm_stat` (macOS; sonst 0): Seiten, die das System von der Platte eingelesen hat.
fn vm_pageins() -> u64 {
    std::process::Command::new("vm_stat")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|t| {
            t.lines()
                .find(|l| l.starts_with("Pageins"))?
                .split(':')
                .nth(1)?
                .trim()
                .trim_end_matches('.')
                .parse()
                .ok()
        })
        .unwrap_or(0)
}

#[cfg(unix)]
fn usage() -> (u64, u64, u64) {
    // SAFETY: `getrusage` schreibt in eine ausreichend große, nullinitialisierte Struktur.
    let mut u: libc::rusage = unsafe { std::mem::zeroed() };
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut u) } != 0 {
        return (0, 0, 0);
    }
    let rss = u64::try_from(u.ru_maxrss).unwrap_or(0);
    let rss = if cfg!(target_os = "macos") {
        rss
    } else {
        rss * 1024
    };
    (
        rss,
        u64::try_from(u.ru_inblock).unwrap_or(0),
        u64::try_from(u.ru_oublock).unwrap_or(0),
    )
}

#[cfg(not(unix))]
fn usage() -> (u64, u64, u64) {
    (0, 0, 0)
}

#[test]
#[ignore = "Benchmark (N27): Vorlage mit OS_BENCH_PAGES Seiten x 30 Links ohne links_target anlegen (OS_BENCH_MASTER)"]
fn create_master() {
    let pages: i64 = std::env::var("OS_BENCH_PAGES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1_000_000);
    let master = env_path("OS_BENCH_MASTER");
    let dir = master.parent().unwrap();
    let name = master.file_name().unwrap().to_string_lossy().to_string();
    let _ = std::fs::remove_file(&master);
    // breite Zeilen (Standard) oder die schmalen des Writer-Benchmarks (`OS_BENCH_WIDE=0`)
    let wide = std::env::var("OS_BENCH_WIDE").map_or(true, |v| v != "0");
    let profile = StorageProfile::Balanced;
    let path = new_crawl(dir, &name, profile);
    let mut sink = sink(&path, profile);
    let start = Instant::now();
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
                .map(|k| {
                    let mut l = hyperlink(spread_target(pages, id, k), u32::try_from(k).unwrap());
                    if wide {
                        // Zeilenbreite wie in einer echten Crawl-Datei (rund 130 Byte je Link)
                        l.target_raw =
                            format!("https://site.test/seite-{}.html", l.target_id.unwrap().0);
                        l.anchor = Some(format!("Linktext {k} der Seite {id}"));
                        l.link_path = Some(format!("//html/body/main/div[2]/ul/li[{}]/a", k + 1));
                        l.link_position = Some(LinkPosition::Content);
                        l.path_type = Some(PathType::Absolute);
                    }
                    l
                })
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
    assert_eq!(
        count(&file, "SELECT COUNT(*) FROM links"),
        pages * LINKS_PER_PAGE
    );
    println!(
        "Vorlage {}: {pages} Seiten x {LINKS_PER_PAGE} Links in {:.0} s, {} MiB",
        path.display(),
        start.elapsed().as_secs_f64(),
        std::fs::metadata(&path).unwrap().len() / (1024 * 1024)
    );
}

#[test]
#[ignore = "Benchmark (N27): Aufbau von links_target je Einstellung (OS_BENCH_MASTER, OS_BENCH_VARIANTS=threads:cache_kib,...)"]
fn build_variants() {
    let master = env_path("OS_BENCH_MASTER");
    let variants = std::env::var("OS_BENCH_VARIANTS").unwrap_or_else(|_| "2:4096".to_string());
    for variant in variants.split(',').filter(|v| !v.is_empty()) {
        let (threads, cache_kib) = variant.split_once(':').expect("threads:cache_kib");
        let (threads, cache_kib): (i64, i64) =
            (threads.parse().unwrap(), cache_kib.parse().unwrap());
        let work = {
            let mut p = master.clone().into_os_string();
            p.push(".work");
            PathBuf::from(p)
        };
        for suffix in ["", "-wal", "-shm"] {
            let mut p = work.clone().into_os_string();
            p.push(suffix);
            let _ = std::fs::remove_file(PathBuf::from(p));
        }
        std::fs::copy(&master, &work).unwrap();
        // Kalter Dateicache ohne `sudo purge`: eine Füllerdatei (`OS_BENCH_FILLER_MB`, mehr als der RAM) verdrängt die
        // Seiten der Kopie aus dem Cache des Betriebssystems; `pageins` zählt, was der Aufbau danach von der Platte holt.
        if let Some(mb) = std::env::var("OS_BENCH_FILLER_MB")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
        {
            let mut filler = work.clone().into_os_string();
            filler.push(".filler");
            let filler = PathBuf::from(filler);
            {
                use std::io::Write;
                let chunk = vec![0x5Au8; 1024 * 1024];
                let mut f = std::fs::File::create(&filler).unwrap();
                for _ in 0..mb {
                    f.write_all(&chunk).unwrap();
                }
                f.sync_all().unwrap();
            }
            std::fs::remove_file(&filler).unwrap();
        }
        let pageins_before = vm_pageins();
        let file = CrawlFile::open(&work, OpenMode::ReadWrite).unwrap();
        os_store::collation::ensure(file.conn()).unwrap();
        os_store::profile::apply(file.conn(), StorageProfile::Balanced).unwrap();
        let conn = file.conn();
        assert!(!os_store::link_index::present(conn).unwrap());
        let before = usage();
        let cpu_before = cpu_seconds();
        conn.pragma_update(None, "cache_size", -cache_kib).unwrap();
        conn.pragma_update(None, "threads", threads).unwrap();
        let start = Instant::now();
        conn.execute_batch(
            "BEGIN IMMEDIATE; CREATE INDEX links_target ON links(target_id, link_type)",
        )
        .unwrap();
        let built = start.elapsed().as_secs_f64();
        conn.execute_batch("COMMIT").unwrap();
        let total = start.elapsed().as_secs_f64();
        let after = usage();
        let cpu_after = cpu_seconds();
        let pageins = vm_pageins().saturating_sub(pageins_before);
        println!(
            "links_target threads={threads} cache={cache_kib} KiB: Aufbau {built:.1} s, mit Commit {total:.1} s, \
             CPU Benutzer {:.1} s System {:.1} s, \
             Spitzen-RSS {} MiB (Zuwachs {} MiB), inblock {}, oublock {}, Seiteneinlesungen des Systems (vm_stat pageins) {pageins}",
            cpu_after.0 - cpu_before.0,
            cpu_after.1 - cpu_before.1,
            after.0 / (1024 * 1024),
            (after.0.saturating_sub(before.0)) / (1024 * 1024),
            after.1 - before.1,
            after.2 - before.2
        );
        drop(file);
        for suffix in ["", "-wal", "-shm"] {
            let mut p = work.clone().into_os_string();
            p.push(suffix);
            let _ = std::fs::remove_file(PathBuf::from(p));
        }
    }
}
