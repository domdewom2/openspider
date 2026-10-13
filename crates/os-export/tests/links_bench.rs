//! Messung der Link-Exporte (OUT-031): 1 Mio. Links werden in weniger als 60 s als CSV geschrieben.
//! `#[ignore]`-Benchmark, wird in P28 ausgeführt (Referenz-Hardware Apple M1, SSD):
//!
//! ```text
//! cargo test -p os-export --release --test links_bench -- --ignored --nocapture
//! ```
//!
//! `OS_BENCH_LINKS` verkleinert die Zahl der Links für einen Probelauf (Standard 1 000 000).

#[path = "links_support.rs"]
mod support;

use std::time::Instant;

use os_export::{export_bulk, Format};
use os_model::CrawlConfig;
use support::*;

// spec: OUT-031
#[test]
#[ignore = "Benchmark (P28): 1 Mio Links < 60 s"]
fn one_million_links_are_written_as_csv_in_under_60_seconds() {
    let links: u64 = std::env::var("OS_BENCH_LINKS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1_000_000);
    let pages = (links / 10).max(1);
    let dir = tempfile::tempdir().unwrap();
    let file = new_file(dir.path(), "links.ospider");
    exec(
        &file,
        &format!(
            "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < {pages}) \
             INSERT INTO urls (id, url, address, origin, is_internal, state, discovered_via, folder_depth, url_length, status_code, status, size_bytes) \
             SELECT i, 'https://site.test/p' || i, 'https://site.test/p' || i, 'https://site.test', 1, 'done', 'link', 0, 20, 200, 'OK', 1000 + i FROM n"
        ),
    );
    exec(
        &file,
        &format!(
            "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < {links}) \
             INSERT INTO links (id, source_id, target_id, target_raw, link_type, position, anchor, follow, rel, path_type, link_path, link_position, link_origin) \
             SELECT i, 1 + (i % {pages}), 1 + ((i * 7) % {pages}), '/p' || i, 'Hyperlink', i, 'Anker ' || i, 1, NULL, 'Root-Relative', \
                    '/html[1]/body[1]/main[1]/p[' || (i % 50) || ']/a[1]', 'Content', 'HTML' FROM n"
        ),
    );
    let before = rss_mib();
    let ((), peak) = peak_rss_during(|| {
        let t = Instant::now();
        let infos = export_bulk(
            &file,
            &CrawlConfig::default(),
            "Links:All Inlinks",
            dir.path(),
            Format::Csv,
        )
        .unwrap();
        let took = t.elapsed();
        println!(
            "{} Links in {took:?}, {} MiB",
            infos[0].rows,
            infos[0].bytes / (1024 * 1024)
        );
        assert_eq!(infos[0].rows, links);
        assert!(took.as_secs() < 60, "{took:?} ≥ 60 s");
    });
    if let (Some(b), Some(p)) = (before, peak) {
        println!("RSS vorher {b:.0} MiB, Spitze {p:.0} MiB");
    }
}
