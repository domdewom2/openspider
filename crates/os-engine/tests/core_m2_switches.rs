#[path = "core_support.rs"]
mod support;
use os_engine::testing::{crawl_fixture, crawl_fixture_from};
use os_model::CrawlConfig;
use support::*;

// spec: CRAWL-053 [M2]
#[tokio::test]
async fn shared_uncrawlable_catalog_keeps_non_anchor_handlers_out_of_the_frontier() {
    for enabled in [false, true] {
        let mut cfg = CrawlConfig::default();
        cfg.spider.store_uncrawlable_links = enabled;
        let fx = crawl_fixture_from("crawl-m2-switches/crawl-uncrawlable", "/", cfg).await;
        let requests = requested_paths(&fx, None);
        assert!(requests.contains(&"/ordinary".into()));
        assert!(!requests.iter().any(|p| p.ends_with("-navigation")));
        assert_eq!(
            scalar(&fx, "SELECT count(*) FROM links WHERE crawlable = 0"),
            if enabled { 3 } else { 0 }
        );
    }
}

// spec: CRAWL-004
#[tokio::test]
async fn fragment_option_leaves_legacy_hashbangs_to_the_ajax_mode() {
    let mut cfg = CrawlConfig::default();
    cfg.advanced.crawl_fragments = true;
    let fx = crawl_fixture_from("crawl-m2-switches/crawl-resources", "/hashbang", cfg).await;
    assert_eq!(
        scalar(&fx, "SELECT count(*) FROM urls WHERE url LIKE '%#!%'"),
        0
    );
    assert_eq!(
        scalar(
            &fx,
            "SELECT count(*) FROM urls WHERE url LIKE '%/base#ordinary' AND status_code = 200"
        ),
        1
    );
    assert_eq!(
        requested_paths(&fx, None)
            .iter()
            .filter(|p| *p == "/base")
            .count(),
        1
    );
}

// spec: CRAWL-004
#[tokio::test]
async fn fragments_share_one_fetch_even_when_discovered_after_the_base() {
    for enabled in [false, true] {
        let mut cfg = CrawlConfig::default();
        cfg.advanced.crawl_fragments = enabled;
        cfg.speed.threads = 4;
        let fx = crawl_fixture_from("crawl-m2-switches/crawl-resources", "/fragments", cfg).await;
        assert_eq!(
            requested_paths(&fx, None)
                .iter()
                .filter(|p| *p == "/base")
                .count(),
            1
        );
        assert_eq!(
            scalar(&fx, "SELECT count(*) FROM urls WHERE url LIKE '%#%'"),
            if enabled { 4 } else { 0 }
        );
        if enabled {
            assert_eq!(scalar(&fx, "SELECT count(*) FROM urls WHERE url LIKE '%/base#%' AND status_code = 200 AND title_1 = 'Shared base' AND h1_1 = 'Shared heading'"), 4);
            assert_eq!(
                scalar(
                    &fx,
                    "SELECT count(*) FROM url_ext WHERE key = 'crawl.fragment_base'"
                ),
                4
            );
        }
        assert!(requested_paths(&fx, None).contains(&"/child".into()));
    }
}

// spec: CRAWL-010
#[tokio::test]
async fn outside_folder_is_checked_once_without_following_and_resources_are_independent() {
    for enabled in [false, true] {
        let mut cfg = CrawlConfig::default();
        cfg.scope.subfolder_only = true;
        cfg.scope.crawl_outside_start_folder = enabled;
        cfg.spider.external.crawl = false;
        let fx = crawl_fixture_from("crawl-m2-switches/crawl-resources", "/blog/", cfg).await;
        let reqs = requested_paths(&fx, None);
        assert_eq!(reqs.contains(&"/shop/".into()), enabled);
        assert!(!reqs.contains(&"/shop/product".into()));
        assert!(reqs.contains(&"/outside.png".into()));
        let shop = row_of(&fx, &format!("{}/shop/", fx.site.base_url()));
        assert!(shop.is_internal);
        assert_eq!(shop.in_tables, enabled);
        assert_eq!(
            scalar(
                &fx,
                "SELECT count(*) FROM links WHERE target_raw = '/shop/'"
            ),
            1
        );
    }
}

// spec: CRAWL-020
#[tokio::test]
async fn assume_html_only_applies_when_content_type_is_absent() {
    for enabled in [false, true] {
        let mut cfg = CrawlConfig::default();
        cfg.advanced.assume_html = enabled;
        let fx = crawl_fixture_from(
            "crawl-m2-switches/crawl-resources",
            "/nocontenttype",
            cfg.clone(),
        )
        .await;
        let reqs = requested_paths(&fx, None);
        assert_eq!(reqs.contains(&"/only-missing".into()), enabled);
        assert_eq!(
            scalar(
                &fx,
                "SELECT count(*) FROM urls WHERE title_1 = 'Missing type' AND content_type IS NULL"
            ),
            i64::from(enabled)
        );
        let fx = crawl_fixture_from("crawl-m2-switches/crawl-resources", "/wrongtype", cfg).await;
        assert!(!requested_paths(&fx, None).contains(&"/only-wrong".into()));
    }
}

// spec: CRAWL-042
// spec: CRAWL-047
// spec: CRAWL-048
// spec: CRAWL-050
// spec: CRAWL-051
#[tokio::test]
async fn metadata_and_resources_obey_independent_store_crawl_switches() {
    for (store, crawl) in [(true, false), (false, true), (false, false), (true, true)] {
        let mut cfg = CrawlConfig::default();
        let sw = os_model::StoreCrawl { store, crawl };
        cfg.spider.pagination = sw;
        cfg.spider.hreflang = sw;
        cfg.spider.mobile_alternate = sw;
        cfg.spider.iframes = sw;
        cfg.spider.media = sw;
        let fx = crawl_fixture("crawl-m2-switches/crawl-resources", cfg).await;
        let reqs = requested_paths(&fx, None);
        for target in [
            "/next",
            "/en",
            "/mobile",
            "/frame",
            "/video.mp4",
            "/clip.webm",
            "/captions.vtt",
            "/audio.mp3",
            "/audio.ogg",
        ] {
            assert_eq!(
                reqs.contains(&target.into()),
                crawl,
                "{target}, store={store}, crawl={crawl}"
            );
            assert_eq!(
                scalar(
                    &fx,
                    &format!("SELECT count(*) FROM links WHERE target_raw = '{target}'")
                ),
                i64::from(store && (target != "/en" || crawl)),
                "{target}"
            );
        }
        assert!(reqs.contains(&"/poster.png".into()));
        assert_eq!(
            scalar(&fx, "SELECT count(*) FROM links WHERE link_type = 'Iframe'"),
            i64::from(store)
        );
    }
}

// spec: CRAWL-053 [M2]
#[tokio::test]
async fn uncrawlable_targets_are_optional_and_never_fetched() {
    for enabled in [false, true] {
        let mut cfg = CrawlConfig::default();
        cfg.spider.store_uncrawlable_links = enabled;
        let fx = crawl_fixture("crawl-m2-switches/crawl-resources", cfg).await;
        let reqs = requested_paths(&fx, None);
        for target in ["/span", "/blocked", "/onclick", "/js"] {
            assert!(!reqs.contains(&target.into()), "{target}");
        }
        assert!(reqs.contains(&"/ok".into()));
        assert_eq!(
            scalar(&fx, "SELECT count(*) FROM links WHERE crawlable = 0"),
            if enabled { 7 } else { 0 }
        );
        assert_eq!(
            scalar(
                &fx,
                "SELECT count(*) FROM urls WHERE url LIKE '%/span' AND status_code IS NOT NULL"
            ),
            0
        );
    }
}

// spec: CRAWL-061
// spec: CRAWL-063
#[tokio::test]
async fn extraction_switches_and_byte_exact_compressed_html() {
    let mut cfg = CrawlConfig::default();
    cfg.extraction.page_titles = false;
    cfg.extraction.h1 = false;
    cfg.extraction.h2 = false;
    cfg.extraction.meta_description = false;
    cfg.extraction.meta_keywords = false;
    cfg.extraction.hash = false;
    cfg.extraction.word_count = false;
    cfg.extraction.text_ratio = false;
    cfg.extraction.store_html = true;
    let fx = crawl_fixture("crawl-m2-switches/crawl-resources", cfg).await;
    let id: i64 = fx
        .file
        .conn()
        .query_row(
            "SELECT id FROM urls WHERE url = ?1",
            [format!("{}/", fx.site.base_url())],
            |r| r.get(0),
        )
        .unwrap();
    let html = os_store::queries::stored_html(&fx.file, os_model::UrlId(id))
        .unwrap()
        .unwrap();
    assert_eq!(
        html.bytes,
        include_bytes!("../../../tests/fixtures/crawl-m2-switches/crawl-resources/index.html")
    );
    assert_eq!(html.charset.as_deref(), Some("UTF-8"));
    assert!(
        scalar(
            &fx,
            "SELECT count(*) FROM page_html WHERE compression = 'zstd'"
        ) >= 1
    );
    assert_eq!(scalar(&fx,"SELECT count(*) FROM page_html h JOIN urls u ON u.id = h.url_id WHERE u.content_class != 'HTML'"),0);
    assert_eq!(scalar(&fx, "SELECT count(*) FROM urls WHERE title_1 IS NOT NULL OR h1_1 IS NOT NULL OR h2_1 IS NOT NULL OR meta_desc_1 IS NOT NULL OR meta_kw_1 IS NOT NULL OR hash_md5 IS NOT NULL OR word_count IS NOT NULL OR text_ratio IS NOT NULL"), 0);
    let fx = crawl_fixture("crawl-m2-switches/crawl-resources", CrawlConfig::default()).await;
    assert_eq!(scalar(&fx, "SELECT count(*) FROM page_html"), 0);
}

// spec: CRAWL-065
#[tokio::test]
async fn disabled_directives_cannot_affect_indexability_or_following() {
    for (route, meta) in [("/meta-nofollow", true), ("/header-nofollow", false)] {
        let mut cfg = CrawlConfig::default();
        cfg.spider.follow_internal_nofollow = false;
        let fx =
            crawl_fixture_from("crawl-m2-respect/crawl-indexability", route, cfg.clone()).await;
        assert!(!requested_paths(&fx, None).contains(&"/from-nofollow".into()));
        if meta {
            cfg.extraction.meta_robots = false;
        } else {
            cfg.extraction.x_robots_tag = false;
        }
        let fx = crawl_fixture_from("crawl-m2-respect/crawl-indexability", route, cfg).await;
        assert!(requested_paths(&fx, None).contains(&"/from-nofollow".into()));
        assert_eq!(scalar(&fx, "SELECT count(*) FROM directives"), 0);
        assert_eq!(
            scalar(
                &fx,
                "SELECT count(*) FROM urls WHERE indexability_status = 'noindex'"
            ),
            0
        );
    }
}

// spec: CRAWL-091
// spec: CRAWL-092
// spec: CRAWL-094
// spec: TECH-024
// spec: TECH-067
#[tokio::test]
async fn respect_hides_only_matching_pages_and_still_follows_their_children() {
    for option in ["noindex", "canonical", "self_refresh"] {
        let mut cfg = CrawlConfig::default();
        cfg.advanced.respect_noindex = option == "noindex";
        cfg.advanced.respect_canonical = option == "canonical";
        cfg.advanced.respect_self_ref_meta_refresh = option == "self_refresh";
        let fx = crawl_fixture("crawl-m2-respect/crawl-indexability", cfg).await;
        for target in [
            "/from-noindex",
            "/from-header",
            "/from-canonical",
            "/from-refresh",
        ] {
            assert!(requested_paths(&fx, None).contains(&target.into()));
        }
        assert_eq!(
            scalar(
                &fx,
                "SELECT count(*) FROM urls WHERE in_tables = 1 AND url LIKE '%/self-canonical'"
            ),
            1
        );
        assert_eq!(scalar(&fx, &format!("SELECT count(*) FROM url_flags f JOIN urls u ON u.id = f.url_id WHERE flag = 'respect_hidden' AND value = '{option}' AND u.in_tables = 0")), match option {"noindex" => 2, "canonical" => 1, _ => 0});
        assert_eq!(
            scalar(
                &fx,
                "SELECT count(*) FROM urls WHERE in_tables = 0 AND url LIKE '%/self-refresh'"
            ),
            0
        );
        // TECH-068 ist führend für CRAWL-094: Indexierbarkeit ändern, Sichtbarkeit erhalten.
        assert_eq!(scalar(&fx, "SELECT count(*) FROM urls WHERE url LIKE '%/self-refresh' AND indexability_status = 'Self Referencing Meta Refresh'"), i64::from(option == "self_refresh"));
    }
}

// spec: CRAWL-062
// spec: CRAWL-051
#[tokio::test]
async fn header_alternates_are_discovered_and_headers_are_optional() {
    let mut cfg = CrawlConfig::default();
    cfg.extraction.http_headers = true;
    cfg.spider.mobile_alternate.crawl = true;
    let fx = crawl_fixture_from("crawl-m2-switches/crawl-resources", "/headers", cfg).await;
    assert!(requested_paths(&fx, None).contains(&"/mobile-header".into()));
    assert_eq!(
        scalar(
            &fx,
            "SELECT count(*) FROM http_headers WHERE lower(name) = 'cache-control'"
        ),
        1
    );
}

// spec: CRAWL-062
#[tokio::test]
async fn cookies_keep_every_set_cookie_and_attribute_and_headers_keep_duplicates() {
    for enabled in [false, true] {
        let mut cfg = CrawlConfig::default();
        cfg.extraction.cookies = enabled;
        cfg.extraction.http_headers = enabled;
        let fx = crawl_fixture_from("crawl-m2-switches/crawl-resources", "/cookies", cfg).await;
        assert_eq!(
            scalar(&fx, "SELECT count(*) FROM cookies"),
            if enabled { 3 } else { 0 }
        );
        if enabled {
            let cookie: (String,String,String,String,i64,bool,bool,String) = fx.file.conn().query_row("SELECT name,value,domain,path,max_age,secure,http_only,same_site FROM cookies WHERE position = 0",[],|r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?))).unwrap();
            assert_eq!(
                cookie,
                (
                    "a".into(),
                    "first".into(),
                    "site.test".into(),
                    "/".into(),
                    123,
                    true,
                    true,
                    "Lax".into()
                )
            );
            assert_eq!(scalar(&fx,"SELECT count(*) FROM cookies WHERE origin = 'http' AND expires = 'Wed, 21 Oct 2037 07:28:00 GMT'"),1);
            let values: Vec<String> = fx.file.conn().prepare("SELECT value FROM http_headers WHERE lower(name) = 'x-fixture' ORDER BY position").unwrap().query_map([],|r| r.get(0)).unwrap().map(Result::unwrap).collect();
            assert_eq!(values, ["first", "second", "third"]);
        }
    }
}

// spec: CRAWL-004
#[tokio::test]
async fn fragment_list_seeds_keep_distinct_rows_and_deduplicate_the_http_fetch() {
    let site = os_testsite::TestSite::start("crawl-m2-switches/crawl-resources")
        .await
        .unwrap();
    let deps = os_engine::testing::test_deps();
    let mut cfg = CrawlConfig::default();
    cfg.advanced.crawl_fragments = true;
    let fetcher = os_engine::testing::site_fetcher(&site, &cfg, &deps);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("list.ospider");
    os_engine::start_crawl(
        fetcher,
        os_engine::CrawlRequest {
            config: cfg,
            source: os_engine::CrawlSource::List {
                items: vec![
                    site.url("/base#one"),
                    site.url("/base#two"),
                    site.url("/base#one"),
                ],
                label: "fragments".into(),
            },
            file: path.clone(),
            project: "fixture".into(),
            name: "list".into(),
        },
        deps,
    )
    .await
    .unwrap()
    .wait()
    .await
    .unwrap();
    assert_eq!(
        site.requests()
            .iter()
            .filter(|r| r.path() == "/base")
            .count(),
        1
    );
    let file = os_store::CrawlFile::open(&path, os_store::OpenMode::ReadOnly).unwrap();
    assert_eq!(file.conn().query_row("SELECT count(*) FROM urls WHERE url LIKE '%#%' AND in_tables = 1 AND status_code = 200",[],|r|r.get::<_,i64>(0)).unwrap(),2);
    assert_eq!(
        file.conn()
            .query_row(
                "SELECT value FROM crawl_counters WHERE key = 'list_duplicates'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
}

// spec: CRAWL-091
#[tokio::test]
async fn later_discovery_cannot_show_a_hidden_page_initially_fetched_without_storage() {
    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = 1;
    cfg.advanced.respect_noindex = true;
    cfg.spider.hreflang = os_model::StoreCrawl {
        store: false,
        crawl: true,
    };
    let fx = crawl_fixture_from("crawl-m2-switches/crawl-resources", "/rediscovery", cfg).await;
    assert_eq!(scalar(&fx,"SELECT count(*) FROM urls WHERE url LIKE '%/noindex' AND status_code = 200 AND in_tables = 0"),1);
    assert_eq!(
        scalar(
            &fx,
            "SELECT count(*) FROM url_flags WHERE flag = 'respect_hidden' AND value = 'noindex'"
        ),
        1
    );
}

// spec: CRAWL-063
#[tokio::test]
async fn stored_html_preserves_original_non_utf8_bytes_before_charset_conversion() {
    let mut cfg = CrawlConfig::default();
    cfg.extraction.store_html = true;
    let fx = crawl_fixture_from("crawl-m2-switches/crawl-resources", "/latin1.html", cfg).await;
    let id: i64 = fx
        .file
        .conn()
        .query_row(
            "SELECT id FROM urls WHERE url LIKE '%/latin1.html'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let html = os_store::queries::stored_html(&fx.file, os_model::UrlId(id))
        .unwrap()
        .unwrap();
    assert_eq!(
        html.bytes,
        include_bytes!("../../../tests/fixtures/crawl-m2-switches/crawl-resources/latin1.html")
    );
    assert_eq!(html.charset.as_deref(), Some("windows-1252"));
}
