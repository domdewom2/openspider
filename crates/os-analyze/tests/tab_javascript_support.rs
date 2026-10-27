use os_engine::testing::{crawl_fixture_from, FixtureCrawl};
use os_model::{CrawlConfig, RenderingMode};

pub async fn rendered_fixture() -> FixtureCrawl {
    let mut cfg = CrawlConfig::default();
    cfg.rendering.mode = RenderingMode::JavaScript;
    cfg.rendering.ajax_timeout_s = 1.5;
    cfg.rendering.screenshots.enabled = false;
    cfg.rendering.tabs = Some(2);
    cfg.rendering.viewport.preset = "custom".into();
    cfg.speed.threads = 2;
    cfg.scope.exclude = vec!["excluded\\.js$".into()];
    // Comparison facts must survive without either source document being retained.
    cfg.extraction.store_html = false;
    cfg.extraction.store_rendered_html = false;
    crawl_fixture_from("tech-m2-javascript", "/", cfg).await
}
