//! Speicherobergrenze (PERF-001, K-16, M1-P28): Die Cache-Grenze des Profils „Maximale Geschwindigkeit“ wird über
//! die PRAGMA-Werte einer geöffneten Crawl-Datei geprüft (kein neuer Konfigurationsschlüssel; die Grenze ist eine
//! App-Einstellung des Leistungsprofils): höchstens 25 % des physischen RAM und höchstens 4 GiB. Der Spitzen-RSS
//! eines Crawls mit 10 000 bzw. 1 000 000 Seiten wird von `os-loadtest` (Szenario `perf-001`) im Kindprozess
//! gemessen.

use os_engine::testing::test_deps;
use os_model::CrawlConfig;
use os_store::profile::{self, physical_ram_bytes};
use os_store::{CrawlFile, CrawlInit, StorageProfile};

const GIB: u64 = 1024 * 1024 * 1024;

fn create(dir: &std::path::Path, name: &str, profile: StorageProfile) -> CrawlFile {
    let deps = test_deps();
    let init = CrawlInit {
        crawl_id: deps.ids.new_crawl_id(deps.clock.now()),
        mode: os_model::CrawlMode::Spider,
        start_url: Some("http://site.test/".to_string()),
        list_source: None,
        project: "perf".to_string(),
        name: name.to_string(),
        config: CrawlConfig::default(),
        created_at: deps.clock.now(),
    };
    CrawlFile::create(&dir.join(name), &init, profile).unwrap()
}

// spec: PERF-001
#[test]
fn the_max_speed_cache_is_at_most_a_quarter_of_the_ram_and_4_gib() {
    let dir = tempfile::tempdir().unwrap();
    let file = create(dir.path(), "max.ospider", StorageProfile::MaxSpeed);
    let applied = profile::read(file.conn()).unwrap();
    assert!(
        applied.cache_size < 0,
        "negativ = KiB: {}",
        applied.cache_size
    );
    let bytes = applied.cache_size.unsigned_abs() * 1024;
    assert!(bytes <= 4 * GIB, "{bytes} B > 4 GiB");
    let ram = physical_ram_bytes().expect("RAM ermittelbar");
    assert!(
        bytes <= (ram / 4).max(256 * 1024 * 1024),
        "{bytes} B > 25 % von {ram} B"
    );
    // und genau die Formel von K-16
    assert_eq!(
        applied.cache_size,
        StorageProfile::MaxSpeed.cache_size_kib_for_ram(Some(ram))
    );
    assert_eq!(applied.synchronous_name(), "OFF", "während des Crawls");
}

// spec: PERF-001
#[test]
fn the_other_profiles_stay_far_below_the_cap() {
    let dir = tempfile::tempdir().unwrap();
    for (p, kib) in [
        (StorageProfile::Balanced, 262_144),
        (StorageProfile::MinMemory, 65_536),
    ] {
        let file = create(dir.path(), &format!("{kib}.ospider"), p);
        assert_eq!(profile::read(file.conn()).unwrap().cache_size, -kib);
    }
}
