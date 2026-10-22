//! Full command integration cases for F05 data sources owned by other W1 packages (K2-26).
#[path = "cmd_catalog_fixture.rs"]
mod fixture;
use fixture::*;
use serde_json::json;
// spec: TECH-066 [M2]
#[tokio::test]
async fn directives_minimum_snippet_reaches_url_details() {
    let fx = Fx::new();
    let rows = fx.call("detail.urlDetails", json!({"urlId":1})).await;
    assert!(rows.as_array().unwrap().iter().any(|r| r["name"]
        .as_str()
        .unwrap()
        .to_lowercase()
        .contains("max-snippet")
        && r["value"] == "10"));
}
// spec: TECH-089
#[tokio::test]
async fn hreflang_confirmation_reaches_detail_view() {
    let fx = Fx::new();
    fx.write("INSERT INTO hreflang(url_id,source,position,code_raw,code_norm,code_valid,href_raw,url,target_id,in_head) VALUES(1,'html',0,'en','en',1,'https://site.test/','https://site.test/',1,1)");
    let h = fx
        .open(
            json!({"kind":"detail","detail":"hreflang","urlIds":[1],"params":{},"sort":[]}),
            json!([]),
        )
        .await;
    let columns = h["columns"].as_array().unwrap();
    let index = columns
        .iter()
        .position(|c| c["key"] == "Confirmation")
        .unwrap();
    let rows = fx.rows(&h, 0, 500, json!([])).await;
    assert!(rows["cells"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r[index] == "Self"));
}
// spec: ONPAGE-172
#[tokio::test]
async fn image_instances_are_available_through_view_images() {
    let fx = Fx::new();
    let h = fx
        .open(
            json!({"kind":"images","urlIds":[1],"search":null,"sort":[]}),
            json!([]),
        )
        .await;
    assert_eq!(h["total"], 1);
    let rows = fx.rows(&h, 0, 500, json!([])).await;
    assert!(rows["cells"][0]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == "https://site.test/image.png"));
}
// spec: TECH-146
#[tokio::test]
async fn anchors_mode_aggregates_hyperlinks_with_self_links_excluded() {
    let fx = Fx::new();
    let mut spec = links(true);
    spec["filter"] = json!({"id":"Hyperlinks","mode":"anchors","excludeSelf":true});
    let h = fx.open(spec, json!([])).await;
    let rows = fx.rows(&h, 0, 500, json!([])).await;
    let columns = rows["columns"].as_array().unwrap();
    let anchor = columns.iter().position(|c| c == "Anchor Text").unwrap();
    let count = columns.iter().position(|c| c == "Links").unwrap();
    assert!(rows["cells"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r[anchor] == "Alpha" && r[count] == 2));
    assert!(!rows["cells"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r[anchor] == "Self"));
}
// spec: OUT-023
#[tokio::test]
async fn crawl_path_export_uses_the_registered_detail_source() {
    let fx = Fx::new();
    let path = fx.dir.path().join("path.csv");
    fx.op("export.detail",json!({"spec":{"kind":"detail","detail":"crawlPath","urlIds":[2],"params":{},"sort":[]},"selection":{"kind":"ids","ids":[2]},"path":path,"format":"csv"})).await.unwrap();
    let text = std::fs::read_to_string(path).unwrap();
    assert!(text.contains("Target Address,Step,Address,Status Code,Link Type,Anchor,Crawl Depth"));
    assert_eq!(text.lines().count(), 3);
}

// spec: OUT-124
#[tokio::test]
async fn image_sitemap_options_reach_the_configuration_aware_provider() {
    let fx = Fx::new();
    let path = fx.dir.path().join("image-sitemap");
    fx.op(
        "export.sitemap",
        json!({"sitemapKind":"images","options":null,"dir":path}),
    )
    .await
    .unwrap();
    // OUT-127: the image sitemap is named image-sitemap.xml (Standardname), not sitemap.xml.
    let xml = std::fs::read_to_string(path.join("image-sitemap.xml")).unwrap();
    assert!(xml.contains("image:loc"));
    assert!(xml.contains("https://site.test/image.png"));
}

// spec: TECH-146
#[tokio::test]
async fn parameterless_anchor_details_keep_the_f05_provider_reachable() {
    let fx = Fx::new();
    let spec = json!({"kind":"detail","detail":"anchors","urlIds":[2],"params":{},"sort":[]});
    let h = fx.open(spec.clone(), json!([])).await;
    assert!(h["total"].as_u64().unwrap() > 0);
    let rows = fx.rows(&h, 0, 500, json!([])).await;
    assert!(rows["columns"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c == "Anchor Text"));
    let path = fx.dir.path().join("anchors.csv");
    fx.op(
        "export.detail",
        json!({"spec":spec,"selection":{"kind":"ids","ids":[2]},"path":path,"format":"csv"}),
    )
    .await
    .unwrap();
    assert!(std::fs::read_to_string(path)
        .unwrap()
        .contains("Anchor Text"));
}
