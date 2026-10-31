//! Standalone runner linked against the locked release artifacts; no workspace manifest changes.
use os_analyze::search::{Rule, RuleNode, RuleOp};
use os_analyze::views::session::{Budget, RowsReq, SessionSpec, ViewSession};
use os_analyze::views::{ColumnSelection, SearchSpec};
use os_analyze::SortKey;
use os_model::{CancelToken, CrawlConfig};
use os_store::{CrawlFile, OpenMode};
use std::{path::Path, time::Instant};

fn spec(column: Option<&str>, descending: bool, search: Option<SearchSpec>) -> SessionSpec {
    let tab = os_analyze::tabs::configured(&CrawlConfig::default())
        .into_iter()
        .find(|t| t.id == "Internal")
        .unwrap();
    let columns: Vec<_> = tab
        .columns
        .iter()
        .filter(|c| c.default_visible)
        .take(20)
        .map(|c| c.key.to_string())
        .collect();
    assert_eq!(columns.len(), 20);
    let mut q = os_analyze::views::ViewQuery::new("Internal", "All");
    q.search = search;
    q.columns = ColumnSelection::Named(columns.clone());
    let restrict =
        os_analyze::search::SearchFilter::new(&q, &CrawlConfig::default(), CancelToken::default())
            .map(|s| std::sync::Arc::new(s) as std::sync::Arc<dyn os_analyze::search::FilterSource>)
            .into_iter()
            .collect();
    SessionSpec {
        tab: "Internal".into(),
        filter: "All".into(),
        search: None,
        sort: column
            .into_iter()
            .map(|c| SortKey {
                column: c.into(),
                descending,
            })
            .collect(),
        columns: ColumnSelection::Named(columns),
        restrict,
    }
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let rows: u64 = args[2].parse().unwrap();
    let file = CrawlFile::open(Path::new(&args[1]), OpenMode::ReadOnly).unwrap();
    let cfg = CrawlConfig::default();
    let mut view =
        ViewSession::open(&file, &cfg, spec(None, false, None), Budget::default()).unwrap();
    let total = view.prepare(&file, &Default::default()).unwrap();
    assert_eq!(total, rows);
    let mut samples = vec![];
    // spec: UI-142
    for i in 0..55 {
        let at = Instant::now();
        let result = view
            .rows(
                &file,
                RowsReq {
                    version: view.version(),
                    offset: (i * 200) % (rows - 200),
                    limit: 200,
                },
            )
            .unwrap();
        let ms = at.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(result.columns.len(), 20);
        assert_eq!(result.rows.len(), 200);
        if i >= 5 {
            samples.push(ms);
        }
    }
    let raw = samples.clone();
    samples.sort_by(f64::total_cmp);
    let mut passed = true;
    let mut metrics = vec![];
    let mut errors = vec![];
    let mut record = |name: &str, value: f64, budget: f64, completed: bool| {
        let ok = completed && value <= budget;
        passed &= ok;
        metrics.push(format!("\"{name}\":{{\"value\":{value},\"budget\":{budget},\"direction\":\"max\",\"passed\":{ok},\"completed\":{completed}}}"));
    };
    record("rowsP95Ms", samples[47], 30.0, true);
    // spec: UI-141
    for (name, next, budget) in [
        (
            "filterMs",
            {
                let mut s = spec(None, false, None);
                s.filter = "HTML".into();
                s
            },
            200.0,
        ),
        (
            "numericMs",
            spec(Some("Status Code"), false, None),
            if rows > 1_000_000 { 5000.0 } else { 1500.0 },
        ),
        ("reverseMs", spec(Some("Status Code"), true, None), 200.0),
        (
            "textMs",
            spec(Some("Title 1"), false, None),
            if rows > 1_000_000 { 8000.0 } else { 2500.0 },
        ),
        (
            "substringMs",
            spec(None, false, Some(SearchSpec::Simple("0001".into()))),
            if rows > 1_000_000 { 3000.0 } else { 1000.0 },
        ),
        (
            "regexMs",
            spec(
                None,
                false,
                Some(SearchSpec::Rules {
                    tree: RuleNode::Rule(Rule {
                        column: "Address".into(),
                        op: RuleOp::Regex,
                        value: "000[12]".into(),
                    }),
                    case_sensitive: false,
                }),
            ),
            if rows > 1_000_000 { 10000.0 } else { 3000.0 },
        ),
    ] {
        let at = Instant::now();
        let operation = view.update(&file, &cfg, next).and_then(|_| {
            view.prepare(&file, &Default::default())?;
            view.rows(
                &file,
                RowsReq {
                    version: view.version(),
                    offset: 0,
                    limit: 200,
                },
            )?;
            Ok(())
        });
        let completed = operation.is_ok();
        if let Err(error) = operation {
            errors.push(format!("{name}: {error:?}"));
        }
        record(name, at.elapsed().as_secs_f64() * 1000.0, budget, completed);
    }
    println!("{{\"scenario\":\"core\",\"size\":\"{}\",\"started_at\":\"{}\",\"engine\":\"core\",\"metrics\":{{{}}},\"details\":{{\"rows\":{},\"seed\":19,\"samples\":{:?},\"retained_bytes\":{},\"errors\":{:?}}},\"passed\":{}}}",
        if rows == 5_000_000 { "large" } else { "small" }, args[3], metrics.join(","), rows, raw, view.memory_bytes(), errors, passed && errors.is_empty());
}
