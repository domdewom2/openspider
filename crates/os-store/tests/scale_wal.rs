mod store_common;

use os_model::{ExtKey, ExtWrite, UrlId, Value, WriteBatch};
use os_store::{CrawlSink, StorageProfile};
use store_common::*;

// spec: PERF-003
#[test]
fn completion_and_phase_checkpoint_release_the_wal() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "wal.ospider", StorageProfile::Balanced);
    let mut writer = sink(&path, StorageProfile::Balanced)
        .with_checkpoint_bounds(128, 256)
        .unwrap();
    for first in (1..=3000).step_by(100) {
        let mut batch = batch_with_urls((first..first + 100).map(|id| (id, true)));
        batch.pages = (first..first + 100)
            .map(|id| page(id, vec![hyperlink(id, 0)]))
            .collect();
        batch.ext = (first..first + 100)
            .map(|id| ExtWrite {
                url: UrlId(id),
                key: ExtKey::from_static("content.text"),
                value: Value::Text("x".repeat(1100)),
            })
            .collect();
        writer.write(batch).unwrap();
    }
    writer
        .write(WriteBatch {
            meta: vec![("state".into(), "Completed".into())],
            ..Default::default()
        })
        .unwrap();
    // Wie die CLI: erst Indexaufbau, dann Abschluss-Checkpoint, danach Analyse.
    writer.checkpoint().unwrap();
    let mut wal = path.as_os_str().to_os_string();
    wal.push("-wal");
    assert_eq!(std::fs::metadata(&wal).map_or(0, |m| m.len()), 0);
    writer
        .write(WriteBatch {
            ext: (1..=3000)
                .map(|id| ExtWrite {
                    url: UrlId(id),
                    key: ExtKey::from_static("linkscore.score"),
                    value: Value::Int(100),
                })
                .collect(),
            ..Default::default()
        })
        .unwrap();
    writer.finish().unwrap();
    assert_eq!(std::fs::metadata(&wal).map_or(0, |m| m.len()), 0);
}
