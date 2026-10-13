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
