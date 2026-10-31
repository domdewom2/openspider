use os_engine::{start_crawl, testing, CrawlRequest, CrawlSource};
use os_model::CrawlConfig;
use std::{mem::size_of_val, sync::Arc};

// spec: AGENT-020 [M2]
// spec: CRAWL-001
#[test]
fn crawl_start_future_has_bounded_size_for_all_sources() {
    let deps = testing::test_deps();
    let config = CrawlConfig::default();
    let fetcher = Arc::new(
        os_mcp::target::ProtectedFetcher::new(
            os_fetch::FetchConfig::from_crawl_config(
                &config,
                deps.secrets.as_ref(),
                deps.clock.clone(),
            )
            .unwrap(),
            os_mcp::permissions::DomainPolicy::default(),
            true,
        )
        .unwrap(),
    );
    for source in [
        CrawlSource::Spider {
            start_url: "http://127.0.0.1/".into(),
        },
        CrawlSource::List {
            items: vec![],
            label: "size".into(),
        },
        CrawlSource::Serp {
            rows: vec![],
            label: "size".into(),
        },
    ] {
        let future = start_crawl(
            fetcher.clone(),
            CrawlRequest {
                config: config.clone(),
                source,
                file: "unused.ospider".into(),
                project: String::new(),
                name: String::new(),
            },
            deps.clone(),
        );
        let bytes = size_of_val(&future);
        eprintln!("Crawl start future: {bytes} bytes");
        assert!(
            bytes <= 16 * 1024,
            "Crawl start future: {bytes} bytes exceeds 16 KiB"
        );
    }
}
