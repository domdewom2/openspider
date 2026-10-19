#[path = "aggregates_fixture.rs"]
mod fixture;
use os_analyze::aggregates::{self, live};
use os_model::CrawlConfig;
use std::time::{Duration, Instant};

// spec: UI-145
#[test]
#[ignore = "Benchmark: 5 Mio. URLs; auf ruhiger Referenzhardware ausführen"]
fn five_million_precomputed_sidebar_reads() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture::create(dir.path());
    file.conn().execute_batch("WITH RECURSIVE n(i) AS (VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i<5000000)
        INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,crawl_depth,folder_depth,url_length,
         content_class,indexability,status_code,response_time_ms,change_seq)
        SELECT i,'https://site.test/d'||(i%100)||'/'||i,'https://site.test/'||i,'https://site.test',1,'done','hyperlink',i%13,1,40,
         'HTML','Indexable',200,i%12000,1 FROM n;").unwrap();
    file.set_meta("change_seq", "1").unwrap();
    let build = Instant::now();
    live::refresh(&file, &CrawlConfig::default()).unwrap();
    eprintln!("aggregate build: {:?}", build.elapsed());
    for _ in 0..5 {
        let start = Instant::now();
        live::refresh(&file, &CrawlConfig::default()).unwrap();
        let tree = aggregates::path_tree(&file, "", 0, 500, 4).unwrap();
        let depths = aggregates::depths(&file).unwrap();
        let times = aggregates::response_times(&file, false).unwrap();
        assert_eq!(tree[0].row.counts.count, 5_000_000);
        assert_eq!(times.0.iter().sum::<u64>(), 5_000_000);
        assert_eq!(depths.len(), 10);
        assert!(start.elapsed() <= Duration::from_secs(1));
        let start = Instant::now();
        let _ = aggregates::depths(&file).unwrap();
        assert!(start.elapsed() <= Duration::from_millis(200));
    }
}
