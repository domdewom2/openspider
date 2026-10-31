//! SQL plans and extraction timings; the unchanged U31 runner supplies budget measurements.
use os_analyze::{views::ViewQuery, SortKey};
use os_model::CrawlConfig;
use os_store::{CrawlFile, OpenMode};
use std::{path::Path, time::Instant};

fn main() {
    let path = std::env::args().nth(1).expect("fixture path");
    let file = CrawlFile::open(Path::new(&path), OpenMode::ReadOnly).unwrap();
    profile(
        &file,
        &CrawlConfig::default(),
        !std::env::args().any(|arg| arg == "--plans-only"),
    );
}

// spec: UI-141
fn profile(file: &CrawlFile, cfg: &CrawlConfig, extract: bool) {
    for (label, filter, column) in [
        ("filter", "HTML", None),
        ("numeric", "All", Some("Status Code")),
        ("text", "All", Some("Title 1")),
        ("address", "All", None),
    ] {
        let mut q = ViewQuery::new("Internal", filter);
        q.sort = column
            .into_iter()
            .map(|c| SortKey {
                column: c.into(),
                descending: false,
            })
            .collect();
        let plan = os_analyze::views::plan(&q, cfg).unwrap();
        let select = match label {
            "numeric" => "u.id,u.status_code",
            "text" => "u.id,u.title_1",
            "address" => "u.id,u.address",
            _ => "u.id",
        };
        let sql = format!(
            "SELECT {select} FROM urls u WHERE ({}) AND ({}) {}",
            plan.population_sql,
            plan.filter_sql,
            if column.is_some() {
                plan.order_by.as_str()
            } else {
                ""
            }
        );
        println!("{label} SQL: {sql}");
        let mut explain = file
            .conn()
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .unwrap();
        let rows = explain.query_map([], |r| r.get::<_, String>(3)).unwrap();
        for row in rows {
            println!("{label} PLAN: {}", row.unwrap());
        }
    }
    // Actual pre-N22 permutation/search statements, distinct from the SQL page oracle above.
    for (label, sql) in [
        ("legacy numeric permutation", "SELECT u.id,u.status_code FROM urls u ORDER BY u.status_code ASC"),
        ("legacy text extraction", "SELECT u.id,CASE WHEN u.content_class IN ('HTML','PDF') AND u.status_code BETWEEN 200 AND 299 THEN u.title_1 END FROM urls u"),
        ("new numeric extraction", "SELECT u.id,u.status_code,u.address FROM urls u"),
        ("new text extraction", "SELECT u.id,CASE WHEN u.content_class IN ('HTML','PDF') AND u.status_code BETWEEN 200 AND 299 THEN u.title_1 END,u.address FROM urls u"),
        ("legacy address search", "SELECT u.id,u.address FROM urls u LEFT JOIN url_counters c ON c.url_id=u.id WHERE u.is_internal=1 AND u.in_tables=1"),
        ("new address search", "SELECT u.id,u.address FROM urls u WHERE u.is_internal=1 AND u.in_tables=1"),
    ] {
        println!("{label} SQL: {sql}");
        let mut stmt = file.conn().prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
        for row in stmt.query_map([], |r| r.get::<_, String>(3)).unwrap() {
            println!("{label} PLAN: {}", row.unwrap());
        }
    }
    if !extract {
        return;
    }
    for (label, sql) in [
        (
            "numeric extraction",
            "SELECT id,status_code,address FROM urls",
        ),
        ("text extraction", "SELECT id,title_1,address FROM urls"),
        ("address extraction", "SELECT id,address FROM urls"),
    ] {
        let at = Instant::now();
        let mut stmt = file.conn().prepare(sql).unwrap();
        let mut rows = stmt.query([]).unwrap();
        let mut count = 0u64;
        let mut bytes = 0usize;
        while let Some(row) = rows.next().unwrap() {
            for i in 0..row.as_ref().column_count() {
                let value = row.get_ref(i).unwrap();
                if let Ok(text) = value.as_str() {
                    bytes += text.len();
                }
                std::hint::black_box(value);
            }
            count += 1;
        }
        println!(
            "{label}: rows={count} bytes={bytes} ms={:.3}",
            at.elapsed().as_secs_f64() * 1000.0
        );
    }
}
