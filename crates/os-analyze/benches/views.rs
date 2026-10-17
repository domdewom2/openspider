//! K01 core benchmarks: 100k by default, 1m/5m with OPENSPIDER_BENCH_LARGE=1.
//! Run: cargo bench -p os-analyze --bench views -- --noplot
mod views_session;
use criterion::{criterion_group, criterion_main, Criterion};
fn sessions(c: &mut Criterion) {
    views_session::run(c);
}
criterion_group! { name = benches; config = Criterion::default().sample_size(10); targets = sessions }
criterion_main!(benches);
