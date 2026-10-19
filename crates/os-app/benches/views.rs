//! Benchmarks der View-Sitzungen der App (K-1, UI-141, UI-142), gemessen über `App::dispatch` (ohne IPC; der
//! IPC-Anteil folgt in M2-U31).
//!
//! Standard: 100 000 Zeilen; `OPENSPIDER_BENCH_LARGE=1` misst 1 Mio. und 5 Mio. Zeilen (Detail-View je mit bis zu
//! 1 Mio. Inlinks einer URL), `OPENSPIDER_BENCH_ROWS=<n>` genau eine Größe. Jede Messung erscheint als Zeile `K05 rows=<n> <metric>=<wert>`; die Zeilenfenster
//! und `view.locate` laufen zusätzlich als criterion-Gruppe `app_view`.
// spec: UI-141
// spec: UI-142
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "../tests/runtime_common.rs"]
mod common;

use criterion::{BenchmarkId, Criterion};
use os_app::{
    dto::OpState, pool::CrawlKey, runtime::sessions::SessionInfo, App, AppConfig, CallCtx,
};
use os_model::{CrawlConfig, CrawlState};
use os_store::{CrawlFile, OpenMode};
use serde_json::{json, Value};
use std::{
    hint::black_box,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

const WINDOW: u32 = 200;

fn synthetic(dir: &Path, rows: u32) -> std::path::PathBuf {
    let path = common::make_crawl(dir, "bench.ospider");
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    file.conn()
        .execute(
            "WITH RECURSIVE n(id) AS (SELECT 1 UNION ALL SELECT id + 1 FROM n WHERE id < ?1) \
         INSERT INTO urls (id, url, address, origin, is_internal, in_tables, state, discovered_via, folder_depth, url_length, content_class, status_code, title_1, title_count, size_bytes, word_count) \
         SELECT id, printf('https://fixture.test/%08d', id), printf('https://fixture.test/%08d', id), 'https://fixture.test', 1, 1, 'Done', 'Link', 0, 38, \
         CASE WHEN id % 10 = 0 THEN 'Image' ELSE 'HTML' END, 200 + (id % 5), \
         CASE WHEN id % 17 = 0 THEN NULL ELSE printf('Title %08d', (id * 7919) % ?1) END, 1, (id * 3571) % 100000, (id * 31) % 5000 FROM n",
            [rows],
        )
        .unwrap();
    // Detail view: up to one million inlinks of URL 1 (K-1g).
    file.conn()
        .execute(
            "WITH RECURSIVE n(id) AS (SELECT 1 UNION ALL SELECT id + 1 FROM n WHERE id < ?1) \
         INSERT INTO links (id, source_id, target_id, target_raw, link_type, position, anchor, follow, link_position) \
         SELECT id, 1 + (id % ?2), 1, 'https://fixture.test/00000001', 'Hyperlink', id, printf('a%d', id % 97), 1, 'Content' FROM n",
            [rows.min(1_000_000), rows],
        )
        .unwrap();
    file.conn()
        .execute_batch("UPDATE crawl_meta SET value='1' WHERE key='change_seq'")
        .unwrap();
    file.set_meta("state", "Completed").unwrap();
    path
}

fn columns() -> Vec<String> {
    let tab = os_analyze::tabs::configured(&CrawlConfig::default())
        .into_iter()
        .find(|t| t.id == "Internal")
        .unwrap();
    tab.columns
        .iter()
        .filter(|c| c.default_visible)
        .take(20)
        .map(|c| c.key.to_string())
        .collect()
}

fn tab(filter: &str, sort: Value, search: Value) -> Value {
    json!({"kind":"tab","tab":"Internal","filter":filter,"search":search,"sort":sort,"segment":null,"compare":null})
}

struct Bench {
    app: App,
    rows: u32,
}
impl Bench {
    async fn call(&self, name: &str, args: Value) -> Value {
        self.app
            .dispatch(CallCtx::for_window("main"), name, args)
            .await
            .unwrap_or_else(|e| panic!("{name}: {e:?}"))
    }
    /// Waits for a `Building` handle's operation and returns the ready handle.
    async fn ready(&self, h: Value) -> Value {
        self.try_ready(h).await.unwrap_or_else(|e| panic!("{e:?}"))
    }
    async fn try_ready(&self, h: Value) -> Result<Value, os_app::CmdError> {
        if h["state"] == "ready" {
            return Ok(h);
        }
        let id = h["op"]["opId"].as_str().unwrap().to_string();
        loop {
            let s = self.app.ops().status(&id).unwrap();
            match s.state {
                OpState::Done => return Ok(s.result.unwrap()),
                OpState::Running => tokio::time::sleep(Duration::from_millis(1)).await,
                _ => return Err(s.error.unwrap()),
            }
        }
    }
    async fn timed_update(&self, h: &Value, spec: Value) -> (Value, Duration) {
        let t = Instant::now();
        let h = self
            .call("view.update", json!({"viewId":h["viewId"],"spec":spec}))
            .await;
        let h = self.ready(h).await;
        (h, t.elapsed())
    }
    /// Like `timed_update`, but a failure (e.g. the search budget) is reported instead of aborting the run.
    async fn measured_update(&self, metric: &str, h: &Value, spec: Value) -> Option<Value> {
        let t = Instant::now();
        let r = match self
            .app
            .dispatch(
                CallCtx::for_window("main"),
                "view.update",
                json!({"viewId":h["viewId"],"spec":spec}),
            )
            .await
        {
            Ok(h) => self.try_ready(h).await,
            Err(e) => Err(e),
        };
        match r {
            Ok(h) => {
                self.report(metric, t.elapsed());
                Some(h)
            }
            Err(e) => {
                eprintln!(
                    "K05 rows={} {metric}=failed:{} after_ms={:.1}",
                    self.rows,
                    e.message_key,
                    t.elapsed().as_secs_f64() * 1000.0
                );
                None
            }
        }
    }
    async fn rows(&self, h: &Value, offset: u64) -> Value {
        self.call(
            "view.rows",
            json!({"viewId":h["viewId"],"version":h["version"],"offset":offset,"limit":WINDOW,"columns":[]}),
        )
        .await
    }
    fn report(&self, metric: &str, d: Duration) {
        eprintln!(
            "K05 rows={} {metric}_ms={:.1}",
            self.rows,
            d.as_secs_f64() * 1000.0
        );
    }
}

fn percentile(mut v: Vec<Duration>, p: f64) -> Duration {
    v.sort();
    v[((v.len() as f64 - 1.0) * p).round() as usize]
}
/// Deterministic offsets across the whole view (scrollbar jumps).
fn offsets(total: u64, n: usize) -> Vec<u64> {
    let mut x = 0x2545_f491_4f6c_dd1du64;
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x % total.saturating_sub(u64::from(WINDOW)).max(1)
        })
        .collect()
}

async fn scenario(b: &Bench) -> Value {
    let cols = columns();
    // Cold open: filter bitmap, positions and projection (UI-141 first build).
    let t = Instant::now();
    let h = b
        .call(
            "view.open",
            json!({"spec":tab("All", json!([]), Value::Null),"columns":cols}),
        )
        .await;
    let h = b.ready(h).await;
    b.report("open_cold", t.elapsed());
    let total = h["total"].as_u64().unwrap();
    assert_eq!(total, u64::from(b.rows));
    // UI-142: 200 rows × 20 columns once the view is built.
    let mut times = Vec::new();
    let mut bytes = 0;
    for offset in offsets(total, 200) {
        let t = Instant::now();
        let w = b.rows(&h, offset).await;
        times.push(t.elapsed());
        bytes = bytes.max(serde_json::to_vec(&w).unwrap().len());
    }
    b.report("rows_200x20_p50", percentile(times.clone(), 0.5));
    b.report("rows_200x20_p95", percentile(times, 0.95));
    eprintln!("K05 rows={} rows_200x20_payload_bytes={bytes}", b.rows);
    let mut times = Vec::new();
    for id in offsets(total, 100) {
        let t = Instant::now();
        black_box(
            b.call("view.locate", json!({"viewId":h["viewId"],"urlId":id + 1}))
                .await,
        );
        times.push(t.elapsed());
    }
    b.report("locate_p95", percentile(times, 0.95));
    // UI-141: tab/filter switch – cold once, then warm (bitmap cached in the session).
    let (_, cold) = b
        .timed_update(&h, tab("HTML", json!([]), Value::Null))
        .await;
    b.report("filter_switch_cold", cold);
    let (_, warm_all) = b.timed_update(&h, tab("All", json!([]), Value::Null)).await;
    let (_, warm_html) = b
        .timed_update(&h, tab("HTML", json!([]), Value::Null))
        .await;
    b.report("filter_switch_warm", warm_all.max(warm_html));
    // UI-141: numeric and text sort, then reversal.
    for (metric, column) in [("sort_numeric", "Status Code"), ("sort_text", "Title 1")] {
        let (_, d) = b
            .timed_update(
                &h,
                tab("All", json!([{"column":column,"dir":"asc"}]), Value::Null),
            )
            .await;
        b.report(&format!("{metric}_cold"), d);
        let (_, d) = b
            .timed_update(
                &h,
                tab("All", json!([{"column":column,"dir":"desc"}]), Value::Null),
            )
            .await;
        b.report(&format!("{metric}_reverse"), d);
    }
    // UI-141: Address substring and regex search.
    if let Some(s) = b
        .measured_update(
            "search_substring",
            &h,
            tab("All", json!([]), json!({"kind":"simple","text":"00001"})),
        )
        .await
    {
        assert!(s["total"].as_u64().unwrap() > 0);
    }
    let tree = json!({"kind":"group","op":"and","children":[{"kind":"cond","column":"Address","op":"regex","value":"/0+1[0-9]*5$"}]});
    b.measured_update(
        "search_regex",
        &h,
        tab(
            "All",
            json!([]),
            json!({"kind":"rules","tree":tree,"caseSensitive":false}),
        ),
    )
    .await;
    // UI-141: cancelling a running build restores the previous view.
    let (h, _) = b.timed_update(&h, tab("All", json!([]), Value::Null)).await;
    let building = b
        .call(
            "view.update",
            json!({"viewId":h["viewId"],"spec":tab("All", json!([{"column":"Size (Bytes)","dir":"desc"}]), Value::Null)}),
        )
        .await;
    if building["state"] == "building" {
        let t = Instant::now();
        b.call("op.cancel", json!({"opId":building["op"]["opId"]}))
            .await;
        let id = building["op"]["opId"].as_str().unwrap();
        while b.app.ops().status(id).unwrap().state == OpState::Running {
            tokio::time::sleep(Duration::from_micros(200)).await;
        }
        black_box(b.rows(&h, 0).await);
        b.report("cancel_restore", t.elapsed());
    } else {
        eprintln!("K05 rows={} cancel_restore=build_under_300ms", b.rows);
    }
    // K-1g: detail view with up to one million inlinks of one URL.
    let t = Instant::now();
    let d = b
        .call(
            "view.open",
            json!({"spec":{"kind":"inlinks","urlIds":[1],"filter":{"id":"All"},"search":null,"sort":[],"compare":null},"columns":[]}),
        )
        .await;
    b.report("inlinks_open", t.elapsed());
    let inlinks = d["total"].as_u64().unwrap();
    let t = Instant::now();
    black_box(b.rows(&d, 0).await);
    b.report("inlinks_materialize", t.elapsed());
    let mut times = Vec::new();
    for offset in offsets(inlinks, 20) {
        let t = Instant::now();
        black_box(b.rows(&d, offset).await);
        times.push(t.elapsed());
    }
    eprintln!("K05 rows={} inlinks={inlinks}", b.rows);
    b.report("inlinks_rows_p95", percentile(times, 0.95));
    h
}

fn main() {
    let sizes: Vec<u32> = if let Ok(n) = std::env::var("OPENSPIDER_BENCH_ROWS") {
        vec![n.parse().expect("OPENSPIDER_BENCH_ROWS")]
    } else if std::env::var("OPENSPIDER_BENCH_LARGE").as_deref() == Ok("1") {
        vec![1_000_000, 5_000_000]
    } else {
        vec![100_000]
    };
    let mut c = Criterion::default().configure_from_args();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    for rows in sizes {
        let dir = tempfile::tempdir().unwrap();
        let path = synthetic(dir.path(), rows);
        let home = tempfile::tempdir().unwrap();
        let app = App::new(AppConfig::new(home.path())).unwrap();
        let key = CrawlKey("bench".into());
        app.register_crawl(key.clone(), path.clone());
        app.sessions().set_current(
            os_app::WindowId("main".into()),
            SessionInfo {
                key,
                path,
                config: Arc::new(CrawlConfig::default()),
                state: CrawlState::Completed,
                writable: false,
                engine: None,
            },
        );
        let b = Bench { app, rows };
        let h = rt.block_on(scenario(&b));
        let total = h["total"].as_u64().unwrap();
        let mut group = c.benchmark_group("app_view");
        group.warm_up_time(Duration::from_secs(1));
        group.measurement_time(Duration::from_secs(3));
        let mut at = offsets(total, 1024).into_iter().cycle();
        group.bench_with_input(BenchmarkId::new("rows_200x20", rows), &rows, |bench, _| {
            bench.iter(|| black_box(rt.block_on(b.rows(&h, at.next().unwrap()))))
        });
        group.bench_with_input(BenchmarkId::new("locate", rows), &rows, |bench, _| {
            bench.iter(|| {
                black_box(rt.block_on(b.call(
                    "view.locate",
                    json!({"viewId":h["viewId"],"urlId":rows / 2}),
                )))
            })
        });
        group.finish();
    }
    c.final_summary();
}
