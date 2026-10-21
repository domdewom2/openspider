use std::{future::Future, pin::Pin};
type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
use os_engine::render_glue::{RenderError, RenderOpts, RenderedPage};
use os_engine::{
    testing::{crawl_fixture_with, test_deps},
    Renderer,
};
use os_model::{CrawlConfig, RenderingMode, Value};
use std::sync::Arc;
struct FixtureRenderer;
impl Renderer for FixtureRenderer {
    fn render<'a>(
        &'a self,
        url: &'a str,
        _: &'a RenderOpts,
    ) -> BoxFuture<'a, Result<RenderedPage, RenderError>> {
        Box::pin(async move {
            Ok(RenderedPage {
                dom_html: include_str!("dom.html").into(),
                navigation_url: Some(url.into()),
                ..Default::default()
            })
        })
    }
}
// spec: ONPAGE-016
// spec: CRAWL-079
#[tokio::test]
async fn js_counts_are_live_distinct_and_empty_in_text_mode() {
    let mut cfg = CrawlConfig::default();
    cfg.rendering.mode = RenderingMode::JavaScript;
    cfg.limits.max_depth = Some(0);
    let mut deps = test_deps();
    deps.renderer = Some(Arc::new(FixtureRenderer));
    let fx = crawl_fixture_with("crawl-m2-render", "/", cfg.clone(), deps).await;
    fx.file
        .conn()
        .execute("DROP INDEX IF EXISTS links_target", [])
        .unwrap();
    let mut query = os_analyze::views::ViewQuery::new("Internal", "All");
    query.columns = os_analyze::views::ColumnSelection::Named(vec![
        "Address".into(),
        "Unique JS Inlinks".into(),
        "Unique JS Outlinks".into(),
        "Unique External JS Outlinks".into(),
    ]);
    let page = os_analyze::views::query_view(&fx.file, &cfg, &query).unwrap();
    let root = page
        .rows
        .iter()
        .find(|row| row[0] == Value::Text(fx.site.url("/")))
        .unwrap();
    assert_eq!(&root[1..], &[Value::Int(0), Value::Int(1), Value::Int(1)]);
    let columns = os_analyze::tabs::internal::rendering_column_source()
        .unwrap()
        .columns(&cfg);
    let os_analyze::defs::ColumnSql::Expr(inlinks) = columns[0].sql else {
        panic!("expression")
    };
    let count: i64 = fx
        .file
        .conn()
        .query_row(
            &format!("SELECT {inlinks} FROM urls u WHERE u.url LIKE '%/js-only'"),
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
    cfg.rendering.mode = RenderingMode::Text;
    let page = os_analyze::views::query_view(&fx.file, &cfg, &query).unwrap();
    assert!(page
        .rows
        .iter()
        .all(|row| row[1..].iter().all(|value| *value == Value::Null)));
    for column in os_analyze::tabs::internal::rendering_column_source()
        .unwrap()
        .columns(&{
            cfg.rendering.mode = RenderingMode::JavaScript;
            cfg
        })
    {
        let os_analyze::defs::ColumnSql::Expr(expr) = column.sql else {
            panic!("expression")
        };
        let mut stmt = fx
            .file
            .conn()
            .prepare(&format!("EXPLAIN QUERY PLAN SELECT {expr} FROM urls u"))
            .unwrap();
        let plan = stmt
            .query_map([], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(
            plan.iter()
                .any(|line| line.contains("AUTOMATIC") && line.contains("id=?")),
            "{plan:?}"
        );
    }
}
