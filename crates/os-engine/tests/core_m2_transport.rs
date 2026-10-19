use os_engine::testing::{sim_deps, MemorySink, SimFetcher, SimSite};
use os_engine::{start_crawl, CrawlRequest, CrawlSource};
use os_fetch::{Fetch, FetchRequest, FetchResult};
use os_model::CrawlConfig;
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};
struct Probe {
    inner: Arc<SimFetcher>,
    limits: Mutex<Vec<(u64, u64, Option<u64>)>>,
}
impl Fetch for Probe {
    fn fetch(&self, req: FetchRequest) -> Pin<Box<dyn Future<Output = FetchResult> + Send + '_>> {
        if req.url.path() == "/" {
            self.limits.lock().unwrap().push((
                req.max_body_bytes,
                req.body_limit.max_decoded_bytes,
                req.body_limit.abort_media_after,
            ));
        }
        self.inner.fetch(req)
    }
}
// spec: CRAWL-042
// spec: CRAWL-075
#[tokio::test(start_paused = true)]
async fn decoded_body_and_media_limits_are_passed_to_the_fetch_contract_including_unlimited_zero() {
    for kb in [0, 1, 51_200] {
        let mut cfg = CrawlConfig::default();
        cfg.limits.max_page_kb = kb;
        let probe = Arc::new(Probe {
            inner: SimFetcher::new(SimSite::new().page(
                "https://site.test/",
                include_str!(
                    "../../../tests/fixtures/crawl-m2-switches/crawl-resources/index.html"
                ),
            )),
            limits: Mutex::new(Vec::new()),
        });
        let dir = tempfile::tempdir().unwrap();
        let sink = MemorySink::new();
        start_crawl(
            probe.clone(),
            CrawlRequest {
                config: cfg,
                source: CrawlSource::Spider {
                    start_url: "https://site.test/".into(),
                },
                file: dir.path().join("probe.ospider"),
                project: "fixture".into(),
                name: "limits".into(),
            },
            sim_deps(&sink),
        )
        .await
        .unwrap()
        .wait()
        .await
        .unwrap();
        assert_eq!(
            *probe.limits.lock().unwrap(),
            [(kb * 1024, kb * 1024, Some(64 * 1024))]
        );
    }
}

// spec: CRAWL-073
#[tokio::test(start_paused = true)]
async fn encoded_url_length_has_an_inclusive_boundary_and_zero_disables_it() {
    let prefix = "https://site.test/";
    let urls: Vec<_> = [99, 100, 101]
        .map(|len| format!("{prefix}{}", "a".repeat(len - prefix.len())))
        .into_iter()
        .collect();
    let body = urls
        .iter()
        .map(|url| format!("<a href='{url}'>boundary</a>"))
        .collect::<String>();
    for limit in [0, 100] {
        let mut cfg = CrawlConfig::default();
        cfg.limits.max_url_length = limit;
        let mut site = SimSite::new().page(prefix, &body);
        for url in &urls {
            site = site.page(url, "boundary");
        }
        let fetcher = SimFetcher::new(site);
        let dir = tempfile::tempdir().unwrap();
        let sink = MemorySink::new();
        start_crawl(
            fetcher.clone(),
            CrawlRequest {
                config: cfg,
                source: CrawlSource::Spider {
                    start_url: prefix.into(),
                },
                file: dir.path().join("boundary.ospider"),
                project: "fixture".into(),
                name: "boundary".into(),
            },
            sim_deps(&sink),
        )
        .await
        .unwrap()
        .wait()
        .await
        .unwrap();
        let requested: Vec<_> = fetcher
            .page_requests()
            .iter()
            .map(|r| r.url.clone())
            .collect();
        assert!(requested.contains(&urls[0]) && requested.contains(&urls[1]));
        assert_eq!(requested.contains(&urls[2]), limit == 0);
    }
}
