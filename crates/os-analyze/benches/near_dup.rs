//! Reproduzierbarer Mikrobenchmark; Großlauf über near_dup_scale (M2-Q03).
use criterion::{criterion_group, criterion_main, Criterion};
fn benchmark(c: &mut Criterion) {
    let text = (0..500)
        .map(|n| format!("word{n}"))
        .collect::<Vec<_>>()
        .join(" ");
    let shingles = os_analyze::minhash::shingles(&text);
    c.bench_function("near_dup/signature_500_words", |b| {
        b.iter(|| os_analyze::minhash::signature(std::hint::black_box(&shingles)))
    });
    c.bench_function("near_dup/jaccard_500_words", |b| {
        b.iter(|| os_analyze::minhash::jaccard(std::hint::black_box(&shingles), &shingles))
    });
}
criterion_group!(benches, benchmark);
criterion_main!(benches);
