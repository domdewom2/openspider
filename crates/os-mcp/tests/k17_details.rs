#[path = "tools_support.rs"]
mod support;
use os_mcp::{tools::url_details::UrlDetails, Tool, ToolContext};
use os_model::CrawlConfig;
use serde_json::json;
use std::sync::Arc;
use support::*;

const RAW: &str = include_str!("../../../tests/fixtures/crawl-m2-render/raw.html");
const DOM: &str = include_str!("../../../tests/fixtures/crawl-m2-render/dom.html");

struct FixtureRenderer(&'static str);
impl os_engine::Renderer for FixtureRenderer {
    fn render<'a>(
        &'a self,
        _: &'a str,
        _: &'a os_render::RenderOpts,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<os_render::RenderedPage, os_render::RenderError>,
                > + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            Ok(os_render::RenderedPage {
                dom_html: self.0.into(),
                timings: os_model::RenderTimings {
                    load_ms: 12,
                    wait_ms: 34,
                    total_ms: 56,
                    ..Default::default()
                },
                ..Default::default()
            })
        })
    }
}

// spec: AGENT-012 [M2]
#[tokio::test]
async fn d20_structured_data_fixture_reaches_untrusted_details() {
    let world = World::new();
    let mut cfg = CrawlConfig::default();
    cfg.extraction.structured_data.json_ld = true;
    cfg.extraction.structured_data.microdata = true;
    cfg.extraction.structured_data.rdfa = true;
    let stored = world.add("tech-m2-sd", "/mixed.html", cfg, 70).await;
    let reply = world
        .server()
        .call(
            "url_details",
            Some(
                json!({
                    "crawl_id":stored.id, "url_id":1, "sections":["structured_data"]
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await;
    let reply = Reply::from_result(&reply);
    assert_eq!(reply.summary()["structured_data"]["status"], "collected");
    assert_eq!(reply.summary()["structured_data"]["total"], 7);
    let rows = reply.untrusted.as_ref().unwrap()["structured_data"]
        .as_array()
        .unwrap();
    assert_eq!(rows.len(), 7);
    for (format, kind, name) in [
        ("json-ld", "Product", "X"),
        ("microdata", "Product", "X"),
        ("rdfa", "Organization", "O"),
    ] {
        let row = rows
            .iter()
            .find(|r| r[2] == format && r[3] == kind)
            .unwrap();
        let item: serde_json::Value = serde_json::from_str(row[4].as_str().unwrap()).unwrap();
        assert_eq!(item["name"], name);
    }
    assert!(!reply.trusted_text().contains("schema.org"));
}

// spec: AGENT-012 [M2]
#[tokio::test]
async fn e05_rendering_fixture_reports_metadata_and_html_changes() {
    for (dom, changed) in [(DOM, true), (RAW, false)] {
        let world = World::new();
        let mut cfg = CrawlConfig::default();
        cfg.rendering.mode = os_model::RenderingMode::JavaScript;
        cfg.rendering.screenshots.enabled = false;
        cfg.extraction.store_html = true;
        cfg.extraction.store_rendered_html = true;
        cfg.limits.max_depth = Some(0);
        let mut deps = os_engine::testing::test_deps();
        deps.renderer = Some(Arc::new(FixtureRenderer(dom)));
        let fx =
            os_engine::testing::crawl_fixture_with("crawl-m2-render", "/raw.html", cfg, deps).await;
        let id = fx.file.meta().unwrap().crawl_id.to_string();
        fx.file
            .save_as(&world.dir.path().join(format!("{id}.ospider")))
            .unwrap();
        let ctx = ToolContext::new(world.dir.path().into());
        let out = UrlDetails
            .call(
                &ctx,
                json!({
                    "crawl_id":id, "url_id":1, "sections":["rendering"]
                }),
            )
            .unwrap();
        assert_eq!(out.summary["rendering"]["status"], "ok");
        assert_eq!(out.summary["rendering"]["load_ms"], 12);
        assert_eq!(out.summary["rendering"]["wait_ms"], 34);
        assert_eq!(out.summary["rendering"]["total_ms"], 56);
        assert!(out.summary["rendering"]["word_count"].as_u64().unwrap() > 0);
        let data = out.untrusted.unwrap();
        assert_eq!(data["rendering"]["original_html"], RAW);
        assert_eq!(data["rendering"]["rendered_html"], dom);
        assert_eq!(data["rendering"]["html_changed"], changed);
        assert!(!out.summary.to_string().contains("<html"));
    }
}
