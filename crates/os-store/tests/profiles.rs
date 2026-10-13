//! Leistungsprofile, Speicherplatz und Determinismus (CRAWL-124).

mod store_common;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use os_model::time::FixedClock;
use os_model::{Timestamp, UrlId};
use os_store::files::{
    classify_free_space, disk_space_status, location_warnings, DiskLevel, DiskSpaceStatus,
    LocationWarning, DISK_PAUSE_BELOW_BYTES, DISK_WARN_BELOW_BYTES,
};
use os_store::profile::{self, AppliedPragmas};
use os_store::{queries, CrawlFile, CrawlSink, OpenMode, SqliteSink, StorageProfile};
use store_common::*;

const GIB: u64 = 1024 * 1024 * 1024;

fn pragmas_of_sink(path: &std::path::Path, p: StorageProfile) -> (AppliedPragmas, SqliteSink) {
    let sink = SqliteSink::open(path, p).unwrap();
    (profile::read(sink.file().conn()).unwrap(), sink)
}

// spec: CRAWL-124
#[test]
fn balanced_profile_sets_256_mib_cache_and_synchronous_normal() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "b.ospider", StorageProfile::Balanced);
    let (p, _sink) = pragmas_of_sink(&path, StorageProfile::Balanced);
    assert_eq!(p.cache_size, -262_144);
    assert_eq!(p.synchronous_name(), "NORMAL");
    assert_eq!(p.journal_mode, "wal");
    assert!(!p.foreign_keys);
}

// spec: CRAWL-124
#[test]
fn min_memory_profile_sets_64_mib_cache_and_synchronous_normal() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "m.ospider", StorageProfile::MinMemory);
    let (p, _sink) = pragmas_of_sink(&path, StorageProfile::MinMemory);
    assert_eq!(p.cache_size, -65_536);
    assert_eq!(p.synchronous_name(), "NORMAL");
}

// spec: CRAWL-124
#[test]
fn max_speed_cache_is_a_quarter_of_ram_capped_at_4_gib() {
    let kib = |ram| StorageProfile::MaxSpeed.cache_size_kib_for_ram(Some(ram));
    assert_eq!(kib(8 * GIB), -2_097_152, "25 % von 8 GiB = 2 GiB");
    assert_eq!(kib(4 * GIB), -1_048_576);
    assert_eq!(kib(16 * GIB), -4_194_304, "25 % von 16 GiB = genau 4 GiB");
    assert_eq!(kib(64 * GIB), -4_194_304, "höchstens 4 GiB");
    assert_eq!(kib(GIB / 2), -262_144, "nie kleiner als „Ausgewogen“");
    assert_eq!(
        StorageProfile::MaxSpeed.cache_size_kib_for_ram(None),
        -262_144
    );
    // Die übrigen Profile hängen nicht vom RAM ab.
    assert_eq!(
        StorageProfile::Balanced.cache_size_kib_for_ram(Some(64 * GIB)),
        -262_144
    );
    assert_eq!(
        StorageProfile::MinMemory.cache_size_kib_for_ram(Some(64 * GIB)),
        -65_536
    );
}

// spec: CRAWL-124
#[test]
fn max_speed_profile_uses_synchronous_off_during_the_crawl_and_normal_afterwards() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "x.ospider", StorageProfile::MaxSpeed);
    // Die Anlegeverbindung hat bereits die Werte des Profils.
    let created = CrawlFile::create(
        &dir.path().join("y.ospider"),
        &init(3),
        StorageProfile::MaxSpeed,
    )
    .unwrap();
    assert_eq!(
        profile::read(created.conn()).unwrap().synchronous_name(),
        "OFF"
    );

    let (p, mut sink) = pragmas_of_sink(&path, StorageProfile::MaxSpeed);
    let ram = profile::physical_ram_bytes().expect("RAM ermittelbar");
    assert_eq!(
        p.cache_size,
        StorageProfile::MaxSpeed.cache_size_kib_for_ram(Some(ram))
    );
    assert!(p.cache_size <= -262_144 && p.cache_size >= -4_194_304);
    assert_eq!(p.synchronous_name(), "OFF", "während des Crawls");
    sink.write(batch_with_urls([(1, true)])).unwrap();
    assert_eq!(
        profile::read(sink.file().conn())
            .unwrap()
            .synchronous_name(),
        "OFF"
    );
    sink.checkpoint().unwrap();
    assert_eq!(
        profile::read(sink.file().conn())
            .unwrap()
            .synchronous_name(),
        "OFF"
    );
    sink.finish().unwrap();
    assert_eq!(
        profile::read(sink.file().conn())
            .unwrap()
            .synchronous_name(),
        "NORMAL",
        "nach Abschluss"
    );
}

// spec: CRAWL-124
#[test]
fn every_profile_supports_writing_saving_resuming_and_queries() {
    for p in [
        StorageProfile::Balanced,
        StorageProfile::MaxSpeed,
        StorageProfile::MinMemory,
    ] {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("p.ospider");
        CrawlFile::create(&source, &init(1), p).unwrap();
        let mut sink = SqliteSink::open(&source, p).unwrap();
        let mut batch = batch_with_urls([(1, true), (2, true), (3, true)]);
        batch
            .pages
            .push(page(1, vec![hyperlink(2, 0), hyperlink(3, 1)]));
        batch.queue = vec![os_model::QueueOp::Push {
            id: UrlId(3),
            seq: 3,
            depth: Some(1),
            priority: 0,
        }];
        sink.write(batch).unwrap();
        sink.checkpoint().unwrap();

        let copy = dir.path().join("kopie.ospider");
        sink.file().save_as(&copy).unwrap();
        let opened = CrawlFile::open(&copy, OpenMode::ReadOnly).unwrap();
        assert!(opened.integrity_check().unwrap(), "{p:?}");
        assert_eq!(opened.resume_state().unwrap().queue.len(), 1, "{p:?}");
        let inlinks = queries::inlinks(
            opened.conn(),
            UrlId(2),
            os_store::queries::PageReq::default(),
        )
        .unwrap();
        assert_eq!(inlinks.total, 1, "{p:?}");
        sink.finish().unwrap();
    }
}

fn build_deterministic(dir: &std::path::Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    let clock = Arc::new(FixedClock::new(
        Timestamp::from_db("2026-10-13T09:00:00.000Z").unwrap(),
        Duration::from_secs(1),
    ));
    CrawlFile::create(&path, &init(99), StorageProfile::Balanced).unwrap();
    let mut sink = SqliteSink::open(&path, StorageProfile::Balanced)
        .unwrap()
        .with_clock(clock);
    for round in 0..3 {
        let first = round * 20 + 1;
        let mut batch = batch_with_urls((first..first + 20).map(|i| (i, true)));
        for i in first..first + 20 {
            batch.pages.push(page(
                i,
                vec![hyperlink(i % 17 + 1, 0), hyperlink((i * 7) % 29 + 1, 1)],
            ));
        }
        batch.meta = vec![(
            "heartbeat_at".into(),
            format!("2026-10-13T09:00:0{round}.000Z"),
        )];
        sink.write(batch).unwrap();
    }
    sink.finish().unwrap();
    path
}

// spec: CRAWL-124
#[test]
fn two_identical_crawls_produce_byte_identical_files() {
    let a_dir = tempfile::tempdir().unwrap();
    let b_dir = tempfile::tempdir().unwrap();
    let a = build_deterministic(a_dir.path(), "a.ospider");
    let b = build_deterministic(b_dir.path(), "b.ospider");
    // Nach dem abschließenden WAL-Checkpoint (finish) und dem Schließen der Verbindungen.
    assert!(!std::path::Path::new(&format!("{}-wal", a.display())).exists());
    let (bytes_a, bytes_b) = (std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
    assert_eq!(bytes_a.len(), bytes_b.len());
    assert!(bytes_a == bytes_b, "Crawl-Dateien weichen voneinander ab");
}

// spec: CRAWL-124
#[test]
fn disk_space_thresholds_warn_below_2_gib_and_pause_below_1_gib() {
    assert_eq!(DISK_WARN_BELOW_BYTES, 2 * GIB);
    assert_eq!(DISK_PAUSE_BELOW_BYTES, GIB);
    assert_eq!(classify_free_space(500 * GIB), DiskLevel::Ok);
    assert_eq!(classify_free_space(2 * GIB), DiskLevel::Ok);
    assert_eq!(classify_free_space(2 * GIB - 1), DiskLevel::Low);
    assert_eq!(classify_free_space(GIB), DiskLevel::Low);
    assert_eq!(classify_free_space(GIB - 1), DiskLevel::Critical);
    assert_eq!(classify_free_space(0), DiskLevel::Critical);

    let status = |free| DiskSpaceStatus {
        checked: PathBuf::from("."),
        free_bytes: free,
        level: classify_free_space(free),
    };
    assert!(status(GIB + GIB / 2).warn_at_start() && !status(GIB + GIB / 2).should_pause());
    assert!(status(GIB / 2).warn_at_start() && status(GIB / 2).should_pause());
    assert!(!status(3 * GIB).warn_at_start() && !status(3 * GIB).should_pause());
    assert!((status(GIB + GIB / 2).free_gib() - 1.5).abs() < 1e-9);
}

// spec: CRAWL-124
#[test]
fn disk_space_status_reads_the_drive_of_the_storage_location() {
    let dir = tempfile::tempdir().unwrap();
    let status = disk_space_status(dir.path()).unwrap();
    assert!(status.free_bytes > 0);
    assert_eq!(status.checked, dir.path());
    // Ein noch nicht angelegter Crawl-Ordner wird über den nächsten vorhandenen Ordner geprüft.
    let future = dir.path().join("noch").join("nicht").join("da");
    let status = disk_space_status(&future).unwrap();
    assert_eq!(status.checked, dir.path());
    assert_eq!(status.level, classify_free_space(status.free_bytes));
}

// spec: CRAWL-124
#[test]
fn sync_folders_are_flagged_as_storage_location() {
    let flagged = |p: &str| location_warnings(std::path::Path::new(p));
    assert_eq!(
        flagged("/Users/x/Dropbox/crawls"),
        vec![LocationWarning::SyncFolder("Dropbox")]
    );
    assert_eq!(
        flagged("/Users/x/OneDrive - Firma/OpenSpider"),
        vec![LocationWarning::SyncFolder("OneDrive")]
    );
    assert_eq!(
        flagged("/Users/x/Library/Mobile Documents/com~apple~CloudDocs/c"),
        vec![LocationWarning::SyncFolder("iCloud Drive")]
    );
    assert_eq!(
        flagged("/Users/x/Google Drive/Crawls"),
        vec![LocationWarning::SyncFolder("Google Drive")]
    );
    assert!(
        !flagged("/Users/x/Library/Application Support/OpenSpider/crawls")
            .iter()
            .any(|w| matches!(w, LocationWarning::SyncFolder(_)))
    );
}
