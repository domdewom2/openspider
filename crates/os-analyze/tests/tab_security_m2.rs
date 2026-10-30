use os_analyze::{
    tabs,
    views::{self, ColumnSelection, ViewQuery},
};
use os_engine::testing::{crawl_fixture_from, FixtureCrawl};
use os_model::{CrawlConfig, Value};

async fn fixture(scheme: &str) -> FixtureCrawl {
    let mut cfg = CrawlConfig::default();
    cfg.spider.external.crawl = false;
    crawl_fixture_from("onpage-m2-security", &format!("{scheme}://site.test/"), cfg).await
}
fn addresses(fx: &FixtureCrawl, filter: &str) -> Vec<String> {
    let mut q = ViewQuery::new("Security", filter);
    q.columns = ColumnSelection::Named(vec!["Address".into()]);
    q.limit = 500;
    views::query_view(&fx.file, &CrawlConfig::default(), &q)
        .unwrap()
        .rows
        .into_iter()
        .map(|r| match &r[0] {
            Value::Text(s) => s.clone(),
            _ => panic!("address"),
        })
        .collect()
}

// spec: ONPAGE-033, ONPAGE-034, ONPAGE-035, ONPAGE-036, ONPAGE-037
#[tokio::test]
async fn stored_page_facts_and_individual_links_drive_the_live_filters() {
    for scheme in ["http", "https"] {
        let fx = fixture(scheme).await;
        for (filter, path, want) in [
            ("Mixed Content", "mixed.html", scheme == "https"),
            ("Mixed Content", "link-only.html", false),
            ("Mixed Content", "not-resources.html", false),
            ("Form URL Insecure", "form-insecure.html", true),
            ("Form URL Insecure", "form-relative.html", scheme == "http"),
            ("Form on HTTP URL", "form-secure.html", scheme == "http"),
            ("Unsafe Cross-Origin Links", "xo.html", true),
            ("Unsafe Cross-Origin Links", "xo-safe.html", false),
            ("Protocol-Relative Resource Links", "relative.html", true),
            (
                "Protocol-Relative Resource Links",
                "not-resources.html",
                false,
            ),
        ] {
            let got = addresses(&fx, filter);
            assert_eq!(
                got.contains(&format!("{scheme}://site.test/https/{path}")),
                want,
                "{scheme} {filter}: {got:?}"
            );
        }
        let source_id: i64 = fx
            .file
            .conn()
            .query_row(
                "SELECT id FROM urls WHERE url=?1",
                [format!("{scheme}://site.test/https/xo.html")],
                |r| r.get(0),
            )
            .unwrap();
        let outlinks = os_store::queries::outlinks(
            fx.file.conn(),
            os_model::UrlId(source_id),
            os_store::queries::PageReq::default(),
        )
        .unwrap();
        let unsafe_link = outlinks
            .items
            .iter()
            .find(|l| l.target_address == "https://external.test/x")
            .unwrap();
        assert_eq!(unsafe_link.target_attr.as_deref(), Some("_BLANK"));
        // A stored finding on a non-2xx page or non-HTML resource cannot enter page filters.
        fx.file
            .conn()
            .execute(
                "UPDATE urls SET status_code=404 WHERE url LIKE '%/xo.html'",
                [],
            )
            .unwrap();
        assert!(addresses(&fx, "Unsafe Cross-Origin Links").is_empty());
        fx.file
            .conn()
            .execute(
                "UPDATE urls SET content_class='Images' WHERE url LIKE '%/form-insecure.html'",
                [],
            )
            .unwrap();
        assert!(!addresses(&fx, "Form URL Insecure")
            .iter()
            .any(|u| u.ends_with("form-insecure.html")));
    }
}

// spec: ONPAGE-038, ONPAGE-039, ONPAGE-040, ONPAGE-041, ONPAGE-042, ONPAGE-043
#[tokio::test]
async fn header_populations_distinguish_hsts_from_the_four_2xx_filters() {
    let fx = fixture("https").await;
    for filter in [
        "Missing HSTS Header",
        "Missing Content-Security-Policy Header",
        "Missing X-Content-Type-Options Header",
        "Missing X-Frame-Options Header",
        "Missing Secure Referrer-Policy Header",
    ] {
        let got = addresses(&fx, filter);
        assert_eq!(
            got.contains(&"https://site.test/redirect".into()),
            filter == "Missing HSTS Header",
            "{filter} {got:?}"
        );
        assert!(
            !got.contains(&"https://site.test/headers".into()),
            "{filter}"
        );
        assert!(!got
            .iter()
            .any(|u| u.contains("cdn.test") || u.contains("external.test")));
    }
    assert!(addresses(&fx, "Missing Content-Security-Policy Header")
        .contains(&"https://site.test/image.jpg".into()));
    assert!(addresses(&fx, "Bad Content Type").contains(&"https://site.test/image.jpg".into()));
    fx.file
        .conn()
        .execute(
            "UPDATE urls SET status_code=NULL WHERE url LIKE '%/redirect'",
            [],
        )
        .unwrap();
    assert!(!addresses(&fx, "Missing HSTS Header").contains(&"https://site.test/redirect".into()));
    let http = fixture("http").await;
    assert!(addresses(&http, "Missing HSTS Header").is_empty());
    let mut q = ViewQuery::new("Security", "Bad Content Type");
    q.columns = ColumnSelection::Named(vec!["Address".into(), "Expected Content Type".into()]);
    let page = views::query_view(&fx.file, &CrawlConfig::default(), &q).unwrap();
    assert!(page
        .rows
        .iter()
        .any(|r| r[0] == Value::from("https://site.test/image.jpg")
            && r[1] == Value::from("image/jpeg")));
}

// spec: ONPAGE-030
#[test]
fn security_tab_keeps_numbered_directive_columns_and_registers_all_filters() {
    let tab = tabs::security::tab();
    assert_eq!(tab.filters.len(), 14);
    for key in ["Canonical Link Element", "Meta Robots", "X-Robots-Tag"] {
        assert!(tab
            .columns
            .iter()
            .any(|c| c.key == key && matches!(c.sql, os_analyze::ColumnSql::Multi { .. })));
    }
}
