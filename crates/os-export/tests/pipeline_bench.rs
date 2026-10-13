//! Messung der Export-Pipeline (OUT-001): 1 000 000 Zeilen × 60 Spalten erhöhen den Speicherbedarf des
//! Prozesses um höchstens 300 MB. `#[ignore]`-Benchmark, wird in P28 ausgeführt:
//!
//! ```text
//! cargo test -p os-export --release --test pipeline_bench -- --ignored --nocapture
//! ```
//!
//! `OS_BENCH_ROWS` verkleinert die Zeilenzahl für einen Probelauf (Standard 1 000 000).

#[path = "links_support.rs"]
mod support;

use std::time::Instant;

use os_analyze::views::{ColumnSelection, ViewQuery};
use os_analyze::{ColumnDef, ColumnKind, ColumnSql, FilterDef, FilterKind, TabDef};
use os_export::tab_export::export_tab_in;
use os_export::Format;
use os_model::CrawlConfig;
use support::*;

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// Tab mit 60 Spalten (Zahlen und kurze Texte aus der URL-Zeile).
fn wide_tab() -> TabDef {
    let mut columns = vec![ColumnDef {
        key: "Address",
        sql: ColumnSql::Expr("u.address"),
        kind: ColumnKind::Url,
        default_visible: true,
        extension: false,
    }];
    for i in 1..60 {
        let (sql, kind) = match i % 4 {
            0 => (leak(format!("u.id + {i}")), ColumnKind::Int),
            1 => (leak(format!("'t{i}-' || (u.id % 1000)")), ColumnKind::Text),
            2 => (leak(format!("u.id % {}", i + 7)), ColumnKind::Int),
            _ => (
                leak(format!("(u.id % {}) / 7.0", i + 11)),
                ColumnKind::Float { decimals: 2 },
            ),
        };
        columns.push(ColumnDef {
            key: leak(format!("Spalte {i}")),
            sql: ColumnSql::Expr(sql),
            kind,
            default_visible: true,
            extension: false,
        });
    }
    TabDef {
        id: "Breit",
        population: "1",
        columns,
        filters: vec![FilterDef {
            id: "All",
            predicate: "1",
            kind: FilterKind::Live,
            requires: Vec::new(),
            ignore_non_indexable: false,
        }],
        default_sort: Vec::new(),
        params: |_| Vec::new(),
    }
}

#[test]
#[ignore = "Messung, ausgeführt in P28"]
fn one_million_rows_by_sixty_columns_stay_within_300_mb_of_extra_memory() {
    let rows: u64 = std::env::var("OS_BENCH_ROWS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1_000_000);
    let dir = tempfile::tempdir().unwrap();
    let file = new_file(dir.path(), "bench.ospider");
    let t = Instant::now();
    exec(
        &file,
        &format!(
            "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < {rows}) \
             INSERT INTO urls (id, url, address, origin, is_internal, state, discovered_via, folder_depth, url_length) \
             SELECT i, 'https://site.test/p' || i, 'https://site.test/p' || i, 'https://site.test', 1, 'done', 'link', 0, 20 FROM n"
        ),
    );
    println!("{rows} URLs angelegt in {:?}", t.elapsed());

    let mut q = ViewQuery::new("Breit", "All");
    q.columns = ColumnSelection::Default;
    let tabs = [wide_tab()];
    let baseline = rss_mib();
    let (info, peak) = peak_rss_during(|| {
        let t = Instant::now();
        let info = export_tab_in(
            &tabs,
            &file,
            &CrawlConfig::default(),
            &q,
            dir.path(),
            Format::Csv,
        )
        .unwrap();
        println!("Export in {:?}", t.elapsed());
        info
    });
    assert_eq!(info.rows, rows);
    println!("Datei: {} MiB", info.bytes / (1024 * 1024));
    if let (Some(base), Some(peak)) = (baseline, peak) {
        let extra = peak - base;
        println!("RSS vorher {base:.0} MiB, Spitze {peak:.0} MiB, Zuwachs {extra:.0} MiB");
        assert!(extra <= 300.0, "Speicherzuwachs {extra:.0} MiB > 300 MiB");
    } else {
        println!("RSS auf dieser Plattform nicht messbar");
    }
}
