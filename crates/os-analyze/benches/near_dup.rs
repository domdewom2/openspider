//! Reproduzierbarer Mikrobenchmark; Großlauf über near_dup_scale (M2-Q03).
use criterion::{criterion_group, criterion_main, Criterion};
#[path = "../tests/framework_common.rs"]
mod common;
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
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = common::create(dir.path(), "lsh.ospider", 0xd02);
    file.conn().execute_batch("BEGIN").unwrap();
    for id in 1..=2000_i64 {
        // 1000 ähnliche Paare mit verschiedenen HTML-Hashes und tatsächlichen Kandidatentreffern.
        let group = (id - 1) / 2;
        let text = (0..120)
            .map(|word| format!("group{group}word{word}"))
            .collect::<Vec<_>>()
            .join(" ");
        common::insert(
            &file,
            &[common::row(id, &format!("{id}.html"))
                .text("content_class", "HTML")
                .int("status_code", 200)
                .text("hash_md5", &format!("hash{id}"))],
        );
        file.conn()
            .execute(
                "INSERT INTO url_ext(url_id,key,v_text) VALUES (?1,'content.text',?2)",
                (id, &text),
            )
            .unwrap();
        let signature = os_analyze::minhash::signature(&os_analyze::minhash::shingles(&text));
        file.conn()
            .execute(
                "INSERT INTO minhash VALUES (?1,?2)",
                (id, os_store::encode_minhash(&signature)),
            )
            .unwrap();
    }
    file.conn().execute_batch("COMMIT").unwrap();
    let mut cfg = os_model::CrawlConfig::default();
    cfg.duplicates.near_enabled = true;
    c.bench_function("near_dup/lsh_2000_pages", |b| {
        b.iter(|| {
            assert_eq!(
                os_analyze::analysis::steps::near_duplicates::run(
                    &file,
                    &cfg,
                    &os_model::CancelToken::new(),
                )
                .unwrap(),
                os_model::StepState::Done
            );
        });
    });
}
criterion_group!(benches, benchmark);
criterion_main!(benches);
