//! D03 supplies the image/srcset link transport; D11's resource facts are tested without ignores.
use os_engine::testing::crawl_fixture_from;
use os_model::CrawlConfig;

// spec: ONPAGE-033, ONPAGE-037
#[tokio::test]
#[ignore = "Klärung K2-68: N01-C03, Spec-Unklarheit ONPAGE-033/037 gegen A-42 (Einzel-Links aus srcset nur mit extraction.srcset; Linkzeile ohne Abruf bei ausgeschalteter Option ist eine Engine-Entscheidung, M2-E05); Parser geprüft, mit Option an grün"]
async fn srcset_resources_are_available_as_individual_outlinks() {
    let mut cfg = CrawlConfig::default();
    cfg.spider.external.crawl = false;
    let fx = crawl_fixture_from("onpage-m2-security", "https://site.test/", cfg).await;
    let mut stmt = fx.file.conn().prepare("SELECT l.target_raw FROM links l JOIN urls s ON s.id=l.source_id WHERE s.url='https://site.test/https/resources.html' AND l.link_type='Image' ORDER BY l.target_raw").unwrap();
    let got: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    for want in [
        "http://cdn.test/one.png",
        "//cdn.test/two.png",
        "http://cdn.test/three.png",
    ] {
        assert!(got.iter().any(|raw| raw == want), "missing {want}: {got:?}");
    }
}
