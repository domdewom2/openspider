mod store_common;

use os_model::{ExtKey, ExtWrite, UrlId, Value, WriteBatch};
use os_store::{CrawlFile, CrawlSink, OpenMode, StorageProfile};
use store_common::*;

fn measure(n: i64) -> (f64, f64) {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "scale.ospider", StorageProfile::Balanced);
    let mut writer = sink(&path, StorageProfile::Balanced);
    for first in (1..=n).step_by(500) {
        let end = (first + 500).min(n + 1);
        let mut batch = batch_with_urls((first..end).map(|id| (id, true)));
        for id in first..end {
            let links = (0..30)
                .map(|position| hyperlink((id + i64::from(position)) % n + 1, position))
                .collect();
            batch.pages.push(page(id, links));
            for key in 0..24 {
                batch.ext.push(ExtWrite {
                    url: UrlId(id),
                    key: ExtKey::parse(&format!("head.scale_{key}")).unwrap(),
                    value: Value::Int(id % 100 + key),
                });
            }
            // Die 1-KiB-Grenze des WITHOUT-ROWID-Baums ist der reale Regressionstreiber.
            for key in ["content.text", "content.soft404_text"] {
                batch.ext.push(ExtWrite {
                    url: UrlId(id),
                    key: ExtKey::parse(key).unwrap(),
                    value: Value::Text("text ".repeat(220)),
                });
            }
        }
        writer.write(batch).unwrap();
    }
    writer
        .write(WriteBatch {
            meta: vec![("state".into(), "Completed".into())],
            ..Default::default()
        })
        .unwrap();
    writer.finish().unwrap();
    drop(writer);
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    assert_eq!(count(&file, "SELECT count(*) FROM urls"), n);
    assert_eq!(count(&file, "SELECT count(*) FROM url_ext"), n * 26);
    let unused: f64 = file
        .conn()
        .query_row("SELECT sum(unused) FROM dbstat", [], |r| r.get(0))
        .unwrap();
    let free: f64 = file
        .conn()
        .pragma_query_value(None, "freelist_count", |r| r.get(0))
        .unwrap();
    let page_size: f64 = file
        .conn()
        .pragma_query_value(None, "page_size", |r| r.get(0))
        .unwrap();
    let bytes = std::fs::metadata(path).unwrap().len() as f64;
    (bytes / n as f64, (unused + free * page_size) / bytes)
}

// spec: PERF-001 [M2]
#[test]
fn bytes_per_url_and_unused_space_stay_bounded_from_ten_to_fifty_thousand() {
    let small = measure(10_000);
    let large = measure(50_000);
    eprintln!(
        "scale storage: 10k={small:?}, 50k={large:?}, ratio={}",
        large.0 / small.0
    );
    // OS-Festlegung K2-74: 30 % tolerieren Varint-Breiten und zusätzliche Baumebenen.
    // 15 % inklusive Freiliste lassen normale Blattfüllung zu, aber keine fast leeren
    // Overflow-Seiten je 1-KiB-Text. Kein VACUUM: gemessen wird der echte Writer-Stand.
    assert!(
        large.0 / small.0 <= 1.3,
        "Bytes/URL: {small:?} -> {large:?}"
    );
    assert!(
        small.1 <= 0.15 && large.1 <= 0.15,
        "ungenutzter Raum: {small:?} -> {large:?}"
    );
}
