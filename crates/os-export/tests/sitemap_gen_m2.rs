//! M2-X02 gegen Store-Fixtures nach K2-24: out-sitemap-gen, out-sitemap-images, out-hreflang.
#[path = "sitemap_xsd.rs"]
mod xsd;

use os_export::{
    generate_sitemap,
    sitemap_gen::{
        generate_image_sitemap_with, generate_sitemap_configured_with, hreflang_available,
    },
    SitemapOptions, SitemapReport,
};
use os_model::{
    ChangeFreq, ChangeFreqBasis, Clock, CrawlConfig, CrawlMode, FixedClock, IdSource,
    LastmodSource, SeededIds, Timestamp,
};
use os_store::{CrawlFile, CrawlInit, StorageProfile};
use os_testsite::golden::assert_golden;
use std::{path::Path, time::Duration};

const SELECTION: &str =
    include_str!("../../../tests/fixtures/out-m2-sitemap-gen/out-sitemap-gen/selection.sql");
const METADATA: &str =
    include_str!("../../../tests/fixtures/out-m2-sitemap-gen/out-sitemap-gen/metadata.sql");
const IMAGES: &str =
    include_str!("../../../tests/fixtures/out-m2-sitemap-gen/out-sitemap-images/images.sql");
const HREFLANG: &str =
    include_str!("../../../tests/fixtures/out-m2-sitemap-gen/out-hreflang/hreflang.sql");

fn clock() -> FixedClock {
    FixedClock::new(
        Timestamp::from_db("2026-10-13T12:00:00.000Z").unwrap(),
        Duration::ZERO,
    )
}

fn fixture(dir: &Path, sql: &str) -> CrawlFile {
    let file = CrawlFile::create(
        &dir.join("crawl.ospider"),
        &CrawlInit {
            crawl_id: SeededIds::new(2).new_crawl_id(clock().now()),
            mode: CrawlMode::Spider,
            start_url: Some("https://site.test/".into()),
            list_source: None,
            project: "".into(),
            name: "Sitemap".into(),
            config: CrawlConfig::default(),
            created_at: clock().now(),
        },
        StorageProfile::Balanced,
    )
    .unwrap();
    file.conn().execute_batch(sql).unwrap();
    file
}

fn generate(file: &CrawlFile, cfg: &CrawlConfig, out: &Path) -> SitemapReport {
    generate_sitemap_configured_with(file, &SitemapOptions::default(), cfg, None, &clock(), out)
        .unwrap()
}

fn xml(out: &Path, name: &str) -> String {
    std::fs::read_to_string(out.join(name)).unwrap()
}

fn paths(report: &SitemapReport) -> Vec<String> {
    report
        .files
        .iter()
        .filter(|f| {
            !f.path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .contains("index")
        })
        .flat_map(|f| {
            let doc = xsd::validate_file(&xsd::sitemap_schema(), &f.path).unwrap();
            doc.children.into_iter().map(|u| {
                u.children[0]
                    .text
                    .trim_start_matches("https://site.test")
                    .to_string()
            })
        })
        .collect()
}

fn assert_paths(file: &CrawlFile, cfg: &CrawlConfig, expected: &[&str]) {
    let out = tempfile::tempdir().unwrap();
    let report = generate(file, cfg, out.path());
    assert_eq!(paths(&report), expected);
}

macro_rules! inclusion_test {
    ($name:ident, $field:ident, $path:literal) => {
        // spec: OUT-121
        #[test]
        fn $name() {
            let dir = tempfile::tempdir().unwrap();
            let file = fixture(dir.path(), SELECTION);
            let mut cfg = CrawlConfig::default();
            cfg.sitemap_gen.$field = true;
            assert_paths(
                &file,
                &cfg,
                &["/", $path, "/impressum", "/search?sessionid=secret"],
            );
        }
    };
}
inclusion_test!(include_noindex_independently, include_noindex, "/noindex");
inclusion_test!(
    include_canonicalised_independently,
    include_canonicalised,
    "/canonical"
);
inclusion_test!(
    include_paginated_independently,
    include_paginated,
    "/paginated"
);
inclusion_test!(include_pdf_independently, include_pdfs, "/doc.pdf");
inclusion_test!(include_redirects_independently, include_3xx, "/redirect");
inclusion_test!(include_client_errors_independently, include_4xx, "/missing");
inclusion_test!(include_server_errors_independently, include_5xx, "/error");
inclusion_test!(
    include_no_response_independently,
    include_no_response,
    "/no-response"
);

// spec: OUT-121
#[test]
fn all_inclusions_still_exclude_resources_robots_removed_and_unfetched() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture(dir.path(), SELECTION);
    let mut cfg = CrawlConfig::default();
    let s = &mut cfg.sitemap_gen;
    s.include_noindex = true;
    s.include_canonicalised = true;
    s.include_paginated = true;
    s.include_pdfs = true;
    s.include_3xx = true;
    s.include_4xx = true;
    s.include_5xx = true;
    s.include_no_response = true;
    assert_paths(
        &file,
        &cfg,
        &[
            "/",
            "/noindex",
            "/canonical",
            "/paginated",
            "/doc.pdf",
            "/redirect",
            "/missing",
            "/error",
            "/no-response",
            "/both",
            "/pdf-noindex",
            "/impressum",
            "/search?sessionid=secret",
            "/redirect-noindex",
            "/redirect-canonical",
        ],
    );
}

// spec: OUT-121
#[test]
fn combined_exclusion_reasons_require_each_option() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture(dir.path(), SELECTION);
    for noindex in [false, true] {
        for canonical in [false, true] {
            let mut cfg = CrawlConfig::default();
            cfg.sitemap_gen.include_noindex = noindex;
            cfg.sitemap_gen.include_canonicalised = canonical;
            let out = tempfile::tempdir().unwrap();
            let p = paths(&generate(&file, &cfg, out.path()));
            assert_eq!(p.contains(&"/both".into()), noindex && canonical);
        }
    }
}

// spec: OUT-122
#[test]
fn removed_urls_and_regex_matches_are_excluded() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture(dir.path(), SELECTION);
    file.conn()
        .execute(
            "UPDATE urls SET in_tables=0 WHERE url='https://site.test/impressum'",
            [],
        )
        .unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.sitemap_gen.exclude_patterns = vec![r"\?sessionid=".into()];
    assert_paths(&file, &cfg, &["/"]);
    // fancy-regex-Vertrag: Lookaround wird unterstützt, nicht nur einfache Regex.
    cfg.sitemap_gen.exclude_patterns = vec![r"search(?=\?sessionid=)".into()];
    assert_paths(&file, &cfg, &["/"]);
    cfg.sitemap_gen.exclude_patterns = vec!["(".into()];
    let out = tempfile::tempdir().unwrap();
    assert!(generate_sitemap_configured_with(
        &file,
        &SitemapOptions::default(),
        &cfg,
        None,
        &clock(),
        out.path()
    )
    .is_err());
    assert_eq!(std::fs::read_dir(out.path()).unwrap().count(), 0);
}

// spec: OUT-123
// spec: OUT-124
// spec: OUT-125
#[test]
fn default_metadata_matches_golden_and_uses_url_crawl_time() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture(dir.path(), METADATA);
    let out = tempfile::tempdir().unwrap();
    let report = generate(&file, &CrawlConfig::default(), out.path());
    assert_eq!(report.urls, 8);
    let text = xml(out.path(), "sitemap.xml");
    assert_golden(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/golden/out-m2-sitemap-gen/metadata.xml"
        ),
        &text,
    );
    let doc = xsd::validate_file(&xsd::sitemap_schema(), &report.files[0].path).unwrap();
    let values = |tag: &str| {
        doc.children
            .iter()
            .map(|u| {
                u.children
                    .iter()
                    .find(|e| e.name == tag)
                    .map(|e| e.text.as_str())
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(values("lastmod")[0], Some("2026-10-01"));
    assert_eq!(values("lastmod")[4..6], [Some("2026-10-13"); 2]);
    assert_eq!(
        values("priority"),
        ["1.0", "0.9", "0.8", "0.7", "0.6", "0.5", "0.5", "0.5"].map(Some)
    );
    assert_eq!(
        values("changefreq"),
        ["monthly", "daily", "monthly", "monthly", "daily", "daily", "monthly", "daily"].map(Some)
    );
    assert_eq!(report.files[0].bytes, text.len() as u64);
}

// spec: OUT-123
#[test]
fn custom_date_is_shared_and_can_be_disabled() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture(dir.path(), METADATA);
    let mut cfg = CrawlConfig::default();
    cfg.sitemap_gen.lastmod.source = LastmodSource::Custom;
    cfg.sitemap_gen.lastmod.custom_date = Some("2026-10-13".into());
    let out = tempfile::tempdir().unwrap();
    generate(&file, &cfg, out.path());
    assert_eq!(
        xml(out.path(), "sitemap.xml")
            .matches("<lastmod>2026-10-13</lastmod>")
            .count(),
        8
    );
    cfg.sitemap_gen.lastmod.enabled = false;
    generate(&file, &cfg, out.path());
    assert!(!xml(out.path(), "sitemap.xml").contains("<lastmod>"));
    cfg.sitemap_gen.lastmod.enabled = true;
    for bad in ["2026-02-30", "2026-1-1", "2026-10-13<script>", ""] {
        cfg.sitemap_gen.lastmod.custom_date = Some(bad.into());
        assert!(generate_sitemap_configured_with(
            &file,
            &SitemapOptions::default(),
            &cfg,
            None,
            &clock(),
            out.path()
        )
        .is_err());
    }
}

// spec: OUT-124
#[test]
fn priority_overrides_list_depth_zero_and_validation() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture(dir.path(), METADATA);
    let mut cfg = CrawlConfig::default();
    cfg.sitemap_gen.priority.by_depth = vec![0.3, 0.0];
    let out = tempfile::tempdir().unwrap();
    generate(&file, &cfg, out.path());
    let text = xml(out.path(), "sitemap.xml");
    assert_eq!(text.matches("<priority>0.3</priority>").count(), 1);
    assert_eq!(text.matches("<priority>0.0</priority>").count(), 7);
    cfg.sitemap_gen.priority.by_depth = vec![1.0, 0.9, 0.8, 0.7, 0.6, 0.5];
    file.conn()
        .execute("UPDATE urls SET crawl_depth=0", [])
        .unwrap();
    file.conn()
        .execute("UPDATE crawl_meta SET value='list' WHERE key='mode'", [])
        .unwrap();
    generate(&file, &cfg, out.path());
    assert_eq!(
        xml(out.path(), "sitemap.xml")
            .matches("<priority>1.0</priority>")
            .count(),
        8
    );
    for bad in [1.5, -0.1, 0.15, f64::NAN, f64::INFINITY] {
        cfg.sitemap_gen.priority.by_depth = vec![bad];
        assert!(generate_sitemap_configured_with(
            &file,
            &SitemapOptions::default(),
            &cfg,
            None,
            &clock(),
            out.path()
        )
        .is_err());
    }
    cfg.sitemap_gen.priority.enabled = false;
    generate(&file, &cfg, out.path());
    assert!(!xml(out.path(), "sitemap.xml").contains("<priority>"));
}

// spec: OUT-125
#[test]
fn changefreq_depth_table_overrides_and_off() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture(dir.path(), METADATA);
    let mut cfg = CrawlConfig::default();
    cfg.sitemap_gen.changefreq.basis = ChangeFreqBasis::Depth;
    let out = tempfile::tempdir().unwrap();
    generate(&file, &cfg, out.path());
    let text = xml(out.path(), "sitemap.xml");
    assert_eq!(text.matches("<changefreq>daily</changefreq>").count(), 2);
    assert_eq!(text.matches("<changefreq>weekly</changefreq>").count(), 2);
    assert_eq!(text.matches("<changefreq>monthly</changefreq>").count(), 4);
    cfg.sitemap_gen.changefreq.by_depth = vec![
        ChangeFreq::Always,
        ChangeFreq::Hourly,
        ChangeFreq::Yearly,
        ChangeFreq::Never,
    ];
    generate(&file, &cfg, out.path());
    let text = xml(out.path(), "sitemap.xml");
    for value in ["always", "hourly", "yearly"] {
        assert_eq!(
            text.matches(&format!("<changefreq>{value}</changefreq>"))
                .count(),
            1
        );
    }
    assert_eq!(text.matches("<changefreq>never</changefreq>").count(), 5);
    cfg.sitemap_gen.changefreq.enabled = false;
    generate(&file, &cfg, out.path());
    assert!(!xml(out.path(), "sitemap.xml").contains("<changefreq>"));
}

// spec: OUT-126
#[test]
fn image_threshold_counts_pages_inclusively_and_cdn_requires_regex() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture(dir.path(), IMAGES);
    let mut cfg = CrawlConfig::default();
    cfg.sitemap_gen.images.enabled = true;
    cfg.sitemap_gen.images.max_inlinks = Some(5);
    let out = tempfile::tempdir().unwrap();
    generate(&file, &cfg, out.path());
    let text = xml(out.path(), "sitemap.xml");
    assert!(!text.contains("logo.jpg"));
    assert!(!text.contains("six.jpg"));
    assert!(!text.contains("cdn.test"));
    assert!(!text.contains("missing.jpg"));
    assert_eq!(text.matches("product.jpg").count(), 1);
    assert_eq!(text.matches("five.jpg").count(), 5);
    cfg.sitemap_gen.images.external_regex = vec![r"cdn\.test".into()];
    generate(&file, &cfg, out.path());
    let text = xml(out.path(), "sitemap.xml");
    assert!(text.contains("https://cdn.test/p.jpg?a=1&amp;b=2"));
    assert!(!text.contains("other.test"));
    assert_golden(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/golden/out-m2-sitemap-gen/images.xml"
        ),
        &text,
    );
    cfg.sitemap_gen.images.max_inlinks = None;
    generate(&file, &cfg, out.path());
    let text = xml(out.path(), "sitemap.xml");
    assert_eq!(text.matches("logo.jpg").count(), 20);
    assert_eq!(text.matches("six.jpg").count(), 6);
    cfg.sitemap_gen.images.enabled = false;
    generate(&file, &cfg, out.path());
    assert!(!xml(out.path(), "sitemap.xml").contains("image:"));
}

// spec: OUT-126
#[test]
fn image_cap_is_1000_and_warning_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture(dir.path(), IMAGES);
    file.conn().execute_batch("DELETE FROM image_instances; WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<1001) INSERT INTO urls(id,url,address,origin,is_internal,in_tables,state,discovered_via,folder_depth,url_length,status_code,content_class) SELECT 1000+i,'https://site.test/'||i||'.jpg','x','https://site.test',1,1,'done','link',0,30,200,'Image' FROM n; INSERT INTO image_instances(page_id,image_id,src_raw,source,alt_present,position) SELECT 1,id,url,'img',0,id FROM urls WHERE id>1000;").unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.sitemap_gen.images.enabled = true;
    let out = tempfile::tempdir().unwrap();
    let report = generate(&file, &cfg, out.path());
    let text = xml(out.path(), "sitemap.xml");
    assert_eq!(text.matches("<image:image>").count(), 1000);
    assert_eq!(report.files[0].warnings.len(), 1);
    assert!(report.files[0].warnings[0].contains("1000"));
    assert!(!text.contains("/1001.jpg"));
}

// spec: OUT-127
#[test]
fn image_sitemap_omits_empty_pages_and_splits_with_its_own_index() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture(dir.path(), IMAGES);
    let mut cfg = CrawlConfig::default();
    cfg.sitemap_gen.images.max_inlinks = Some(5);
    let out = tempfile::tempdir().unwrap();
    let report = generate_image_sitemap_with(
        &file,
        &SitemapOptions::default(),
        &cfg,
        None,
        &clock(),
        out.path(),
    )
    .unwrap();
    assert_eq!(paths(&report), ["/p1", "/p2", "/p3", "/p4", "/p5"]);
    assert_eq!(
        report.files[0].path.file_name().unwrap(),
        "image-sitemap.xml"
    );
    let opts = SitemapOptions {
        max_urls_per_file: 2,
        ..SitemapOptions::default()
    };
    let split = tempfile::tempdir().unwrap();
    let report =
        generate_image_sitemap_with(&file, &opts, &cfg, None, &clock(), split.path()).unwrap();
    assert_eq!(
        report.files.iter().map(|f| f.rows).collect::<Vec<_>>(),
        [2, 2, 1, 3]
    );
    assert_eq!(paths(&report), ["/p1", "/p2", "/p3", "/p4", "/p5"]);
    let index = xml(split.path(), "image-sitemap_index.xml");
    xsd::validate_file(
        &xsd::index_schema(),
        &split.path().join("image-sitemap_index.xml"),
    )
    .unwrap();
    assert!(index.contains("https://site.test/image-sitemap_1.xml"));
    let split = tempfile::tempdir().unwrap();
    let opts = SitemapOptions {
        max_bytes_per_file: 700,
        ..SitemapOptions::default()
    };
    let report =
        generate_image_sitemap_with(&file, &opts, &cfg, None, &clock(), split.path()).unwrap();
    assert!(report.files.len() > 1);
    for f in &report.files {
        assert_eq!(f.bytes, std::fs::metadata(&f.path).unwrap().len());
        if !f
            .path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .contains("index")
        {
            assert!(f.bytes <= 700);
        }
    }
    cfg.sitemap_gen.images.max_inlinks = Some(0);
    let empty = tempfile::tempdir().unwrap();
    let report = generate_image_sitemap_with(
        &file,
        &SitemapOptions::default(),
        &cfg,
        None,
        &clock(),
        empty.path(),
    )
    .unwrap();
    assert!(report.files.is_empty());
}

// spec: OUT-128
#[test]
fn hreflang_preserves_existing_html_http_annotations_and_filters_targets() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture(dir.path(), HREFLANG);
    let mut cfg = CrawlConfig::default();
    cfg.sitemap_gen.hreflang.enabled = true;
    let out = tempfile::tempdir().unwrap();
    generate(&file, &cfg, out.path());
    let text = xml(out.path(), "sitemap.xml");
    assert_eq!(text.matches("<xhtml:link").count(), 3);
    for code in ["de", "en", "x-default"] {
        assert!(text.contains(&format!("hreflang=\"{code}\"")));
    }
    assert_golden(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/golden/out-m2-sitemap-gen/hreflang.xml"
        ),
        &text,
    );
    cfg.sitemap_gen.hreflang.only_valid_targets = false;
    generate(&file, &cfg, out.path());
    let text = xml(out.path(), "sitemap.xml");
    assert_eq!(text.matches("<xhtml:link").count(), 6);
    assert!(!text.contains("hreflang=\"nl\""));
    cfg.spider.hreflang.store = false;
    assert!(!hreflang_available(&cfg));
    assert!(generate_sitemap_configured_with(
        &file,
        &SitemapOptions::default(),
        &cfg,
        None,
        &clock(),
        out.path()
    )
    .is_err());
    cfg.sitemap_gen.hreflang.enabled = false;
    generate(&file, &cfg, out.path());
    assert!(!xml(out.path(), "sitemap.xml").contains("xhtml:"));
}

// spec: OUT-121
// spec: OUT-123
#[test]
fn existing_entry_point_uses_embedded_v2_configuration() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture(dir.path(), SELECTION);
    let mut cfg = CrawlConfig::default();
    cfg.sitemap_gen.include_pdfs = true;
    cfg.sitemap_gen.lastmod.source = LastmodSource::Custom;
    cfg.sitemap_gen.lastmod.custom_date = Some("2026-10-13".into());
    file.set_config_with_clock(&cfg, &clock()).unwrap();
    let out = tempfile::tempdir().unwrap();
    let report = generate_sitemap(&file, &SitemapOptions::default(), out.path()).unwrap();
    assert!(paths(&report).contains(&"/doc.pdf".into()));
    assert_eq!(
        xml(out.path(), "sitemap.xml")
            .matches("<lastmod>2026-10-13</lastmod>")
            .count(),
        4
    );
}

// spec: OUT-123
#[tokio::test]
async fn server_lastmod_flows_from_http_through_engine_and_store() {
    let fx = os_engine::testing::crawl_fixture(
        "out-m2-sitemap-gen/out-sitemap-gen",
        CrawlConfig::default(),
    )
    .await;
    let out = tempfile::tempdir().unwrap();
    generate_sitemap(&fx.file, &SitemapOptions::default(), out.path()).unwrap();
    let text = xml(out.path(), "sitemap.xml");
    assert_eq!(
        text.matches("<lastmod>").count(),
        text.matches("<url>").count()
    );
    assert!(text.contains("<lastmod>2026-10-01</lastmod>"));
}
