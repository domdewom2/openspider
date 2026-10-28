#[path = "cmd_catalog_fixture.rs"]
mod fixture;
use fixture::*;
use os_model::CrawlConfig;

// spec: CRAWL-145
#[test]
fn crawl_delay_is_opt_in_by_default_k2_76() {
    assert!(!CrawlConfig::default().robots.respect_crawl_delay);
}

// spec: CRAWL-145
// spec: UI-013
#[tokio::test]
async fn feed_reports_effective_crawl_delay_once_in_both_modes() {
    for (applied, seconds) in [(false, 1.0_f64), (true, 1.0), (false, 120.0), (true, 120.0)] {
        let mut cfg = CrawlConfig::default();
        cfg.robots.respect_crawl_delay = applied;
        let fx = Fx::config(cfg);
        fx.write(&format!("INSERT INTO robots_files(origin,robots_url,fetch_status,attempts,body) VALUES ('https://site.test','https://site.test/robots.txt','ok',1,'User-agent: *\nCrawl-delay: {seconds}\n')"));
        let session = fx
            .app
            .sessions()
            .current(&os_app::WindowId("main".into()))
            .unwrap();
        os_app::session::feed::refresh(&fx.app, &session)
            .await
            .unwrap();
        let tick = fx.app.assemble_tick().unwrap();
        let notices: Vec<_> = tick
            .notices
            .iter()
            .filter(|n| n.code == "crawl-delay")
            .collect();
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].args["seconds"], seconds);
        assert_eq!(
            notices[0].args["effectiveSeconds"],
            if applied { seconds.min(60.0) } else { 0.0 }
        );
        assert_eq!(notices[0].args["capped"], applied && seconds > 60.0);
        assert_eq!(notices[0].args["applied"], applied);
        os_app::session::feed::invalidate(&fx.app, &session.key);
        os_app::session::feed::refresh(&fx.app, &session)
            .await
            .unwrap();
        assert!(!fx
            .app
            .assemble_tick()
            .is_some_and(|t| t.notices.iter().any(|n| n.code == "crawl-delay")));
    }
}
