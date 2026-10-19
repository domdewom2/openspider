//! Local throughput probe. Acceptance workloads are in tests/link_score_benchmarks.rs.
#[path = "../tests/tab_links_support.rs"]
mod support;
use criterion::{criterion_group, criterion_main, Criterion};
use os_analyze::analysis::{steps::link_score, CancelToken};
use os_model::CrawlConfig;
fn measure(c: &mut Criterion) {
    let (_dir, file) = support::file();
    support::nodes(&file, 10_000);
    support::sql(&file,"INSERT INTO links(source_id,target_id,target_raw,link_type,position,follow) SELECT id,id%10000+1,'/','Hyperlink',1,1 FROM urls");
    c.bench_function("link_score/10000_nodes_disk_graph", |b| {
        b.iter(|| link_score::run(&file, &CrawlConfig::default(), &CancelToken::new()).unwrap())
    });
}
criterion_group!(benches, measure);
criterion_main!(benches);
