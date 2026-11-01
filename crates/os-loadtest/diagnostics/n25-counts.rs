//! M2-N25 Diagnose: Dauer der Zählungen, die die Oberfläche je Datenstand anstößt (eine Crawl-Datei, ruhend).
//! Nicht kompiliert (Referenz): nach `crates/os-analyze/examples/n25_counts.rs` kopieren, dann
//! `cargo run --release -p os-analyze --example n25_counts -- <datei.ospider>`.
use os_store::{CrawlFile, OpenMode};
use std::time::Instant;

fn main() {
    let path = std::env::args().nth(1).expect("Datei");
    let file = CrawlFile::open(std::path::Path::new(&path), OpenMode::ReadOnly).unwrap();
    let cfg = file.config().unwrap();
    let cancel = os_model::CancelToken::new();
    let t = Instant::now();
    let summary = os_store::queries::app_support::overview_summary(file.conn()).unwrap();
    println!("overview_summary: {:?} {:?}", t.elapsed(), summary);
    let t = Instant::now();
    let counts = os_analyze::views::filter_counts(&file, &cfg).unwrap();
    println!("filter_counts (sequentiell, catalog.filterCounts/export.catalog): {:?}, {} Filter", t.elapsed(), counts.len());
    let t = Instant::now();
    let (_facts, _) = os_analyze::views::overview_facts(&file, &cfg, &cancel).unwrap();
    println!("overview_facts (parallel, sidebar.overview): {:?}", t.elapsed());
    // je Tab
    let tabs = os_analyze::tabs::configured_for_file(&cfg, &file);
    let mut per: Vec<(String, std::time::Duration)> = Vec::new();
    for tab in &tabs {
        let t = Instant::now();
        let _ = os_analyze::views::filter_counts_in(std::slice::from_ref(tab), &file, &cfg);
        per.push((tab.id.to_string(), t.elapsed()));
    }
    per.sort_by(|a, b| b.1.cmp(&a.1));
    for slow in per.iter().take(2).map(|p| p.0.clone()).collect::<Vec<_>>() {
        let tab = tabs.iter().find(|t| t.id == slow).unwrap();
        let mut params = (tab.params)(&cfg);
        params.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
        let bind = |sql: &str| {
            let mut sql = sql.to_string();
            for (k, v) in &params {
                let lit = match v {
                    os_model::Value::Int(i) => i.to_string(),
                    os_model::Value::Float(f) => f.to_string(),
                    os_model::Value::Text(t) => format!("'{}'", t.replace('\'', "''")),
                    _ => "NULL".into(),
                };
                sql = sql.replace(&format!(":{k}"), &lit);
            }
            sql
        };
        for f in &tab.filters {
            let sql = bind(&format!("SELECT COUNT(*) FROM urls u WHERE ({}) AND ({})", tab.population, f.predicate));
            let t = Instant::now();
            let n: Result<i64, _> = file.conn().query_row(&sql, [], |r| r.get(0));
            println!("    {slow}/{}: {:?} -> {:?}", f.id, t.elapsed(), n);
            if t.elapsed().as_secs() >= 2 {
                let mut st = file.conn().prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
                let rows: Vec<String> = st.query_map([], |r| r.get::<_, String>(3)).unwrap().map(|r| r.unwrap()).collect();
                println!("      plan: {}", rows.join(" | "));
            }
        }
    }
}
