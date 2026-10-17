#[path = "../tests/view_session_fixture.rs"]
mod fixture;
use criterion::{BenchmarkId, Criterion};
use fixture::{specification, synthetic};
use os_analyze::{
    search::FilterSource,
    views::session::{Budget, RowsReq, ViewSession},
    ViewError,
};
use os_model::CrawlConfig;
use os_store::CrawlFile;
use roaring::RoaringBitmap;
use std::{hint::black_box, sync::Arc, time::Duration};
struct SearchBitmap(RoaringBitmap);
impl FilterSource for SearchBitmap {
    fn key(&self) -> String {
        "fixture:search".into()
    }
    fn bitmap(&self, _: &CrawlFile) -> Result<RoaringBitmap, ViewError> {
        Ok(self.0.clone())
    }
}
// spec: UI-141
// spec: UI-142
pub fn run(c: &mut Criterion) {
    let sizes = if std::env::var("OPENSPIDER_BENCH_LARGE").as_deref() == Ok("1") {
        vec![100_000, 1_000_000, 5_000_000]
    } else {
        vec![100_000]
    };
    for size in sizes {
        let (_dir, file) = synthetic(size);
        let cfg = CrawlConfig::default();
        let mut view = ViewSession::open(
            &file,
            &cfg,
            specification("Status Code", false),
            Budget::default(),
        )
        .unwrap();
        let req = |v: &ViewSession| RowsReq {
            version: v.version(),
            offset: u64::from(size) - 200,
            limit: 200,
        };
        view.rows(&file, req(&view)).unwrap();
        eprintln!(
            "K01 rows={size} columns=20 retained_bytes={}",
            view.memory_bytes()
        );
        let mut group = c.benchmark_group("view_session");
        group.warm_up_time(Duration::from_secs(1));
        group.measurement_time(Duration::from_secs(3));
        group.bench_with_input(BenchmarkId::new("rows_200x20", size), &size, |b, _| {
            b.iter(|| black_box(view.rows(&file, req(&view)).unwrap()))
        });
        group.bench_with_input(BenchmarkId::new("locate", size), &size, |b, n| {
            b.iter(|| black_box(view.locate(&file, os_model::UrlId(i64::from(*n))).unwrap()))
        });
        for column in ["Status Code", "Title 1"] {
            group.bench_with_input(
                BenchmarkId::new(format!("sort_cold_{column}"), size),
                &size,
                |b, _| {
                    b.iter(|| {
                        let session = ViewSession::open(
                            &file,
                            &cfg,
                            specification(column, false),
                            Budget::default(),
                        )
                        .unwrap();
                        black_box(session.prepare(&file, &Default::default()).unwrap());
                    })
                },
            );
            view.update(&file, &cfg, specification(column, false))
                .unwrap();
            let mut descending = false;
            group.bench_with_input(
                BenchmarkId::new(format!("reverse_{column}"), size),
                &size,
                |b, _| {
                    b.iter(|| {
                        descending = !descending;
                        view.update(&file, &cfg, specification(column, descending))
                            .unwrap();
                        black_box(view.version());
                    })
                },
            );
        }
        group.bench_with_input(BenchmarkId::new("filter_cached", size), &size, |b, _| {
            b.iter(|| {
                let mut spec = specification("Status Code", false);
                spec.filter = "HTML".into();
                view.update(&file, &cfg, spec).unwrap();
                black_box(view.version());
            })
        });
        let search: Arc<dyn FilterSource> =
            Arc::new(SearchBitmap((1..=size).filter(|id| id % 5 == 0).collect()));
        group.bench_with_input(BenchmarkId::new("search_bitmap", size), &size, |b, _| {
            b.iter(|| {
                let mut spec = specification("Status Code", false);
                spec.restrict.push(search.clone());
                view.update(&file, &cfg, spec).unwrap();
                black_box(view.version());
            })
        });
        eprintln!(
            "K01 all_permutations rows={size} retained_bytes={}",
            view.memory_bytes()
        );
        group.finish();
    }
}
