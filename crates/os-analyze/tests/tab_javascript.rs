use os_analyze::views::{self, ViewPage, ViewQuery};
use os_model::{
    time::{IdSource, SeededIds},
    CrawlConfig, CrawlMode, FilterState, RenderingMode, Timestamp, Value,
};
use os_store::{CrawlFile, CrawlInit, StorageProfile};
fn setup() -> (tempfile::TempDir, CrawlFile, CrawlConfig) {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.rendering.mode = RenderingMode::JavaScript;
    let at = Timestamp::from_db("2026-10-21T00:00:00.000Z").unwrap();
    let file = CrawlFile::create(
        &dir.path().join("js.ospider"),
        &CrawlInit {
            crawl_id: SeededIds::new(2).new_crawl_id(at),
            mode: CrawlMode::Spider,
            start_url: Some("https://site.test/".into()),
            list_source: None,
            project: "JavaScript".into(),
            name: "Fixture".into(),
            config: cfg.clone(),
            created_at: at,
        },
        StorageProfile::Balanced,
    )
    .unwrap();
    file.conn()
        .execute_batch(include_str!(
            "../../../tests/fixtures/tech-m2-javascript/facts.sql"
        ))
        .unwrap();
    (dir, file, cfg)
}
fn page(file: &CrawlFile, cfg: &CrawlConfig, filter: &str) -> ViewPage {
    views::query_view(file, cfg, &ViewQuery::new("JavaScript", filter)).unwrap()
}
fn names(file: &CrawlFile, cfg: &CrawlConfig, filter: &str) -> Vec<String> {
    let mut names: Vec<_> = page(file, cfg, filter)
        .rows
        .iter()
        .map(|r| match &r[0] {
            Value::Text(s) => s.rsplit('/').next().unwrap().to_string(),
            _ => panic!(),
        })
        .collect();
    names.sort();
    names
}
fn cell<'a>(page: &'a ViewPage, name: &str, column: &str) -> &'a Value {
    let i = page.columns.iter().position(|c| c == column).unwrap();
    &page
        .rows
        .iter()
        .find(|r| r[0] == Value::Text(format!("https://site.test/{name}")))
        .unwrap()[i]
}
// spec: TECH-101
#[test]
fn population_columns_signed_word_change_percentage_and_unique_js_counts() {
    let (_dir, file, cfg) = setup();
    assert_eq!(
        names(&file, &cfg, "All"),
        ["http", "mutate", "prefix", "removed", "spa", "static", "zero"]
    );
    let p = page(&file, &cfg, "All");
    assert_eq!(p.columns.len(), 25);
    for (name, column, value) in [
        ("spa", "Word Count Change", Value::Int(120)),
        ("spa", "JS Word Count %", Value::Float(100.0)),
        ("zero", "JS Word Count %", Value::Float(0.0)),
        ("removed", "Word Count Change", Value::Int(-30)),
        ("spa", "Unique JS Outlinks", Value::Int(1)),
        ("spa", "Unique External JS Outlinks", Value::Int(1)),
        ("spa", "Unique JS Inlinks", Value::Int(1)),
    ] {
        assert_eq!(cell(&p, name, column), &value, "{name}:{column}");
    }
    for filter in os_analyze::tabs::javascript::tab().filters {
        assert_eq!(
            views::state_of(&file, &cfg, "JavaScript", filter.id).unwrap(),
            FilterState::Ready
        );
        page(&file, &cfg, filter.id);
    }
}
// spec: TECH-102
#[test]
fn only_robots_denials_trigger_blocked_resources() {
    let (_dir, file, cfg) = setup();
    assert_eq!(names(&file, &cfg, "Pages With Blocked Resources"), ["spa"]);
}
// spec: TECH-103
#[test]
fn js_links_require_a_new_hyperlink_target() {
    let (_dir, file, cfg) = setup();
    assert_eq!(
        names(&file, &cfg, "Contains JavaScript Links"),
        ["mutate", "spa"]
    );
}
// spec: TECH-104
#[test]
fn content_requires_a_strict_word_increase() {
    let (_dir, file, cfg) = setup();
    assert_eq!(names(&file, &cfg, "Contains JavaScript Content"), ["spa"]);
}
// spec: TECH-105
// spec: TECH-106
#[test]
fn original_none_counts_but_token_substrings_do_not() {
    let (_dir, file, cfg) = setup();
    for f in [
        "Noindex Only In Original HTML",
        "Nofollow Only In Original HTML",
    ] {
        assert_eq!(names(&file, &cfg, f), ["removed"]);
    }
}
// spec: TECH-107
// spec: TECH-108
#[test]
fn canonical_added_and_mismatch_are_disjoint() {
    let (_dir, file, cfg) = setup();
    assert_eq!(
        names(&file, &cfg, "Canonical Only In Rendered HTML"),
        ["spa"]
    );
    assert_eq!(names(&file, &cfg, "Canonical Mismatch"), ["mutate"]);
}
// spec: TECH-109
// spec: TECH-110
// spec: TECH-111
#[test]
fn added_and_updated_text_filters_are_disjoint() {
    let (_dir, file, cfg) = setup();
    for label in ["Page Title", "Meta Description", "H1"] {
        assert_eq!(
            names(&file, &cfg, &format!("{label} Only In Rendered HTML")),
            ["spa"]
        );
        assert_eq!(
            names(&file, &cfg, &format!("{label} Updated By JavaScript")),
            ["mutate"]
        );
    }
}
// spec: TECH-114
#[test]
fn warnings_do_not_trigger_errors_and_disabled_collection_is_explicit() {
    let (_dir, file, mut cfg) = setup();
    assert_eq!(names(&file, &cfg, "Pages With JavaScript Errors"), ["spa"]);
    cfg.rendering.js_error_reporting = false;
    assert!(matches!(
        views::state_of(&file, &cfg, "JavaScript", "Pages With JavaScript Errors").unwrap(),
        FilterState::NeedsConfig(_)
    ));
    assert!(names(&file, &cfg, "Pages With JavaScript Errors").is_empty());
    cfg.rendering.js_error_reporting = true;
    cfg.rendering.mode = RenderingMode::Text;
    for f in os_analyze::tabs::javascript::tab().filters {
        assert_eq!(
            views::state_of(&file, &cfg, "JavaScript", f.id).unwrap(),
            FilterState::NeedsRendering
        );
    }
}

mod tab_javascript_support;
// spec: TECH-101
// spec: TECH-102
// spec: TECH-103
// spec: TECH-104
// spec: TECH-105
// spec: TECH-106
// spec: TECH-107
// spec: TECH-108
// spec: TECH-109
// spec: TECH-110
// spec: TECH-111
// spec: TECH-114
// spec: TECH-015 [M2]
// spec: ONPAGE-053
// spec: ONPAGE-057
#[tokio::test]
async fn chromium_comparisons_preserve_directives_and_distinguish_resource_denials() {
    let fx = tab_javascript_support::rendered_fixture().await;
    let cfg = fx.file.config().unwrap();
    let names = |tab: &str, filter: &str| {
        let p = views::query_view(&fx.file, &cfg, &ViewQuery::new(tab, filter)).unwrap();
        let mut names: Vec<_> = p
            .rows
            .iter()
            .map(|r| match &r[0] {
                Value::Text(s) => s.rsplit('/').next().unwrap().to_string(),
                _ => panic!(),
            })
            .collect();
        names.sort();
        names
    };
    assert!(names("JavaScript", "Pages With Blocked Resources").contains(&"robots".into()));
    assert!(!names("JavaScript", "Pages With Blocked Resources").contains(&"excluded".into()));
    for filter in [
        "Page Title Only In Rendered HTML",
        "Meta Description Only In Rendered HTML",
        "H1 Only In Rendered HTML",
        "Contains JavaScript Content",
        "Contains JavaScript Links",
    ] {
        assert!(
            names("JavaScript", filter).contains(&"spa".into()),
            "{filter}"
        );
    }
    for filter in [
        "Page Title Updated By JavaScript",
        "Meta Description Updated By JavaScript",
        "H1 Updated By JavaScript",
    ] {
        assert_eq!(names("JavaScript", filter), ["mutate"], "{filter}");
    }
    assert_eq!(
        names("JavaScript", "Noindex Only In Original HTML"),
        ["noindex", "none"]
    );
    assert_eq!(
        names("JavaScript", "Nofollow Only In Original HTML"),
        ["nofollow", "none"]
    );
    let restricted:i64=fx.file.conn().query_row("SELECT COUNT(*) FROM links l JOIN urls u ON u.id=l.source_id WHERE (u.address LIKE '%/nofollow' OR u.address LIKE '%/none') AND l.link_type='Hyperlink' AND l.follow=0",[],|r|r.get(0)).unwrap();
    assert_eq!(restricted, 2);
    let noindex:i64=fx.file.conn().query_row("SELECT COUNT(*) FROM urls WHERE (address LIKE '%/noindex' OR address LIKE '%/none') AND indexability_status='noindex'",[],|r|r.get(0)).unwrap();
    assert_eq!(noindex, 2);
    let canonicals = names("JavaScript", "Canonical Only In Rendered HTML");
    for name in ["spa", "canonical", "httpcanonical"] {
        assert!(canonicals.contains(&name.into()), "{canonicals:?}");
    }
    assert_eq!(names("JavaScript", "Canonical Mismatch"), ["mutate"]);
    assert!(!names("Canonicals", "Missing").contains(&"canonical".into()));
    assert!(names("JavaScript", "Pages With JavaScript Errors").contains(&"error".into()));
    assert!(!names("JavaScript", "Pages With JavaScript Errors").contains(&"warn".into()));
    assert_eq!(
        names("Response Codes", "Redirection (JavaScript)"),
        ["redirect"]
    );
    assert_eq!(
        names("Response Codes", "External Redirection (JavaScript)"),
        Vec::<String>::new()
    );
    let resources = names("Response Codes", "Blocked Resource");
    assert!(resources.contains(&"app.js".into()), "{resources:?}");
    assert!(resources.contains(&"e500.css".into()), "{resources:?}");
    assert!(!resources.contains(&"excluded.js".into()));
    assert!(!names("Response Codes", "Blocked by Robots.txt").contains(&"app.js".into()));
    let excluded:i64=fx.file.conn().query_row("SELECT COUNT(*) FROM render_blocked WHERE url LIKE '%/excluded.js' AND reason='excluded'",[],|r|r.get(0)).unwrap();
    assert!(excluded > 0);
    assert_eq!(
        fx.file
            .conn()
            .query_row("SELECT COUNT(*) FROM page_html", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
}

// spec: TECH-102
// spec: ONPAGE-053
#[tokio::test]
async fn robots_ignored_and_disabled_scripts_do_not_become_robots_rendering_issues() {
    use os_engine::testing::crawl_fixture_from;
    for (mode, scripts) in [
        (os_model::RobotsMode::Ignore, true),
        (os_model::RobotsMode::IgnoreButReport, true),
        (os_model::RobotsMode::Respect, false),
    ] {
        let mut cfg = CrawlConfig::default();
        cfg.rendering.mode = RenderingMode::JavaScript;
        cfg.rendering.ajax_timeout_s = 0.5;
        cfg.rendering.screenshots.enabled = false;
        cfg.limits.max_depth = Some(0);
        cfg.robots.mode = mode;
        cfg.spider.javascript.crawl = scripts;
        let fx = crawl_fixture_from("tech-m2-javascript", "/robots", cfg.clone()).await;
        assert!(
            names(&fx.file, &cfg, "Pages With Blocked Resources").is_empty(),
            "{mode:?}, scripts={scripts}"
        );
        if !scripts {
            let reasons: Vec<_> = fx
                .file
                .conn()
                .prepare("SELECT reason FROM render_blocked WHERE url LIKE '%/blocked/app.js'")
                .unwrap()
                .query_map([], |r| r.get::<_, String>(0))
                .unwrap()
                .map(Result::unwrap)
                .collect();
            assert_eq!(reasons, ["excluded"]);
        }
    }
}
