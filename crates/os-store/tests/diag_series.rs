//! Messzähler je 100 Batches (M1-N22, PERF-002): feste Eimer, die Aufbau, Commit und Anweisungsgruppen über den Lauf
//! sichtbar machen. Eigene Testdatei: die Zähler sind prozessweit.

mod store_common;

use os_store::sink::diag;
use os_store::{CrawlSink, StorageProfile};
use store_common::*;

fn get(name: &str) -> u64 {
    diag::snapshot()
        .into_iter()
        .find(|(k, _)| *k == name)
        .unwrap_or_else(|| panic!("Zähler {name} fehlt"))
        .1
}

// spec: PERF-002
#[test]
fn batches_fall_into_buckets_of_one_hundred_and_the_groups_add_up_to_the_build_time() {
    diag::reset();
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "diag.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);
    // 1 Batch mit URLs, danach 204 kleine Batches mit je einer Seite
    let mut first = batch_with_urls((1..=300).map(|i| (i, true)));
    first
        .pages
        .push(page(1, vec![hyperlink(2, 0), hyperlink(3, 1)]));
    sink.write(first).unwrap();
    for i in 2..=205 {
        let mut b = os_model::WriteBatch::default();
        b.pages.push(page(i, vec![hyperlink(i + 1, 0)]));
        sink.write(b).unwrap();
    }
    assert_eq!(get("batches"), 205);
    assert_eq!(get("batches_h00"), 100);
    assert_eq!(get("batches_h01"), 100);
    assert_eq!(get("batches_h02"), 5);
    assert_eq!(get("batches_h03"), 0);
    // Eimer addieren sich zu den Gesamtwerten
    let sum = |prefix: &str| {
        (0..16)
            .map(|i| get(&format!("{prefix}_h{i:02}")))
            .sum::<u64>()
    };
    assert_eq!(sum("commit_ns"), get("commit_ns"));
    assert_eq!(sum("build_ns"), get("build_ns"));
    // Aufbau = benannte Gruppen + Rest (der Rest ist der Unterschied zur Gesamtzeit)
    let groups: u64 = [
        "g_urls_ins_ns",
        "g_urls_upd_ns",
        "g_links_ns",
        "g_counters_ns",
        "g_queue_ns",
        "g_rest_ns",
    ]
    .iter()
    .map(|g| sum(g))
    .sum();
    let build = get("build_ns");
    assert!(
        groups >= build && groups <= build + build / 100 + 1,
        "Gruppen {groups} gegen Aufbau {build}"
    );
    assert!(sum("g_urls_ins_ns") > 0 && sum("g_links_ns") > 0 && sum("g_counters_ns") > 0);
    // die Länge des WAL steht in jedem Eimer; die Cache-Zähler laufen
    assert!(get("wal_bytes_max_h00") > 0);
    assert!(get("cache_write") > 0 && get("cache_hit") > 0);
    // das Maximum je Eimer ist höchstens das Gesamtmaximum
    assert!(get("commit_max_ns_h00") <= get("commit_max_ns"));
}

// spec: PERF-002
#[test]
fn the_last_bucket_collects_everything_after_the_fifteenth() {
    assert_eq!(diag::bucket_of(0), 0);
    assert_eq!(diag::bucket_of(99), 0);
    assert_eq!(diag::bucket_of(100), 1);
    assert_eq!(diag::bucket_of(1599), 15);
    assert_eq!(diag::bucket_of(1600), 15);
    assert_eq!(diag::bucket_of(u64::MAX), 15);
}
