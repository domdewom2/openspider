//! Benchmark (M1-N27, K-83): die fünf Duplicate-Filter auf 1 Mio. Zeilen von `urls` **ohne** die fünf Indizes
//! `urls_title1`, `urls_md1`, `urls_h1`, `urls_h2`, `urls_hash`. Nur Bericht (Zeiten je Filter), keine Schranke; läuft
//! nicht im normalen Testlauf (K-81):
//!
//! ```text
//! OS_BENCH_ROWS=1000000 OS_BENCH_DIR=<ordner> cargo test -p os-analyze --release --test bench_duplicates -- --ignored --nocapture
//! ```
//!
//! Die Zeilen sind schmaler als die einer echten Crawl-Datei (nur die Spalten der Filter); je Wert rund 20 % Dubletten.
//! Mit korreliertem `EXISTS` ohne Index wären 20 000 Zeilen 6,8 s (N22), 1 Mio. Zeilen hochgerechnet 4 Stunden (quadratisch).

use std::time::Instant;

use os_analyze::views::{query_view, ViewQuery};
use os_model::time::{IdSource, SeededIds};
use os_model::{CrawlConfig, CrawlMode, Timestamp};
use os_store::{CrawlFile, CrawlInit, StorageProfile};

const FILTERS: [(&str, &str); 5] = [
    ("Page Titles", "Duplicate"),
    ("Meta Description", "Duplicate"),
    ("H1", "Duplicate"),
    ("H2", "Duplicate"),
    ("Content", "Exact Duplicates"),
];

// spec: ONPAGE-143
#[test]
#[ignore = "Benchmark (N27): fünf Duplicate-Filter auf 1 Mio. Zeilen ohne Indizes (OS_BENCH_ROWS, OS_BENCH_DIR, OS_BENCH_OUT)"]
fn five_duplicate_filters_on_one_million_rows_without_the_indexes() {
    let rows: i64 = std::env::var("OS_BENCH_ROWS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1_000_000);
    let dir = tempfile::tempdir_in(
        std::env::var_os("OS_BENCH_DIR").unwrap_or_else(|| std::env::temp_dir().into()),
    )
    .unwrap();
    let at = Timestamp::from_db("2026-10-16T07:35:00.000Z").unwrap();
    let init = CrawlInit {
        crawl_id: SeededIds::new(9).new_crawl_id(at),
        mode: CrawlMode::Spider,
        start_url: Some("https://site.test/".into()),
        list_source: None,
        project: "Projekt".into(),
        name: "N27".into(),
        config: CrawlConfig::default(),
        created_at: at,
    };
    let file = CrawlFile::create(
        &dir.path().join("dup.ospider"),
        &init,
        StorageProfile::Balanced,
    )
    .unwrap();
    let conn = file.conn();
    for index in os_store::url_index::WRITE_PHASE_DROPPED {
        conn.execute_batch(&format!("DROP INDEX IF EXISTS {}", index.name))
            .unwrap();
    }
    let fill = Instant::now();
    conn.execute_batch("BEGIN").unwrap();
    {
        let mut stmt = conn
            .prepare(
                "INSERT INTO urls (id, url, address, origin, is_internal, in_tables, state, discovered_via, \
                 folder_depth, url_length, status_code, content_class, hash_md5, \
                 title_count, title_1, title_1_len, meta_desc_count, meta_desc_1, meta_desc_1_len, \
                 h1_count, h1_1, h1_1_len, h2_count, h2_1, h2_1_len, indexability) \
                 VALUES (?1, ?2, ?2, 'https://site.test', 1, 1, 'done', 'Link', 0, 20, 200, 'HTML', ?3, \
                 1, ?4, 20, 1, ?5, 20, 1, ?6, 20, 1, ?7, 20, 'Indexable')",
            )
            .unwrap();
        // 80 % der Werte kommen einmal vor, die übrigen 20 % in Paaren und Dreiergruppen: Gruppen mit Treffern in
        // allen Filtern. Die Werte sind gegen die Zeilenreihenfolge gemischt (Primzahl-Schritt).
        let distinct = rows * 85 / 100;
        for id in 1..=rows {
            let k = (id.wrapping_mul(104_729)) % distinct;
            stmt.execute(rusqlite::params![
                id,
                format!("https://site.test/p{id}"),
                format!("{k:032x}"),
                format!("Titel {k}"),
                format!("Beschreibung {k}"),
                format!("Überschrift {k}"),
                format!("Zwischenüberschrift {k}"),
            ])
            .unwrap();
        }
    }
    conn.execute_batch("COMMIT").unwrap();
    let fill_s = fill.elapsed().as_secs_f64();
    assert!(!os_store::url_index::present(conn).unwrap());

    let cfg = CrawlConfig::default();
    let mut report = Vec::new();
    for (tab, filter) in FILTERS {
        let mut q = ViewQuery::new(tab, filter);
        q.limit = 50;
        let t = Instant::now();
        let page = query_view(&file, &cfg, &q).unwrap();
        let s = t.elapsed().as_secs_f64();
        println!(
            "{tab}:{filter}: {rows} Zeilen ohne Indizes, {} Treffer, {s:.2} s",
            page.total
        );
        assert!(page.total > 0, "{tab}:{filter}");
        report.push(
            serde_json::json!({ "tab": tab, "filter": filter, "hits": page.total, "seconds": s }),
        );
    }
    println!(
        "befüllt in {fill_s:.1} s, Datei {} MiB",
        std::fs::metadata(dir.path().join("dup.ospider"))
            .unwrap()
            .len()
            / (1024 * 1024)
    );
    if let Some(out) = std::env::var_os("OS_BENCH_OUT") {
        let json = serde_json::json!({
            "benchmark": "five_duplicate_filters_on_one_million_rows_without_the_indexes",
            "rows": rows,
            "filters": report,
        });
        std::fs::write(out, serde_json::to_string_pretty(&json).unwrap()).unwrap();
    }
}
