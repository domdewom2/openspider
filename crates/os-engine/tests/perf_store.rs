//! Benchmarks der Speicherschicht unter Last (M1-P28; nicht Teil des normalen Testlaufs, Regel 2.8):
//!
//! ```text
//! cargo test -p os-engine --release --test perf_store -- --ignored --nocapture
//! ```
//!
//! * CRAWL-127: Öffnen einer Crawl-Datei mit 1 Mio. URLs (erste Tabellenseite) in unter 3 s, „Speichern unter“
//!   (`CrawlFile::save_as`) in unter 60 s.
//! * Writer mit gleichverteilten Zielen (P07: 117 s für 100 000 Seiten bei 200 Seiten je Batch): Bewertung des
//!   Gruppen-Commits, also mehrerer Writer-Batches in einer Transaktion. Ein Gruppen-Commit aus K Batches zu je 200
//!   Seiten ist für `SqliteSink` ein Batch mit 200·K Seiten; gemessen werden 200, 1 000 und 5 000 Seiten je Batch
//!   auf derselben Last.
//!
//! Messwerte (M1-P30, Apple M2, 100 000 Seiten x 30 Links, gleichverteilte Ziele; die Maschine war nicht ruhig,
//! Abweichungen von ±15 % zwischen Läufen sind normal):
//!
//! | Seiten je Transaktion | vorher (Checkpoint alle 1 000 Seiten) | nachher (Schwelle ab 256 MiB WAL) |
//! |---:|---:|---:|
//! | 200 | 62,7 s | 37,5 s (−40 %) |
//! | 1 000 | 35,1 s (−44 % gegenüber 200) | 29,1 s (−22 % gegenüber 200) |
//! | 5 000 | 21,4 s (−66 % gegenüber 200) | 22,9 s (−39 % gegenüber 200) |
//!
//! Der Checkpoint-Aufschub nimmt einen Teil des Gruppen-Commit-Gewinns vorweg; zusammen sinkt die Writer-Zeit von
//! 62,7 s (200 Seiten, Checkpoint alle 1 000 Seiten) auf rund 23–29 s. Die Engine fasst bei Rückstau bis zu
//! `BATCH_PAGES_BACKLOG` (2 000) Seiten zusammen (`writer_thread`).
//!
//! Die Datei baut die Daten selbst (kein Zugriff auf die Hilfen der os-store-Tests).

use std::path::Path;
use std::time::{Duration, Instant};

use os_engine::testing::test_deps;
use os_model::{
    ContentClass, CrawlConfig, CrawlMode, DiscoveryKind, FetchFacts, Indexability, LinkOrigin,
    LinkRow, LinkType, NewUrl, PageWrite, Timestamp, UrlId, UrlState, WriteBatch,
};
use os_store::{CrawlFile, CrawlInit, CrawlSink, OpenMode, SqliteSink, StorageProfile};

fn created_at() -> Timestamp {
    test_deps().clock.now()
}

fn init() -> CrawlInit {
    let deps = test_deps();
    CrawlInit {
        crawl_id: deps.ids.new_crawl_id(deps.clock.now()),
        mode: CrawlMode::Spider,
        start_url: Some("https://site.test/".into()),
        list_source: None,
        project: "perf".into(),
        name: "perf".into(),
        config: CrawlConfig::default(),
        created_at: created_at(),
    }
}

fn new_url(id: i64) -> NewUrl {
    let url = format!("https://site.test/p{id}");
    NewUrl {
        id: UrlId(id),
        address: url.clone(),
        origin: "https://site.test".into(),
        url_length: u32::try_from(url.len()).unwrap(),
        url,
        is_internal: true,
        in_tables: true,
        discovered_via: DiscoveryKind::Link,
        crawl_depth: Some(1),
        folder_depth: 0,
        state: UrlState::Queued,
        not_crawled_reason: None,
    }
}

fn hyperlink(target: i64, position: u32) -> LinkRow {
    LinkRow {
        target_id: Some(UrlId(target)),
        target_raw: format!("/p{target}"),
        link_type: LinkType::Hyperlink,
        position,
        anchor: Some(format!("Anker {position}")),
        alt_text: None,
        follow: true,
        rel: None,
        target_attr: None,
        path_type: None,
        link_path: None,
        link_position: None,
        link_origin: LinkOrigin::Html,
        in_head: Some(false),
    }
}

fn page(id: i64, links: Vec<LinkRow>) -> PageWrite {
    PageWrite {
        url_id: UrlId(id),
        replace: false,
        fetch: FetchFacts {
            crawl_timestamp: created_at(),
            status_code: 200,
            status: "OK".into(),
            content_type: Some("text/html".into()),
            content_class: ContentClass::Html,
            size_bytes: 1000,
            ..FetchFacts::default()
        },
        page: None,
        indexability: Indexability::Indexable,
        indexability_status: None,
        links,
        images: vec![],
        canonicals: vec![],
        directives: vec![],
        refresh: vec![],
        hreflang: vec![],
        pagination: vec![],
        headers: vec![],
        html: None,
    }
}

fn batch_with_urls(ids: impl IntoIterator<Item = i64>) -> WriteBatch {
    WriteBatch {
        new_urls: ids.into_iter().map(new_url).collect(),
        ..WriteBatch::default()
    }
}

fn scalar(file: &CrawlFile, sql: &str) -> i64 {
    file.conn().query_row(sql, [], |r| r.get(0)).unwrap()
}

fn mib(path: &Path) -> u64 {
    std::fs::metadata(path).map_or(0, |m| m.len()) / (1024 * 1024)
}

/// Schreibt `pages` abgerufene Seiten mit je `links` Links auf die ersten `links` Seiten (Navigationslinks) in
/// Batches zu `per_batch` Seiten.
fn build_crawl(path: &Path, pages: i64, links: i64, per_batch: i64) -> Duration {
    CrawlFile::create(path, &init(), StorageProfile::Balanced).unwrap();
    let mut sink = SqliteSink::open(path, StorageProfile::Balanced).unwrap();
    let started = Instant::now();
    let mut first = 1;
    while first <= pages {
        let last = (first + per_batch - 1).min(pages);
        let mut batch = batch_with_urls(first..=last);
        for id in first..=last {
            let l = (0..links)
                .map(|k| hyperlink(k + 1, u32::try_from(k).unwrap()))
                .collect();
            batch.pages.push(page(id, l));
        }
        sink.write(batch).unwrap();
        first = last + 1;
    }
    sink.finish().unwrap();
    started.elapsed()
}

// spec: CRAWL-127
#[test]
#[ignore = "Benchmark (P28): CRAWL-127 Öffnen von 1 Mio. URLs < 3 s, Speichern unter < 60 s"]
fn opening_a_million_urls_takes_under_3_seconds_and_saving_under_60() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("million.ospider");
    let build = build_crawl(&path, 1_000_000, 3, 200);
    println!(
        "CRAWL-127: 1 Mio. abgerufene URLs (3 Links je Seite) geschrieben in {:.0} s, Datei {} MiB",
        build.as_secs_f64(),
        mib(&path)
    );

    // Öffnen: Datei öffnen, Kopfdaten und erste Tabellenseite (100 Zeilen in Entdeckungsreihenfolge)
    let started = Instant::now();
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    let opened = started.elapsed();
    let meta = file.meta().unwrap();
    let rows: Vec<(i64, String, i64)> = file
        .conn()
        .prepare(
            "SELECT id, url, COALESCE(status_code, -1) FROM urls WHERE in_tables = 1 ORDER BY id LIMIT 100",
        )
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let first_page = started.elapsed();
    assert_eq!(rows.len(), 100);
    assert_eq!(rows[0].1, "https://site.test/p1");
    assert!(!meta.state.as_str().is_empty());
    println!(
        "CRAWL-127: Öffnen {:.0} ms, + erste 100 Zeilen {:.0} ms (Ziel < 3000 ms, warmer Dateicache)",
        opened.as_secs_f64() * 1000.0,
        first_page.as_secs_f64() * 1000.0
    );
    assert!(
        first_page < Duration::from_secs(3),
        "Öffnen + erste Tabellenseite dauerten {first_page:?}"
    );

    // Zeilenzahl: nur zur Information (kein Teil des Kriteriums). Ein `COUNT(*)` über `urls` liest die ganze
    // Tabelle; die erste Tabellenseite darf ihn nicht abwarten (die Oberfläche braucht die Gesamtzahl erst später).
    let started = Instant::now();
    let total = scalar(&file, "SELECT COUNT(*) FROM urls WHERE in_tables = 1");
    assert_eq!(total, 1_000_000);
    println!(
        "CRAWL-127: COUNT(*) über urls {:.0} ms (Vollscan, nicht Teil des Kriteriums)",
        started.elapsed().as_secs_f64() * 1000.0
    );

    // Speichern unter
    let copy = dir.path().join("copy.ospider");
    let started = Instant::now();
    file.save_as(&copy).unwrap();
    let saved = started.elapsed();
    println!(
        "CRAWL-127: Speichern unter {:.1} s (Ziel < 60 s), Kopie {} MiB",
        saved.as_secs_f64(),
        mib(&copy)
    );
    assert!(
        saved < Duration::from_secs(60),
        "Speichern dauerte {saved:?}"
    );
    let reopened = CrawlFile::open(&copy, OpenMode::ReadOnly).unwrap();
    assert_eq!(scalar(&reopened, "SELECT COUNT(*) FROM urls"), 1_000_000);
}

/// Gleichverteilte Ziele: Jede Seite verlinkt 30 verschiedene Zeilen quer über die ganze Site (ungünstigster
/// Fall für Schreibverstärkung, wie `bench_writer.rs` in os-store).
fn uniform_run(pages: i64, links: i64, per_batch: i64) -> Duration {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("uniform.ospider");
    CrawlFile::create(&path, &init(), StorageProfile::Balanced).unwrap();
    let mut sink = SqliteSink::open(&path, StorageProfile::Balanced).unwrap();
    let started = Instant::now();
    // alle Ziele zuerst anlegen
    let mut first = 1;
    while first <= pages {
        let last = (first + 4_999).min(pages);
        sink.write(batch_with_urls(first..=last)).unwrap();
        first = last + 1;
    }
    let mut first = 1;
    while first <= pages {
        let last = (first + per_batch - 1).min(pages);
        let mut batch = WriteBatch::default();
        for id in first..=last {
            let l = (0..links)
                .map(|k| hyperlink((id * 31 + k * 977) % pages + 1, u32::try_from(k).unwrap()))
                .collect();
            batch.pages.push(page(id, l));
        }
        sink.write(batch).unwrap();
        first = last + 1;
    }
    sink.finish().unwrap();
    let elapsed = started.elapsed();
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    assert_eq!(scalar(&file, "SELECT COUNT(*) FROM links"), pages * links);
    println!(
        "Writer, gleichverteilte Ziele: {pages} Seiten x {links} Links, {per_batch} Seiten je Batch: {:.1} s ({:.0} Seiten/s), Datei {} MiB",
        elapsed.as_secs_f64(),
        pages as f64 / elapsed.as_secs_f64(),
        mib(&path)
    );
    elapsed
}

// spec: CRAWL-124
#[test]
#[ignore = "Benchmark (P28): Writer mit gleichverteilten Zielen, Gruppen-Commit (200 / 1 000 / 5 000 Seiten je Batch)"]
fn group_commit_versus_small_batches_with_uniform_targets() {
    let results: Vec<(i64, Duration)> = [200, 1_000, 5_000]
        .into_iter()
        .map(|per_batch| (per_batch, uniform_run(100_000, 30, per_batch)))
        .collect();
    let base = results[0].1.as_secs_f64();
    for (per_batch, d) in &results {
        println!(
            "Gruppen-Commit {per_batch:>5} Seiten je Transaktion: {:.1} s ({:+.0} % gegenüber 200)",
            d.as_secs_f64(),
            (d.as_secs_f64() / base - 1.0) * 100.0
        );
    }
}
