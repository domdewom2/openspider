//! Engine-to-Chromium proof. These tests also run headless in linux-core.
use os_engine::testing::{crawl_fixture_with, test_deps, FixtureCrawl};
use os_model::{CrawlConfig, RenderingMode};

fn cfg() -> CrawlConfig {
    let mut c = CrawlConfig::default();
    c.rendering.mode = RenderingMode::JavaScript;
    c.rendering.ajax_timeout_s = 0.3;
    c.rendering.screenshots.enabled = false;
    c.limits.max_depth = Some(0);
    c
}
fn int(fx: &FixtureCrawl, sql: &str) -> i64 {
    fx.file.conn().query_row(sql, [], |r| r.get(0)).unwrap()
}
fn text(fx: &FixtureCrawl, sql: &str) -> String {
    fx.file.conn().query_row(sql, [], |r| r.get(0)).unwrap()
}
static BROWSER: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

// spec: CRAWL-078
// spec: CRAWL-079
// spec: CRAWL-039 [M2]
// spec: CRAWL-022 [P1]
// spec: ONPAGE-012 [P1]
// spec: CRAWL-062
// spec: CRAWL-063
// spec: CRAWL-082
// spec: CRAWL-084
// spec: CRAWL-115
// spec: CRAWL-116
// spec: ONPAGE-172
// spec: OUT-273
#[tokio::test]
async fn headless_engine_reuses_response_and_stores_browser_facts() {
    let _guard = BROWSER.lock().await;
    let mut c = cfg();
    c.extraction.store_html = true;
    c.extraction.store_rendered_html = true;
    c.extraction.cookies = true;
    c.rendering.screenshots.enabled = true;
    c.http.headers.push(os_model::HeaderEntry {
        name: "X-Engine-Fixture".into(),
        value: "same-identity".into(),
        host: None,
    });
    c.http.accept_language = Some("de-DE,de;q=0.9".into());
    let fx = crawl_fixture_with("crawl-m2-render", "/", c, test_deps()).await;
    assert_eq!(
        text(
            &fx,
            "SELECT title_1 FROM urls WHERE in_tables=1 AND url LIKE '%/'"
        ),
        "Rendered"
    );
    assert_eq!(
        text(
            &fx,
            "SELECT indexability FROM urls WHERE in_tables=1 AND url LIKE '%/'"
        ),
        "Non-Indexable"
    );
    let requests = fx.site.requests();
    assert_eq!(
        requests.iter().filter(|r| r.path_and_query == "/").count(),
        1,
        "main response reused"
    );
    assert!(!requests.iter().any(|r| r.path_and_query == "/blocked.js"));
    let image = requests
        .iter()
        .find(|r| r.path_and_query == "/lazy.png")
        .expect("lazy image loaded");
    let get = |name: &str| {
        image
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(get("X-Engine-Fixture"), Some("same-identity"));
    assert_eq!(get("Accept-Language"), Some("de-DE,de;q=0.9"));
    let main = requests.iter().find(|r| r.path_and_query == "/").unwrap();
    assert_eq!(
        get("User-Agent"),
        main.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("User-Agent"))
            .map(|(_, v)| v.as_str())
    );
    assert_eq!(
        int(
            &fx,
            "SELECT bytes FROM render_network WHERE url LIKE '%/lazy.png'"
        ),
        20000
    );
    assert_eq!(
        int(
            &fx,
            "SELECT COUNT(*) FROM render_blocked WHERE url LIKE '%/blocked.js' AND reason='robots'"
        ),
        1
    );
    assert!(int(&fx,"SELECT COUNT(*) FROM render_console WHERE text LIKE '%fixture-js-error%' AND level='error'")>=1);
    assert_eq!(
        int(
            &fx,
            "SELECT COUNT(*) FROM cookies WHERE name='js_session' AND origin='javascript'"
        ),
        1
    );
    assert_eq!(
        int(
            &fx,
            "SELECT COUNT(*) FROM render_pages WHERE status='ok' AND html IS NOT NULL"
        ),
        1
    );
    assert!(int(&fx, "SELECT COUNT(*) FROM screenshots") >= 1);
    assert_eq!(
        int(
            &fx,
            "SELECT COUNT(*) FROM image_instances WHERE real_w>0 AND display_w>0"
        ),
        1
    );
    let root = fx
        .file
        .conn()
        .query_row(
            "SELECT id FROM urls WHERE in_tables=1 AND url LIKE '%/'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap();
    let raw = os_store::queries::stored_html(&fx.file, os_model::UrlId(root))
        .unwrap()
        .unwrap();
    let dom = os_store::queries::stored_rendered_html(&fx.file, os_model::UrlId(root))
        .unwrap()
        .unwrap();
    assert!(String::from_utf8_lossy(&raw.bytes).contains("Removed-Marker"));
    let html = String::from_utf8_lossy(&dom.bytes);
    assert!(html.contains("Injected-Marker"));
    assert!(
        html.contains(r#"data-visible-cookies="js_session=active""#),
        "{html}"
    );
    assert_eq!(int(&fx, "SELECT COUNT(*) FROM cookies WHERE name='http_secret' AND http_only=1 AND origin='http'"),1);
}

// spec: CRAWL-016
#[tokio::test]
async fn navigation_is_a_redirect_but_pushstate_and_late_navigation_are_not() {
    let _guard = BROWSER.lock().await;
    for (path, redirects) in [
        ("/redirect", 1),
        ("/pushstate", 0),
        ("/late", 0),
        ("/meta-redirect", 0),
    ] {
        let fx = crawl_fixture_with("crawl-m2-render", path, cfg(), test_deps()).await;
        assert_eq!(
            int(
                &fx,
                "SELECT COUNT(*) FROM urls WHERE redirect_type='javascript'"
            ),
            redirects,
            "{path}"
        );
        if path == "/meta-redirect" {
            assert_eq!(
                int(
                    &fx,
                    "SELECT COUNT(*) FROM urls WHERE redirect_type='meta_refresh'"
                ),
                1
            );
        }
        if redirects == 1 {
            assert_eq!(int(&fx,"SELECT COUNT(*) FROM links WHERE link_type='JavaScript Redirect' AND link_origin='Rendered HTML'"),1);
        }
    }
}

// spec: CRAWL-004
#[tokio::test]
async fn hash_routes_receive_separate_render_passes() {
    let _guard = BROWSER.lock().await;
    let mut c = cfg();
    c.advanced.crawl_fragments = true;
    c.limits.max_depth = Some(1);
    let fx = crawl_fixture_with("crawl-m2-render", "/fragment", c, test_deps()).await;
    assert_eq!(int(&fx, "SELECT COUNT(*) FROM render_pages"), 3);
    let mut stmt = fx
        .file
        .conn()
        .prepare("SELECT title_1 FROM urls WHERE url LIKE '%/fragment%' ORDER BY url")
        .unwrap();
    let titles = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(titles, vec!["none", "#a", "#b"]);
}
