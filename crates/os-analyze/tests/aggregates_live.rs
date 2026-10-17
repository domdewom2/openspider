#[path = "aggregates_fixture.rs"]
mod fixture;
use os_analyze::aggregates::{self, live};
use os_model::{CrawlConfig, UrlId};
use os_store::{CrawlFile, OpenMode};

// spec: UI-066
#[test]
fn depth_and_directory_fixture_has_all_view_dimensions() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture::create(dir.path());
    fixture::depth_fixture(&file);
    fixture::insert(
        &file,
        20,
        "https://site.test/a/image.png",
        Some(3),
        false,
        true,
        Some(200),
        Some(800),
    );
    fixture::insert(
        &file,
        21,
        "https://outside.test/a/",
        Some(2),
        true,
        false,
        Some(200),
        Some(800),
    );
    live::refresh(&file, &CrawlConfig::default()).unwrap();
    let rows = aggregates::depths(&file).unwrap();
    assert_eq!(rows.len(), 10);
    assert_eq!(rows[9].1.count, 3);
    assert_eq!(rows[2].1.non_indexable, 1);
    assert_eq!(rows[2].1.count, 2); // HTML + Bild
    assert_eq!(rows[3].1.response_codes["4xx"], 1);
    assert_eq!(rows.iter().map(|(_, c)| c.count).sum::<u64>(), 13);
    for (_, c) in &rows {
        assert_eq!(c.response_codes.values().sum::<u64>(), c.count);
    }
    let root = aggregates::paths(&file, "", 0, 10).unwrap();
    assert_eq!(root.len(), 1);
    assert_eq!(root[0].path, "https://site.test/");
    assert_eq!(root[0].counts.count, 14);
    assert_eq!(root[0].counts.indexable + root[0].counts.non_indexable, 13);
    let a = aggregates::paths(&file, &root[0].path, 0, 10).unwrap();
    assert_eq!(a[0].path, "https://site.test/a/");
    assert_eq!(a[0].counts.count, 14);
    let b = aggregates::paths(&file, &a[0].path, 0, 10).unwrap();
    assert_eq!(b[0].path, "https://site.test/a/b/");
    assert_eq!(b[0].counts.count, 13);
    assert!(!b[0].has_children);
}

// spec: UI-069
#[test]
fn response_time_boundaries_scopes_and_no_response() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture::create(dir.path());
    for (i, ms) in [
        0, 200, 800, 999, 1000, 1500, 2500, 4000, 7000, 9999, 10000, 11000,
    ]
    .into_iter()
    .enumerate()
    {
        fixture::insert(
            &file,
            i as i64 + 1,
            &format!("https://site.test/{i}"),
            Some(1),
            true,
            true,
            Some(200),
            Some(ms),
        );
    }
    fixture::insert(
        &file,
        20,
        "https://outside.test/",
        None,
        false,
        false,
        Some(301),
        Some(1000),
    );
    fixture::insert(
        &file,
        21,
        "https://site.test/timeout",
        Some(1),
        true,
        true,
        None,
        None,
    );
    fixture::insert(
        &file,
        22,
        "https://site.test/blocked",
        Some(1),
        true,
        true,
        None,
        None,
    );
    file.conn()
        .execute("UPDATE urls SET robots_blocked=1 WHERE id=22", [])
        .unwrap();
    fixture::insert(
        &file,
        23,
        "https://site.test/queued",
        Some(1),
        true,
        true,
        None,
        None,
    );
    file.conn()
        .execute("UPDATE urls SET state='queued' WHERE id=23", [])
        .unwrap();
    live::refresh(&file, &CrawlConfig::default()).unwrap();
    let (b, n) = aggregates::response_times(&file, false).unwrap();
    assert_eq!(b, [4, 3, 1, 0, 1, 0, 0, 1, 0, 1, 2]);
    assert_eq!(n, 1);
    let (b, n) = aggregates::response_times(&file, true).unwrap();
    assert_eq!(b, [4, 2, 1, 0, 1, 0, 0, 1, 0, 1, 2]);
    assert_eq!(n, 1);
}

// spec: UI-145
#[test]
fn updates_retract_old_contributions_and_deletions_do_not_double_count() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture::create(dir.path());
    fixture::depth_fixture(&file);
    live::refresh(&file, &CrawlConfig::default()).unwrap();
    file.conn().execute("UPDATE urls SET response_time_ms=11000,crawl_depth=2,indexability='Non-Indexable',status_code=500,change_seq=2 WHERE id=13",[]).unwrap();
    file.set_meta("change_seq", "2").unwrap();
    live::apply_changes(&file, &CrawlConfig::default(), &[UrlId(13), UrlId(13)]).unwrap();
    live::refresh(&file, &CrawlConfig::default()).unwrap();
    let depths = aggregates::depths(&file).unwrap();
    assert_eq!(depths[1].1.count, 2);
    assert_eq!(depths[1].1.response_codes["5xx"], 1);
    assert_eq!(depths[9].1.count, 2);
    assert_eq!(aggregates::response_times(&file, false).unwrap().0[10], 1);
    file.conn()
        .execute("DELETE FROM urls WHERE id IN(12,13)", [])
        .unwrap();
    file.set_meta("change_seq", "3").unwrap();
    file.set_meta("reset_seq", "3").unwrap();
    live::refresh(&file, &CrawlConfig::default()).unwrap();
    assert_eq!(
        aggregates::paths(&file, "", 0, 1).unwrap()[0].counts.count,
        11
    );
    assert_eq!(aggregates::response_times(&file, false).unwrap().0[10], 0);
    live::apply_changes(&file, &CrawlConfig::default(), &[UrlId(12), UrlId(13)]).unwrap();
    assert_eq!(
        aggregates::paths(&file, "", 0, 1).unwrap()[0].counts.count,
        11
    );
}

// spec: UI-145
#[test]
fn cache_is_readonly_safe_and_warm_reads_do_not_scan_urls() {
    let dir = tempfile::tempdir().unwrap();
    let writable = fixture::create(dir.path());
    fixture::depth_fixture(&writable);
    let file = CrawlFile::open(writable.path(), OpenMode::ReadOnly).unwrap();
    live::refresh(&file, &CrawlConfig::default()).unwrap();
    // Keine SQL-Abfrage auf urls bei unverändertem Feed: selbst eine fehlende
    // Quelltabelle beeinflusst einen bereits vorberechneten Cache nicht.
    writable
        .conn()
        .execute("ALTER TABLE urls RENAME TO source_urls", [])
        .unwrap();
    live::refresh(&file, &CrawlConfig::default()).unwrap();
    assert_eq!(aggregates::depths(&file).unwrap()[9].1.count, 3);
    assert_eq!(aggregates::response_times(&file, false).unwrap().0[0], 13);
}

// spec: UI-066
#[test]
fn path_pages_are_bounded_and_empty_directories_disappear() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture::create(dir.path());
    for i in 0..510 {
        fixture::insert(
            &file,
            i + 1,
            &format!("https://site.test/d{i:04}/x.html"),
            Some(1),
            true,
            true,
            Some(200),
            Some(5),
        );
    }
    live::refresh(&file, &CrawlConfig::default()).unwrap();
    let first = aggregates::paths(&file, "https://site.test/", 0, 3).unwrap();
    let second = aggregates::paths(&file, "https://site.test/", 3, 3).unwrap();
    assert_eq!(first[2].path, "https://site.test/d0002/");
    assert_eq!(second[0].path, "https://site.test/d0003/");
    let tree = aggregates::path_tree(&file, "", 0, 500, 4).unwrap();
    assert_eq!(tree[0].children.len(), 499);
    assert!(tree[0].has_more_children);
    file.conn()
        .execute("DELETE FROM urls WHERE id=1", [])
        .unwrap();
    live::apply_changes(&file, &CrawlConfig::default(), &[UrlId(1)]).unwrap();
    assert_eq!(
        aggregates::paths(&file, "https://site.test/", 0, 1).unwrap()[0].path,
        "https://site.test/d0001/"
    );
}

// spec: UI-145
#[test]
fn failed_refresh_rolls_back_contributions_and_watermark() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture::create(dir.path());
    fixture::depth_fixture(&file);
    live::refresh(&file, &CrawlConfig::default()).unwrap();
    file.conn()
        .execute(
            "UPDATE urls SET response_time_ms=11000,change_seq=2 WHERE id=1",
            [],
        )
        .unwrap();
    file.conn()
        .execute(
            "UPDATE urls SET response_time_ms=-1,change_seq=2 WHERE id=2",
            [],
        )
        .unwrap();
    file.set_meta("change_seq", "2").unwrap();
    assert!(live::refresh(&file, &CrawlConfig::default()).is_err());
    assert_eq!(aggregates::response_times(&file, false).unwrap().0[10], 0);
    file.conn()
        .execute("UPDATE urls SET response_time_ms=1000 WHERE id=2", [])
        .unwrap();
    live::refresh(&file, &CrawlConfig::default()).unwrap();
    assert_eq!(aggregates::response_times(&file, false).unwrap().0[10], 1);
    assert_eq!(aggregates::response_times(&file, false).unwrap().0[1], 1);
}
