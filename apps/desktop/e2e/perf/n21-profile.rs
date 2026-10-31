//! Additive UI-147/UI-142 component profile; no acceptance budgets are changed.
use os_analyze::views::{
    self,
    session::{Budget, RowsReq, SessionSpec, ViewSession},
    ColumnSelection, ViewQuery,
};
use os_store::{CrawlFile, OpenMode};
use std::{path::Path, time::Instant};
fn mark(name: &str, at: Instant) {
    println!("{name}\t{:.3}", at.elapsed().as_secs_f64() * 1000.0);
}
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let at = Instant::now();
    let file = CrawlFile::open(Path::new(&args[1]), OpenMode::ReadOnly).unwrap();
    mark("file.open", at);
    let cfg = file.config().unwrap();
    if args.get(2).is_some_and(|v| v == "counts") {
        let at = Instant::now();
        let c = views::filter_counts(&file, &cfg).unwrap();
        mark("filter_counts", at);
        println!("filters={}", c.len());
        return;
    }
    if args.get(2).is_some_and(|v| v == "counts-tabs") {
        let tabs = os_analyze::tabs::configured_for_file(&cfg, &file);
        for tab in &tabs {
            let at = Instant::now();
            let c = views::filter_counts_in(std::slice::from_ref(tab), &file, &cfg).unwrap();
            mark(&format!("counts.{}[{}]", tab.id, c.len()), at);
        }
        return;
    }
    if args.get(2).is_some_and(|v| v == "sidebar") {
        let at = Instant::now();
        os_analyze::aggregates::live::refresh(&file, &cfg).unwrap();
        mark("sidebar.refresh", at);
        return;
    }
    let at = Instant::now();
    let count = os_store::counters::discovered(file.conn()).unwrap();
    mark("opening.url_total", at);
    println!("rows={count}");
    let tabs = os_analyze::tabs::configured_for_file(&cfg, &file);
    let mut q = ViewQuery::new("Internal", "All");
    q.columns = ColumnSelection::All;
    let at = Instant::now();
    let names = views::column_names_in(&tabs, &file, &cfg, &q).unwrap();
    mark("catalog.columns", at);
    let columns: Vec<_> = names.into_iter().take(65).collect();
    let s = SessionSpec {
        tab: "Internal".into(),
        filter: "All".into(),
        search: None,
        sort: vec![],
        columns: ColumnSelection::Named(columns),
        restrict: vec![],
    };
    let at = Instant::now();
    let view = ViewSession::open(&file, &cfg, s, Budget::default()).unwrap();
    mark("view.validate", at);
    let at = Instant::now();
    let total = view.prepare(&file, &Default::default()).unwrap();
    mark("view.prepare", at);
    for offset in [0, total - 200] {
        let at = Instant::now();
        let r = view
            .rows(
                &file,
                RowsReq {
                    version: view.version(),
                    offset,
                    limit: 200,
                },
            )
            .unwrap();
        mark(&format!("rows.{offset}"), at);
        assert_eq!(r.rows.len(), 200);
    }
}
