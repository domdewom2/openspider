//! K2-47 acceptance workloads; opt in on idle reference hardware with --release --ignored.
#[path = "tab_links_support.rs"]
mod support;
use os_analyze::analysis::{steps::link_score, CancelToken};
use os_model::CrawlConfig;

fn workload(n: u64, fanout: u64, seconds: u64) {
    let (_dir, file) = support::file();
    let conn = file.conn();
    conn.execute_batch("BEGIN").unwrap();
    conn.execute(&format!("WITH RECURSIVE ids(id) AS (VALUES(1) UNION ALL SELECT id+1 FROM ids WHERE id < {n}) INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,folder_depth,url_length,content_class,status_code) SELECT id,'https://site.test/'||id,'https://site.test/'||id,'https://site.test',1,'done','link',0,1,'HTML',200 FROM ids"),[]).unwrap();
    for delta in 1..=fanout {
        conn.execute(&format!("INSERT INTO links(source_id,target_id,target_raw,link_type,position,follow) SELECT id, (id+{delta}-1)%{n}+1,'/','Hyperlink',{delta},1 FROM urls"),[]).unwrap();
    }
    conn.execute_batch("COMMIT").unwrap();
    let start = std::time::Instant::now();
    link_score::run(&file, &CrawlConfig::default(), &CancelToken::new()).unwrap();
    let elapsed = start.elapsed();
    eprintln!(
        "nodes={n} edges={} elapsed={elapsed:?}; edges on disk; O(nodes) routing/rank vectors",
        n * fanout
    );
    assert!(
        elapsed.as_secs_f64() <= seconds as f64,
        "{elapsed:?} > {seconds}s"
    );
    // On Linux record process high-water RSS; on macOS use Instruments for the reference-hardware run.
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        if let Some(kib) = status
            .lines()
            .find(|l| l.starts_with("VmHWM:"))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|s| s.parse::<u64>().ok())
        {
            eprintln!("process_peak_rss_kib={kib}");
            assert!(kib < 2 * 1024 * 1024);
        }
    }
}
// spec: PERF-007
#[test]
#[ignore = "Benchmark: 1M nodes / 30M edges < 5 min on reference hardware"]
fn thirty_million_edges_under_five_minutes() {
    workload(1_000_000, 30, 300);
}
// spec: TECH-145
#[test]
#[ignore = "Benchmark: 5M nodes / 100M edges <= 60s and <= 2GB on reference hardware"]
fn hundred_million_edges_under_sixty_seconds() {
    workload(5_000_000, 20, 60);
}
