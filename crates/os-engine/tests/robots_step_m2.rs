use os_engine::testing::{crawl_dir_with, crawl_fixture_from};
use os_model::CrawlConfig;

// spec: CRAWL-102, CRAWL-098
#[tokio::test]
async fn custom_files_override_live_errors_per_host_and_roundtrip_with_delay_note() {
    for applied in [false, true] {
        let mut cfg = CrawlConfig::default();
        cfg.robots.retries = 0;
        cfg.robots.respect_crawl_delay = applied;
        cfg.robots.custom_files.insert(
            "site.test".into(),
            "User-agent: *\nDisallow: /allowed\nCrawl-delay: 0.5\n".into(),
        );
        cfg.robots.custom_files.insert(
            "unreachable.test".into(),
            "User-agent: *\nAllow: /\n".into(),
        );
        let fx = crawl_fixture_from("crawl-m2-robots", "http://site.test/", cfg).await;
        assert_eq!(fx.site.count("site.test", "/robots.txt"), 0);
        assert_eq!(fx.site.count("site.test", "/allowed"), 0);
        assert_eq!(fx.site.count("site.test", "/blocked"), 1);
        assert_eq!(fx.site.count("other.test", "/robots.txt"), 1);
        assert_eq!(fx.site.count("other.test", "/blocked"), 0);
        let note: String = fx
            .file
            .conn()
            .query_row(
                "SELECT note FROM robots_files WHERE origin='http://site.test'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            note,
            format!(
                "Crawl-delay: 0.5 s ({})",
                if applied {
                    "angewandt"
                } else {
                    "nicht angewandt"
                }
            )
        );
        let mut stored = fx.file.config().unwrap();
        assert_eq!(stored.robots.custom_files.len(), 2);
        stored.network.resolve.clear();
        let fx = crawl_fixture_from(
            "crawl-m2-robots",
            "http://site.test/unreachable-start",
            stored,
        )
        .await;
        assert_eq!(fx.site.count("unreachable.test", "/robots.txt"), 0);
        assert_eq!(fx.site.count("unreachable.test", "/blocked"), 1);
    }
}

// spec: ONPAGE-026, CRAWL-100, CRAWL-101
#[tokio::test]
async fn a_closed_external_port_is_visible_with_status_zero_in_unknown_population() {
    let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!(
        "http://localhost:{}/closed",
        socket.local_addr().unwrap().port()
    );
    drop(socket);
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("scenario.toml"),format!("[[route]]\npath = \"/\"\nbody = '<html><a href=\"{address}\">closed</a></html>'\n[[route]]\npath = \"/robots.txt\"\nbody = 'User-agent: *'\n")).unwrap();
    for show in [false, true] {
        let mut cfg = CrawlConfig::default();
        cfg.robots.retries = 0;
        cfg.robots.show_blocked_external = show;
        cfg.robots.unreachable_policy = if show {
            os_model::RobotsUnreachablePolicy::BlockHost
        } else {
            os_model::RobotsUnreachablePolicy::AllowAll
        };
        let fx = crawl_dir_with(dir.path(), "/", cfg, os_engine::testing::test_deps()).await;
        // Persisted population and predicate of External:Unknown (tabs/external.rs, internal::unknown_filter).
        let (url,status):(String,i64)=fx.file.conn().query_row("SELECT u.url,u.status_code FROM urls u WHERE u.is_internal=0 AND u.in_tables=1 AND u.content_class='Unknown'",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
        assert_eq!(url, address);
        assert_eq!(status, 0);
    }
}
