//! Real crawl → mobile DOM facts → configured SQL views. Lighthouse scores are independent fixture inputs.
use os_analyze::{
    tabs::mobile,
    views::{self, ColumnSelection, ViewQuery},
};
use os_engine::testing::{
    crawl_fixture, crawl_fixture_from, crawl_fixture_with, test_deps, FixtureCrawl,
};
use os_model::{CrawlConfig, FilterState, UrlId, Value};
use serde_json::Value as Json;

fn cfg() -> CrawlConfig {
    let mut cfg = CrawlConfig::default();
    cfg.api.psi.enabled = true;
    cfg.api.psi.local = true;
    cfg.spider.external.crawl = false;
    cfg
}
async fn fixture() -> FixtureCrawl {
    crawl_fixture("tech-m2-mobile", cfg()).await
}
fn id(fx: &FixtureCrawl, path: &str) -> i64 {
    fx.file
        .conn()
        .query_row(
            "SELECT id FROM urls WHERE url=?1",
            [fx.site.url(&format!("/{path}"))],
            |r| r.get(0),
        )
        .unwrap()
}
fn put(fx: &FixtureCrawl, id: i64, name: &str, value: Json) {
    fx.file.conn().execute("INSERT OR REPLACE INTO api_values(url_id,source,metric,level,v_real,v_text,fetched_at) VALUES(?1,'psi',?2,'url',?3,?4,'2026-10-22T08:00:00Z')",
        (id, name, value.as_f64(), value.as_str())).unwrap();
}
fn success(fx: &FixtureCrawl, path: &str) -> i64 {
    let id = id(fx, path);
    put(fx, id, "PSI Status", Json::from("Success"));
    id
}
fn page(fx: &FixtureCrawl, cfg: &CrawlConfig, filter: &str) -> views::ViewPage {
    let mut q = ViewQuery::new("Mobile", filter);
    q.limit = 500;
    q.columns = ColumnSelection::All;
    views::query_view(&fx.file, cfg, &q).unwrap()
}
fn value<'a>(page: &'a views::ViewPage, id: i64, name: &str) -> &'a Value {
    let row = page.row_keys.iter().position(|u| u.0 == id).unwrap();
    let col = page.columns.iter().position(|n| n == name).unwrap();
    &page.rows[row][col]
}
fn keys(page: &views::ViewPage) -> Vec<i64> {
    page.row_keys.iter().map(|u| u.0).collect()
}
fn load_scores(fx: &FixtureCrawl) -> Json {
    let expected: Json = serde_json::from_str(include_str!(
        "../../../tests/fixtures/tech-m2-mobile/expected.json"
    ))
    .unwrap();
    for (path, cells) in expected.as_object().unwrap() {
        if path == "partial-target.html" || path == "js-flash-rendered.html" {
            continue;
        }
        let id = success(fx, path);
        for (name, value) in cells.as_object().unwrap() {
            if name != "Plugins" {
                put(fx, id, name, value.clone());
            }
        }
    }
    expected
}

// spec: TECH-231
#[tokio::test]
async fn population_and_nine_columns_include_only_internal_html_200_with_status_or_alternate() {
    let fx = fixture().await;
    let checked = success(&fx, "ok.html");
    put(&fx, checked, "Tap Targets", Json::from(50));
    let all = page(&fx, &cfg(), "All");
    assert_eq!(
        all.columns,
        [
            "Address",
            "PSI Status",
            "PSI Error",
            "Viewport",
            "Tap Targets",
            "Content Width",
            "Font Display",
            "Plugins",
            "Mobile Alternate Link"
        ]
    );
    assert_eq!(value(&all, checked, "Tap Targets"), &Value::Int(50));
    assert!(!keys(&all).contains(&id(&fx, "no-viewport.html")));
    for (field, val) in [
        ("is_internal", "0"),
        ("status_code", "201"),
        ("content_class", "'Image'"),
    ] {
        fx.file
            .conn()
            .execute(
                &format!("UPDATE urls SET {field}={val} WHERE id=?1"),
                [checked],
            )
            .unwrap();
        assert!(!keys(&page(&fx, &cfg(), "All")).contains(&checked));
        fx.file
            .conn()
            .execute(
                "UPDATE urls SET is_internal=1,status_code=200,content_class='HTML' WHERE id=?1",
                [checked],
            )
            .unwrap();
    }
    assert_eq!(
        mobile::column_tooltip("Font Display"),
        Some("Lesbare Schriftgrößen")
    );
}

async fn score_cases(filter: &str, column: &str) {
    let fx = fixture().await;
    let expected = load_scores(&fx);
    let all = page(&fx, &cfg(), "All");
    let filtered = page(&fx, &cfg(), filter);
    for (path, cells) in expected.as_object().unwrap() {
        if path == "partial-target.html" || path == "js-flash-rendered.html" {
            continue;
        }
        let id = id(&fx, path);
        let score = cells[column].as_i64().unwrap();
        assert_eq!(value(&all, id, column), &Value::Int(score), "{path}");
        assert_eq!(
            keys(&filtered).contains(&id),
            score < 100,
            "{path}: {filter}"
        );
    }
}
// spec: TECH-232
#[tokio::test]
async fn viewport_scores_include_scale_only_and_fixed_width_passes() {
    score_cases("Viewport Not Set", "Viewport").await;
}
// spec: TECH-233
#[tokio::test]
async fn target_geometry_fixtures_and_partial_scores_use_the_audit_verdict() {
    score_cases("Target Size", "Tap Targets").await;
    let fx = fixture().await;
    let id = success(&fx, "ok.html");
    put(&fx, id, "Tap Targets", Json::from(50));
    assert!(keys(&page(&fx, &cfg(), "Target Size")).contains(&id));
}
// spec: TECH-234
#[tokio::test]
async fn content_width_is_local_only_even_if_old_remote_values_exist() {
    score_cases("Content Not Sized Correctly", "Content Width").await;
    let fx = fixture().await;
    let id = success(&fx, "wide-content.html");
    put(&fx, id, "Content Width", Json::from(0));
    let mut remote = cfg();
    remote.api.psi.local = false;
    assert_eq!(
        value(&page(&fx, &remote, "All"), id, "Content Width"),
        &Value::Null
    );
    let result = page(&fx, &remote, "Content Not Sized Correctly");
    assert!(result.rows.is_empty());
    assert_eq!(
        views::state_of(&fx.file, &remote, "Mobile", "Content Not Sized Correctly").unwrap(),
        FilterState::NeedsConfig(mobile::LOCAL_ONLY.into())
    );
}
// spec: TECH-235
#[tokio::test]
async fn font_size_boundaries_follow_scores_and_missing_audits_have_a_hint() {
    score_cases("Illegible Font Size", "Font Display").await;
    let fx = fixture().await;
    success(&fx, "ok.html");
    let result = page(&fx, &cfg(), "Illegible Font Size");
    assert_eq!(
        views::state_of(&fx.file, &cfg(), "Mobile", "Illegible Font Size").unwrap(),
        FilterState::NeedsConfig(mobile::MISSING_AUDIT.into())
    );
    assert!(result.rows.is_empty());
}
// spec: TECH-236
// spec: INTEG-146
#[tokio::test]
async fn plugins_fall_back_to_real_crawl_dom_only_when_the_audit_is_absent() {
    score_cases("Unsupported Plugins", "Plugins").await;
    let fx = fixture().await;
    let flash = success(&fx, "flash.html");
    put(&fx, flash, "mobile.plugins_audit", Json::from(1));
    assert_eq!(
        value(&page(&fx, &cfg(), "All"), flash, "Plugins"),
        &Value::Null,
        "manual/error audit must not fall back"
    );
    put(&fx, flash, "Plugins", Json::from(100));
    assert_eq!(
        value(&page(&fx, &cfg(), "All"), flash, "Plugins"),
        &Value::Int(100),
        "audit has priority over DOM"
    );
}
// spec: TECH-237
// spec: INTEG-147
#[tokio::test]
async fn html_and_header_alternates_work_without_psi_and_respect_order_base_and_store_switch() {
    let fx = fixture().await;
    let mut off = cfg();
    off.api.psi.enabled = false;
    let all = page(&fx, &off, "Mobile Alternate Link");
    for (path, target, count) in [
        (
            "m-alternate.html",
            "https://m.site.test/page".to_string(),
            1,
        ),
        (
            "m-alternate-relative.html",
            "https://site.test/m/page".to_string(),
            2,
        ),
        ("m-alternate-header", fx.site.url("/m/header"), 1),
        ("html-before-header", fx.site.url("/m/html"), 2),
    ] {
        let id = id(&fx, path);
        assert_eq!(
            value(&all, id, "Mobile Alternate Link"),
            &Value::Text(target)
        );
        assert_eq!(mobile::alternate_count(&fx.file, UrlId(id)).unwrap(), count);
    }
    for path in [
        "hreflang-only.html",
        "rss-alternate.html",
        "body-alternate.html",
    ] {
        assert!(!keys(&all).contains(&id(&fx, path)), "{path}");
    }
    off.spider.mobile_alternate.store = false;
    assert!(page(&fx, &off, "All").rows.is_empty());
    let fx = crawl_fixture_from("tech-m2-mobile", "/m-alternate.html", off).await;
    assert_eq!(
        fx.file
            .conn()
            .query_row(
                "SELECT count(*) FROM url_ext WHERE key='mobile.alternate'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
}
// spec: TECH-230
#[tokio::test]
async fn disabled_groups_errors_and_missing_audits_never_become_passes() {
    let fx = fixture().await;
    let id = success(&fx, "flash.html");
    for (_, column) in mobile::FILTERS {
        put(&fx, id, column, Json::from(0));
    }
    let mut group_off = cfg();
    group_off.api.psi.metric_groups = vec!["lighthouse".into()];
    for off in [group_off, CrawlConfig::default()] {
        let all = page(&fx, &off, "All");
        for (filter, column) in mobile::FILTERS {
            assert_eq!(value(&all, id, column), &Value::Null);
            assert_eq!(
                views::state_of(&fx.file, &off, "Mobile", filter).unwrap(),
                FilterState::NeedsConfig(mobile::NOT_CONFIGURED.into())
            );
        }
        assert!(!page(&fx, &off, "Mobile Alternate Link").rows.is_empty());
    }
    for reason in [
        "Timeout",
        "LIGHTHOUSE_CRASHED",
        "500 API failure",
        "Quota exceeded: daily quota",
    ] {
        os_store::queries::api::record_error(
            fx.file.conn(),
            "psi",
            &fx.site.url("/flash.html"),
            "",
            reason,
            os_model::Timestamp::UNIX_EPOCH,
        )
        .unwrap();
        let all = page(&fx, &cfg(), "All");
        assert_eq!(value(&all, id, "PSI Status"), &Value::Text("Error".into()));
        assert_eq!(value(&all, id, "PSI Error"), &Value::Text(reason.into()));
        for (filter, column) in mobile::FILTERS {
            assert_eq!(value(&all, id, column), &Value::Null);
            assert!(!keys(&page(&fx, &cfg(), filter)).contains(&id));
        }
    }
}

struct RenderedPlugin;
impl os_engine::Renderer for RenderedPlugin {
    fn render<'a>(
        &'a self,
        url: &'a str,
        _: &'a os_engine::render_glue::RenderOpts,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<
                        os_engine::render_glue::RenderedPage,
                        os_engine::render_glue::RenderError,
                    >,
                > + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            Ok(os_engine::render_glue::RenderedPage {
                dom_html: include_str!(
                    "../../../tests/fixtures/tech-m2-mobile/js-flash-rendered.html"
                )
                .into(),
                navigation_url: Some(url.into()),
                ..Default::default()
            })
        })
    }
}
// spec: TECH-236
#[tokio::test]
async fn javascript_inserted_plugins_use_rendered_dom_when_rendering_is_on() {
    let mut c = cfg();
    c.rendering.mode = os_model::RenderingMode::JavaScript;
    let mut deps = test_deps();
    deps.renderer = Some(std::sync::Arc::new(RenderedPlugin));
    let fx = crawl_fixture_with("tech-m2-mobile", "/js-flash.html", c.clone(), deps).await;
    let id = success(&fx, "js-flash.html");
    assert_eq!(value(&page(&fx, &c, "All"), id, "Plugins"), &Value::Int(0));
}
