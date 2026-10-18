#[path = "view_session_fixture.rs"]
mod fixture;
use os_analyze::views::session::{Budget, RowsReq, ViewSession};
use os_model::CrawlConfig;
use std::time::Instant;

fn measure(count: u32) {
    let (_dir, file) = fixture::synthetic(count);
    let cfg = CrawlConfig::default();
    let mut view = ViewSession::open(
        &file,
        &cfg,
        fixture::specification("Status Code", false),
        Budget::default(),
    )
    .unwrap();
    view.prepare(&file, &Default::default()).unwrap();
    let mut times = Vec::new();
    for i in 0..100u64 {
        let start = Instant::now();
        let rows = view
            .rows(
                &file,
                RowsReq {
                    version: view.version(),
                    offset: i * (u64::from(count) - 200) / 99,
                    limit: 200,
                },
            )
            .unwrap();
        assert_eq!(rows.rows.len(), 200);
        assert_eq!(rows.columns.len(), 20);
        times.push(start.elapsed());
    }
    times.sort();
    assert!(times[94].as_millis() <= 30, "rows p95 {:?}", times[94]);
    let start = Instant::now();
    view.update(&file, &cfg, fixture::specification("Status Code", true))
        .unwrap();
    assert!(
        start.elapsed().as_millis() <= 200,
        "direction {:?}",
        start.elapsed()
    );
    eprintln!(
        "rows={count} p95={:?} direction={:?} retained_bytes={}",
        times[94],
        start.elapsed(),
        view.memory_bytes()
    );
}
// spec: UI-141
// spec: UI-142
#[test]
fn synthetic_100k_windows_and_reversal() {
    let (_dir, file) = fixture::synthetic(100_000);
    let cfg = CrawlConfig::default();
    let mut view = ViewSession::open(
        &file,
        &cfg,
        fixture::specification("Status Code", false),
        Budget::default(),
    )
    .unwrap();
    let first = view
        .rows(
            &file,
            RowsReq {
                version: 1,
                offset: 99_800,
                limit: 200,
            },
        )
        .unwrap();
    assert_eq!(first.rows.len(), 200);
    assert_eq!(first.columns.len(), 20);
    view.update(&file, &cfg, fixture::specification("Status Code", true))
        .unwrap();
    assert_eq!(
        view.rows(
            &file,
            RowsReq {
                version: 2,
                offset: 0,
                limit: 200
            }
        )
        .unwrap()
        .total,
        100_000
    );
}
// spec: UI-141, UI-142
#[test]
#[ignore = "Benchmark (M2-U31): 100k Zeitbudgets auf ruhiger Referenzhardware"]
fn synthetic_100k_latency_budgets() {
    measure(100_000);
}
// spec: UI-141
// spec: UI-142
#[test]
#[ignore = "Benchmark (M2-U31): 1 Mio. auf ruhiger Referenzhardware"]
fn synthetic_1m_windows_and_reversal() {
    measure(1_000_000);
}
// spec: UI-141
// spec: UI-142
#[test]
#[ignore = "Benchmark (M2-U31): 5 Mio. auf ruhiger Referenzhardware"]
fn synthetic_5m_windows_and_reversal() {
    measure(5_000_000);
}

// spec: UI-141
#[test]
fn text_merge_across_sort_blocks_matches_sql_with_unicode_ties_and_nulls() {
    use os_analyze::views::{self, ViewQuery};
    let (_dir, file) = fixture::synthetic(70_000);
    file.conn()
        .execute(
            "UPDATE urls SET title_1=CASE WHEN id%26=0 THEN 'Äpfel' ELSE 'äPFEL' END WHERE id%13=0",
            [],
        )
        .unwrap();
    let cfg = CrawlConfig::default();
    let mut session = ViewSession::open(
        &file,
        &cfg,
        fixture::specification("Title 1", false),
        Budget::default(),
    )
    .unwrap();
    for descending in [false, true] {
        let spec = fixture::specification("Title 1", descending);
        let mut query = ViewQuery::new("Internal", "All");
        query.sort = spec.sort.clone();
        query.columns = spec.columns.clone();
        query.limit = 500;
        let expected = views::query_view(&file, &cfg, &query).unwrap();
        session.update(&file, &cfg, spec).unwrap();
        let actual = session
            .rows(
                &file,
                RowsReq {
                    version: session.version(),
                    offset: 0,
                    limit: 500,
                },
            )
            .unwrap();
        assert_eq!(actual.row_keys, expected.row_keys);
        assert_eq!(actual.rows, expected.rows);
    }
}
