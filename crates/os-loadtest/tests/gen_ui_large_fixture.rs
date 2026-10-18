//! Store-backed UI fixture tests; no server, clock, renderer or foreign W1 stub.
#[path = "../src/gen_ui/mod.rs"]
mod gen_ui;

use clap::Parser;
use gen_ui::{generate, DEFAULT_ROWS, DEFAULT_SEED, HTML_BYTES, LINKS_PER_PAGE};
use os_model::UrlId;
use os_store::{CrawlFile, OpenMode};

fn scalar(file: &CrawlFile, sql: &str) -> i64 {
    file.conn().query_row(sql, [], |r| r.get(0)).unwrap()
}

// spec: UI-148
#[test]
fn cli_accepts_defaults_and_explicit_seed_and_generates_small_fixture() {
    let defaults =
        gen_ui::cli::Args::try_parse_from(["gen-ui-large", "--output", "ui.ospider"]).unwrap();
    assert_eq!(defaults.rows, DEFAULT_ROWS);
    assert_eq!(defaults.seed, DEFAULT_SEED);
    assert!(gen_ui::cli::Args::try_parse_from(["gen-ui-large"]).is_err());
    assert!(gen_ui::cli::Args::try_parse_from([
        "gen-ui-large",
        "--output",
        "ui.ospider",
        "--seed",
        "bad"
    ])
    .is_err());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("small.ospider");
    let args = gen_ui::cli::Args::try_parse_from([
        "gen-ui-large",
        "--output",
        path.to_str().unwrap(),
        "--rows",
        "22",
        "--seed",
        "18446744073709551615",
    ])
    .unwrap();
    let report = args.run().unwrap();
    assert_eq!(report.seed, u64::MAX);
    assert_eq!(report.links, 440);
    assert!(path.is_file());
}

// spec: UI-148
#[test]
fn hundred_thousand_rows_are_a_complete_queryable_crawl() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("small.ospider");
    let report = generate(&path, 100_000, DEFAULT_SEED, |_, _| {}).unwrap();
    assert_eq!(report.rows, 100_000);
    assert_eq!(report.links, 2_000_000);
    assert_eq!(report.hub_inlinks, 99_999);
    assert_eq!(report.schema_version, os_store::SCHEMA_VERSION);
    assert_eq!(report.html_bytes, HTML_BYTES);
    assert_eq!(report.seed, DEFAULT_SEED);
    // Check before reopening: SQLite can create WAL/SHM files even for a reader.
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    os_store::read_api::check(file.conn()).unwrap();
    assert_eq!(
        scalar(
            &file,
            "SELECT COUNT(*) FROM q_urls WHERE is_internal=1 AND crawled=1"
        ),
        100_000
    );
    assert_eq!(
        scalar(
            &file,
            "SELECT COUNT(*) FROM urls WHERE content_class='HTML'"
        ),
        100_000
    );
    assert_eq!(scalar(&file, "SELECT COUNT(*) FROM links"), 2_000_000);
    assert_eq!(
        scalar(
            &file,
            "SELECT COUNT(*) FROM urls WHERE status_code=404 AND indexability='Non-Indexable'"
        ),
        1_000
    );
    assert_eq!(scalar(&file, "SELECT COUNT(*) FROM links l LEFT JOIN urls s ON s.id=l.source_id LEFT JOIN urls t ON t.id=l.target_id WHERE s.id IS NULL OR t.id IS NULL OR l.source_id=l.target_id OR l.target_raw!=t.url"), 0);
    assert_eq!(scalar(&file, "SELECT COUNT(*) FROM (SELECT source_id FROM links GROUP BY source_id HAVING COUNT(*)!=20 OR COUNT(DISTINCT target_id)!=20)"), 0);
    assert_eq!(
        scalar(&file, "SELECT COUNT(*) FROM links WHERE target_id=1"),
        99_999
    );
    // Independent aggregation over actual persisted edges, for every URL.
    assert_eq!(scalar(&file, "WITH actual AS (SELECT l.target_id, COUNT(*) AS n, COUNT(DISTINCT l.source_id) AS u, COUNT(DISTINCT CASE WHEN s.status_code BETWEEN 200 AND 299 THEN l.source_id END) AS html FROM links l JOIN urls s ON s.id=l.source_id GROUP BY l.target_id) SELECT COUNT(*) FROM url_counters c LEFT JOIN actual a ON a.target_id=c.url_id WHERE c.inlinks!=COALESCE(a.n,0) OR c.unique_inlinks!=COALESCE(a.u,0) OR c.inlinks_all!=COALESCE(a.n,0) OR c.html_2xx_sources!=COALESCE(a.html,0) OR c.self_inlinks!=0 OR c.self_outlinks!=0 OR c.outlinks!=20 OR c.unique_outlinks!=20"), 0);
    assert_eq!(
        scalar(&file, "SELECT SUM(inlinks) FROM url_counters"),
        2_000_000
    );
    assert!(scalar(&file, "SELECT COUNT(*) FROM pragma_table_info('urls')") >= 65);
    // Sort/search cases: NULL and empty are different, Unicode and duplicate keys
    // occur alongside numeric values 2 and 10.
    for predicate in [
        "title_1 IS NULL",
        "title_1=''",
        "title_1 LIKE 'Äpfel 東京 café %'",
        "length(title_1)=300",
        "word_count=2",
        "word_count=10",
        "word_count IS NULL",
        "indexability='Non-Indexable'",
    ] {
        assert!(
            scalar(
                &file,
                &format!("SELECT COUNT(*) FROM urls WHERE {predicate}")
            ) > 0,
            "{predicate}"
        );
    }
    assert!(scalar(&file, "SELECT COUNT(*) FROM (SELECT title_1 FROM urls WHERE title_1 IS NOT NULL GROUP BY title_1 HAVING COUNT(*)>1)") > 0);
    let html = os_store::queries::stored_html(&file, UrlId(2))
        .unwrap()
        .unwrap();
    assert_eq!(html.bytes.len(), HTML_BYTES);
    assert!(html.bytes.starts_with(b"<!doctype html>"));
    assert!(html.bytes.ends_with(b"</body></html>"));
    assert_eq!(
        scalar(&file, "SELECT size_bytes FROM urls WHERE id=2"),
        HTML_BYTES as i64
    );
    let state: String = file
        .conn()
        .query_row(
            "SELECT value FROM q_manifest WHERE key='completeness'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(state, "complete");
    let integrity: String = file
        .conn()
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
    assert_eq!(scalar(&file, "SELECT COUNT(*) FROM sqlite_master WHERE name IN ('links_source','links_target','urls_counters_insert')"), 3);
}

// spec: UI-148
#[test]
fn same_seed_is_byte_identical_and_different_seed_changes_values() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.ospider");
    let b = dir.path().join("b.ospider");
    let c = dir.path().join("c.ospider");
    generate(&a, 64, u64::MAX, |_, _| {}).unwrap();
    generate(&b, 64, u64::MAX, |_, _| {}).unwrap();
    generate(&c, 64, 1, |_, _| {}).unwrap();
    assert_eq!(std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
    let open = |p: &std::path::Path| CrawlFile::open(p, OpenMode::ReadOnly).unwrap();
    let a = open(&a);
    let c = open(&c);
    assert_ne!(
        scalar(&a, "SELECT response_time_ms FROM urls WHERE id=3"),
        scalar(&c, "SELECT response_time_ms FROM urls WHERE id=3")
    );
}

// spec: UI-148
#[test]
fn rejects_invalid_sizes_and_preserves_existing_output() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("existing.ospider");
    for rows in [0, 21, DEFAULT_ROWS + 1, u64::MAX] {
        assert!(generate(&path, rows, DEFAULT_SEED, |_, _| {}).is_err());
        assert!(!path.exists());
    }
    std::fs::write(&path, b"keep me").unwrap();
    assert!(generate(&path, 22, DEFAULT_SEED, |_, _| {}).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"keep me");
}

// spec: UI-148
#[test]
fn publication_race_preserves_winner_and_cleans_staging() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("race.ospider");
    let result = generate(&path, 22, DEFAULT_SEED, |phase, _| {
        if phase == "checkpoint" {
            std::fs::write(&path, b"other process").unwrap();
        }
    });
    assert!(result.is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"other process");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

// spec: UI-148
#[test]
#[ignore = "Benchmark: 5 Mio. URLs / 100 Mio. Links, <= 10 min (K-19)"]
fn five_million_rows_with_million_inlinks_within_ten_minutes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("large.ospider");
    let started = std::time::Instant::now();
    let report = generate(&path, DEFAULT_ROWS, DEFAULT_SEED, |phase, n| {
        if phase != "rows" || n % 100_000 == 0 {
            eprintln!("{phase}: {n}");
        }
    })
    .unwrap();
    eprintln!("{}", serde_json::to_string(&report).unwrap());
    assert!(started.elapsed() <= std::time::Duration::from_secs(600));
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    assert_eq!(
        scalar(&file, "SELECT COUNT(*) FROM urls"),
        DEFAULT_ROWS as i64
    );
    assert_eq!(
        scalar(&file, "SELECT COUNT(*) FROM links"),
        (DEFAULT_ROWS * LINKS_PER_PAGE) as i64
    );
    assert_eq!(
        scalar(&file, "SELECT COUNT(*) FROM links WHERE target_id=1"),
        1_000_000
    );
    assert_eq!(
        scalar(
            &file,
            "SELECT COUNT(DISTINCT source_id) FROM links WHERE target_id=1"
        ),
        1_000_000
    );
    assert_eq!(
        scalar(&file, "SELECT inlinks FROM url_counters WHERE url_id=1"),
        1_000_000
    );
    assert_eq!(
        scalar(
            &file,
            "SELECT html_2xx_sources FROM url_counters WHERE url_id=1"
        ),
        990_000
    );
    for id in [2, 3, 21, 22, 1_000_002, 1_000_003, 5_000_000] {
        assert_eq!(
            scalar(
                &file,
                &format!("SELECT COUNT(*) FROM links WHERE target_id={id}")
            ),
            scalar(
                &file,
                &format!("SELECT inlinks FROM url_counters WHERE url_id={id}")
            ),
            "counter for URL {id}"
        );
    }
    assert_eq!(scalar(&file, "SELECT COUNT(*) FROM sqlite_master WHERE name IN ('urls_title1','urls_md1','urls_h1','urls_h2','urls_hash')"), 0);
}
