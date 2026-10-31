//! Tabellen von Schema v3, die kein `WriteBatch`-Teil befüllt (`view_cache`, `serp_edits`, `near_duplicates`,
//! `url_segments`, `api_status`, `api_errors`): Rundreise, Schlüssel und Standardwerte der Spalten. Die übrigen Tabellen
//! prüft `writer_m2_roundtrip.rs` über den Writer. Infrastrukturtest ohne Spec-Tag; nur der letzte Test prüft die
//! `q_`-Sichten der Leseschnittstelle (OUT-233, OUT-234).

mod schema_common;

use os_store::{CrawlFile, OpenMode};
use schema_common::*;

fn open() -> (tempfile::TempDir, CrawlFile) {
    let dir = tempfile::tempdir().unwrap();
    let (_path, file) = create(dir.path(), "v3.ospider");
    (dir, file)
}

fn n(file: &CrawlFile, sql: &str) -> i64 {
    file.conn().query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn view_cache_keeps_an_opaque_blob_per_key_and_parameter_hash() {
    let (_dir, file) = open();
    let c = file.conn();
    c.execute(
        "INSERT INTO view_cache (key, params_hash, step_stamp, built_seq, data) VALUES ('internal:all', 'h1', 's', 7, ?1)",
        [vec![1u8, 2, 3, 255]],
    )
    .unwrap();
    let (stamp, seq, data): (String, i64, Vec<u8>) = c
        .query_row(
            "SELECT step_stamp, built_seq, data FROM view_cache WHERE key = 'internal:all' AND params_hash = 'h1'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!((stamp.as_str(), seq, data), ("s", 7, vec![1, 2, 3, 255]));
    // gleicher Schlüssel, anderer Parameter-Hash: eigene Zeile; gleicher Schlüssel und Hash: Konflikt
    c.execute(
        "INSERT INTO view_cache (key, params_hash, data) VALUES ('internal:all', 'h2', x'00')",
        [],
    )
    .unwrap();
    assert!(c
        .execute(
            "INSERT INTO view_cache (key, params_hash, data) VALUES ('internal:all', 'h2', x'01')",
            [],
        )
        .is_err());
    assert_eq!(n(&file, "SELECT COUNT(*) FROM view_cache"), 2);
    // der Cache ist leer verwerfbar
    c.execute("DELETE FROM view_cache", []).unwrap();
    assert_eq!(n(&file, "SELECT COUNT(*) FROM view_cache"), 0);
}

#[test]
fn serp_edits_has_one_row_per_url_with_a_default_device() {
    let (_dir, file) = open();
    let c = file.conn();
    c.execute(
        "INSERT INTO serp_edits (url_id, title, description, edited_at) VALUES (5, 'T', 'D', '2026-10-17T10:00:00.000Z')",
        [],
    )
    .unwrap();
    let device: String = c
        .query_row("SELECT device FROM serp_edits WHERE url_id = 5", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(device, "desktop");
    assert!(c
        .execute(
            "INSERT INTO serp_edits (url_id, edited_at) VALUES (5, '2026-10-17T10:00:00.000Z')",
            [],
        )
        .is_err());
}

#[test]
fn near_duplicates_and_url_segments_are_unique_per_pair() {
    let (_dir, file) = open();
    let c = file.conn();
    c.execute("INSERT INTO near_duplicates VALUES (1, 2, 0.93)", [])
        .unwrap();
    c.execute("INSERT INTO near_duplicates VALUES (2, 1, 0.93)", [])
        .unwrap();
    assert!(c
        .execute("INSERT INTO near_duplicates VALUES (1, 2, 0.5)", [])
        .is_err());
    let sim: f64 = c
        .query_row(
            "SELECT similarity FROM near_duplicates WHERE url_id = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(sim, 0.93);

    c.execute("INSERT INTO url_segments VALUES (10, 1)", [])
        .unwrap();
    c.execute("INSERT INTO url_segments VALUES (10, 2)", [])
        .unwrap();
    c.execute("INSERT INTO url_segments VALUES (11, 1)", [])
        .unwrap();
    assert!(c
        .execute("INSERT INTO url_segments VALUES (10, 1)", [])
        .is_err());
    // Zugriff in beide Richtungen über den Schlüssel bzw. den Index auf `segment_id`
    assert_eq!(
        n(
            &file,
            "SELECT COUNT(*) FROM url_segments WHERE segment_id = 1"
        ),
        2
    );
    assert_eq!(
        n(&file, "SELECT COUNT(*) FROM url_segments WHERE url_id = 10"),
        2
    );
}

#[test]
fn api_status_and_api_errors_roundtrip() {
    let (_dir, file) = open();
    let c = file.conn();
    c.execute(
        "INSERT INTO api_status (source, state) VALUES ('gsc', 'idle')",
        [],
    )
    .unwrap();
    let (total, done, failed): (i64, i64, i64) = c
        .query_row(
            "SELECT urls_total, urls_done, urls_failed FROM api_status WHERE source = 'gsc'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!((total, done, failed), (0, 0, 0));
    assert!(c
        .execute(
            "INSERT INTO api_status (source, state) VALUES ('gsc', 'idle')",
            []
        )
        .is_err());
    for i in 0..3 {
        c.execute(
            "INSERT INTO api_errors (source, url, code, message, at) VALUES ('gsc', ?1, '429', 'Limit', '2026-10-17T10:00:00.000Z')",
            [format!("https://site.test/{i}")],
        )
        .unwrap();
    }
    assert_eq!(
        n(
            &file,
            "SELECT COUNT(*) FROM api_errors WHERE source = 'gsc'"
        ),
        3
    );
    assert_eq!(n(&file, "SELECT MAX(id) - MIN(id) FROM api_errors"), 2);
}

#[test]
fn sitemap_columns_added_by_v3_default_to_empty_values() {
    let (_dir, file) = open();
    let c = file.conn();
    c.execute(
        "INSERT INTO sitemaps (id, url, kind, depth, found_via) VALUES (1, 'https://site.test/s.xml', 'urlset', 0, 'config')",
        [],
    )
    .unwrap();
    c.execute(
        "INSERT INTO sitemap_entries (sitemap_id, url_id, loc_raw) VALUES (1, 1, '/p1')",
        [],
    )
    .unwrap();
    assert_eq!(
        n(&file, "SELECT COUNT(*) FROM sitemaps WHERE error IS NULL"),
        1
    );
    assert_eq!(
        n(
            &file,
            "SELECT COUNT(*) FROM sitemap_entries WHERE changefreq IS NULL AND priority IS NULL \
             AND images = 0 AND videos = 0 AND news = 0"
        ),
        1
    );
    // Dateien ohne die M2-Spalten gibt es nach der Migration nicht mehr: ein erneutes Öffnen ändert nichts.
    drop(file);
}

// spec: OUT-233
// spec: OUT-234
#[test]
fn a_reopened_v3_file_has_all_tables_and_views_and_the_integrity_check_passes() {
    let dir = tempfile::tempdir().unwrap();
    let (path, file) = create(dir.path(), "v3.ospider");
    drop(file);
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    assert_eq!(tables(&file), ALL_TABLES);
    assert!(file.integrity_check().unwrap());
    let views: Vec<String> = file
        .conn()
        .prepare("SELECT name FROM sqlite_master WHERE type = 'view' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        views,
        [
            "analysis_link_facts",
            "q_columns",
            "q_config",
            "q_links",
            "q_manifest",
            "q_sitemap_urls",
            "q_urls",
            "url_ext"
        ]
    );
}
