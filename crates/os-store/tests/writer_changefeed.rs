//! Änderungsfeed (M1-Plan 3.9, P07): `urls.change_seq` und `crawl_meta.change_seq` je Batch.
//! Infrastruktur des Writers, daher ohne eigene Spec-ID.

mod store_common;

use os_model::*;
use os_store::queries;
use os_store::{CrawlFile, CrawlSink, OpenMode, StorageProfile};
use store_common::*;

fn reader(path: &std::path::Path) -> CrawlFile {
    CrawlFile::open(path, OpenMode::ReadOnly).unwrap()
}

#[test]
fn touched_rows_get_the_batch_sequence_untouched_rows_keep_theirs() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "f.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);
    let file = reader(&path);
    assert_eq!(meta_change_seq(&file), 0);

    // Batch 1: sechs URLs (alle neu → 1), eine Seite mit Links (Zielzeilen → 1).
    let mut b1 = batch_with_urls([
        (1, true),
        (2, true),
        (3, true),
        (4, true),
        (5, true),
        (6, false),
    ]);
    b1.pages.push(page(
        1,
        vec![hyperlink(2, 0), hyperlink(3, 1), hyperlink(6, 2)],
    ));
    sink.write(b1).unwrap();
    assert_eq!(meta_change_seq(&file), 1);
    assert_eq!(queries::change_seq(file.conn()).unwrap(), 1);
    for id in 1..=6 {
        assert_eq!(change_seq_of(&file, id), 1, "URL {id}");
    }

    // Batch 2: Seite 4 wird geschrieben (Link auf 5), URL 2 bekommt ein Depth-Update; 1, 3, 6 bleiben unberührt.
    let b2 = WriteBatch {
        url_updates: vec![UrlUpdate::Depth {
            id: UrlId(2),
            depth: 0,
        }],
        pages: vec![page(4, vec![hyperlink(5, 0)])],
        ..WriteBatch::default()
    };
    sink.write(b2).unwrap();
    assert_eq!(meta_change_seq(&file), 2);
    for (id, expected) in [(1, 1), (2, 2), (3, 1), (4, 2), (5, 2), (6, 1)] {
        assert_eq!(change_seq_of(&file, id), expected, "nach Batch 2: URL {id}");
    }

    // Batch 3: replace von Seite 1 mit nur noch einem Link auf 3: Seite 1, alte Ziele (2, 3, 6) und neue
    // Ziele sind berührt; 4 und 5 behalten ihre 2.
    let mut again = page(1, vec![hyperlink(3, 0)]);
    again.replace = true;
    sink.write(WriteBatch {
        pages: vec![again],
        ..WriteBatch::default()
    })
    .unwrap();
    assert_eq!(meta_change_seq(&file), 3);
    for (id, expected) in [(1, 3), (2, 3), (3, 3), (4, 2), (5, 2), (6, 3)] {
        assert_eq!(change_seq_of(&file, id), expected, "nach Batch 3: URL {id}");
    }
    let max_in_urls = count(&file, "SELECT MAX(change_seq) FROM urls");
    assert_eq!(max_in_urls, meta_change_seq(&file));
}

#[test]
fn changed_since_lists_urls_in_feed_order() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "f.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);
    let file = reader(&path);
    sink.write(batch_with_urls([(1, true), (2, true), (3, true)]))
        .unwrap(); // seq 1
    sink.write(WriteBatch {
        url_updates: vec![UrlUpdate::InTables {
            id: UrlId(3),
            value: false,
        }],
        ..WriteBatch::default()
    })
    .unwrap(); // seq 2
    sink.write(WriteBatch {
        url_updates: vec![
            UrlUpdate::InTables {
                id: UrlId(2),
                value: false,
            },
            UrlUpdate::InTables {
                id: UrlId(1),
                value: false,
            },
        ],
        ..WriteBatch::default()
    })
    .unwrap(); // seq 3

    let c = file.conn();
    let ids = |seq, limit| -> Vec<i64> {
        queries::changed_since(c, seq, limit)
            .unwrap()
            .into_iter()
            .map(|u| u.0)
            .collect()
    };
    assert_eq!(ids(0, 100), vec![3, 1, 2]);
    assert_eq!(ids(1, 100), vec![3, 1, 2]);
    assert_eq!(ids(2, 100), vec![1, 2]);
    assert_eq!(ids(3, 100), Vec::<i64>::new());
    assert_eq!(ids(0, 2), vec![3, 1]);
    assert_eq!(queries::change_seq(c).unwrap(), 3);
}
