use os_analyze::{
    analysis::{self, CancelToken},
    details::DetailReq,
    tabs::images::{self, ImageFilter},
    views::{self, ViewQuery},
};
use os_model::{
    time::{IdSource, SeededIds},
    AnalysisStep, CrawlConfig, CrawlMode, FilterState, RenderingMode, StepState, Timestamp, UrlId,
    Value,
};
use os_store::{CrawlFile, CrawlInit, StorageProfile};
fn setup() -> (tempfile::TempDir, CrawlFile, CrawlConfig) {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.rendering.mode = RenderingMode::JavaScript;
    let at = Timestamp::from_db("2026-10-18T00:00:00.000Z").unwrap();
    let file = CrawlFile::create(
        &dir.path().join("images.ospider"),
        &CrawlInit {
            crawl_id: SeededIds::new(3).new_crawl_id(at),
            mode: CrawlMode::Spider,
            start_url: Some("https://site.test/".into()),
            list_source: None,
            project: "Images".into(),
            name: "Fixture".into(),
            config: cfg.clone(),
            created_at: at,
        },
        StorageProfile::Balanced,
    )
    .unwrap();
    file.conn()
        .execute_batch(include_str!(
            "../../../tests/fixtures/onpage-m2-images/analysis/facts.sql"
        ))
        .unwrap();
    (dir, file, cfg)
}
fn analyze(file: &CrawlFile, cfg: &CrawlConfig) {
    assert_eq!(
        analysis::steps::images::run(file, cfg, &CancelToken::new()).unwrap(),
        StepState::Done
    );
    analysis::persist::write_step(
        file,
        AnalysisStep::Images,
        StepState::Done,
        None,
        Some(&os_analyze::state::step_config_hash(
            AnalysisStep::Images,
            cfg,
        )),
    )
    .unwrap();
}
fn names(file: &CrawlFile, cfg: &CrawlConfig, filter: &str) -> Vec<String> {
    views::query_view(file, cfg, &ViewQuery::new("Images", filter))
        .unwrap()
        .rows
        .iter()
        .map(|r| match &r[0] {
            Value::Text(s) => s.rsplit('/').next().unwrap().to_string(),
            _ => panic!(),
        })
        .collect()
}
fn data(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    ids: &[UrlId],
    filter: ImageFilter,
    offset: u64,
    limit: u64,
) -> os_analyze::details::DetailRows {
    images::instance_rows(
        file,
        cfg,
        &DetailReq {
            url_ids: ids,
            params: &serde_json::json!({}),
            sort: &[],
            offset,
            limit,
        },
        filter,
    )
    .unwrap()
}
// spec: ONPAGE-178
#[test]
fn backgrounds_exclude_img_and_srcset_on_any_page_and_reanalysis_is_atomic() {
    let (_dir, file, mut cfg) = setup();
    cfg.rendering.mode = RenderingMode::Text;
    assert_eq!(
        views::state_of(&file, &cfg, "Images", "Background Images").unwrap(),
        FilterState::NeedsRendering
    );
    cfg.images.parse_inline_css_backgrounds = true;
    assert_eq!(
        views::state_of(&file, &cfg, "Images", "Background Images").unwrap(),
        FilterState::NeedsCrawlAnalysis
    );
    analyze(&file, &cfg);
    assert_eq!(
        names(&file, &cfg, "Background Images"),
        ["bg.png", "dynamic.png"]
    );
    let seq = os_store::queries::change_seq(file.conn()).unwrap();
    analyze(&file, &cfg);
    assert_eq!(os_store::queries::change_seq(file.conn()).unwrap(), seq);
    file.conn().execute("INSERT INTO image_instances(page_id,image_id,src_raw,source,alt_present,position) VALUES (2,13,'bg.png','srcset',1,99)",[]).unwrap();
    let cancel = CancelToken::new();
    cancel.cancel();
    assert_eq!(
        analysis::steps::images::run(&file, &cfg, &cancel).unwrap(),
        StepState::Pending
    );
    assert_eq!(
        names(&file, &cfg, "Background Images"),
        ["bg.png", "dynamic.png"]
    );
    analyze(&file, &cfg);
    assert_eq!(names(&file, &cfg, "Background Images"), ["dynamic.png"]);
    assert!(os_store::queries::change_seq(file.conn()).unwrap() > seq);
}
// spec: ONPAGE-179
#[test]
fn missing_size_is_per_img_instance_including_empty_attributes() {
    let (_dir, file, cfg) = setup();
    analyze(&file, &cfg);
    assert_eq!(
        names(&file, &cfg, "Missing Size Attributes"),
        ["hidden.png", "missing.png", "mixed.jpg"]
    );
    let instances = data(&file, &cfg, &[], ImageFilter::MissingSize, 0, 500);
    assert_eq!(instances.total, 3);
    assert!(instances
        .rows
        .iter()
        .all(|r| r[13] != Value::Text("css".into()) && r[13] != Value::Text("a".into())));
}
// spec: ONPAGE-180
#[test]
fn savings_threshold_is_inclusive_and_excludes_hidden_svg_offscreen_and_upscaled() {
    let (_dir, file, cfg) = setup();
    analyze(&file, &cfg);
    assert_eq!(
        names(&file, &cfg, "Incorrectly Sized Images"),
        [
            "above.png",
            "edge.png",
            "long.png",
            "missing.png",
            "mixed.jpg"
        ]
    );
    let rows = data(
        &file,
        &cfg,
        &[UrlId(10)],
        ImageFilter::IncorrectSize,
        0,
        500,
    );
    assert_eq!(rows.total, 2);
    assert!(rows.rows.iter().all(|r| r[7] == Value::Int(196608)));
    let mut text = cfg.clone();
    text.rendering.mode = RenderingMode::Text;
    assert_eq!(
        views::state_of(&file, &text, "Images", "Incorrectly Sized Images").unwrap(),
        FilterState::NeedsRendering
    );
    assert!(names(&file, &text, "Incorrectly Sized Images").is_empty());
    let mut threshold = cfg.clone();
    threshold.images.incorrect_size_min_saving_kb = 5;
    assert_eq!(
        views::state_of(&file, &threshold, "Images", "Incorrectly Sized Images").unwrap(),
        FilterState::Stale
    );
}
// spec: ONPAGE-180
#[test]
fn zero_threshold_still_requires_an_oversized_dimension_and_nonnegative_savings() {
    let (_dir, file, mut cfg) = setup();
    cfg.images.incorrect_size_min_saving_kb = 0;
    // Gleich große Fläche, aber eine zu große Dimension: bei Schwelle 0 eingeschlossen.
    file.conn().execute("UPDATE image_instances SET real_w=300,real_h=400,display_w=400,display_h=300 WHERE image_id=15",[]).unwrap();
    analyze(&file, &cfg);
    let found = names(&file, &cfg, "Incorrectly Sized Images");
    assert!(found.contains(&"small.png".into()));
    assert!(!found.contains(&"up.png".into()));
    // Größere Anzeigefläche trotz einer zu großen Quelldimension: negative Schätzung.
    file.conn()
        .execute(
            "UPDATE image_instances SET real_h=350 WHERE image_id=15",
            [],
        )
        .unwrap();
    analyze(&file, &cfg);
    assert!(!names(&file, &cfg, "Incorrectly Sized Images").contains(&"small.png".into()));
}

// spec: ONPAGE-172
#[test]
fn details_page_and_image_selection_preserve_parity_columns_and_empty_facts() {
    let (_dir, file, mut cfg) = setup();
    let params = serde_json::json!({});
    let ids = [UrlId(11)];
    let mut req = DetailReq {
        url_ids: &ids,
        params: &params,
        sort: &[],
        offset: 0,
        limit: 500,
    };
    cfg.rendering.mode = RenderingMode::Text;
    let raw = images::detail_rows(&file, &cfg, &req).unwrap();
    assert_eq!(
        raw.columns
            .iter()
            .map(|(n, _)| n.as_str())
            .collect::<Vec<_>>(),
        [
            "From",
            "To",
            "Alt Text",
            "Real Dimensions (WxH)",
            "Dimensions in Attributes (WxH)",
            "Display Dimensions (WxH)",
            "Potential Savings",
            "Path Type",
            "Link Path",
            "Link Position",
            "Link Origin"
        ]
    );
    assert_eq!(raw.total, 2);
    assert_eq!(raw.rows[0][3], Value::Null);
    assert_eq!(raw.rows[0][4], Value::Null);
    assert_eq!(raw.rows[0][5], Value::Null);
    cfg.images.real_dimensions_from_header = true;
    assert_eq!(
        images::detail_rows(&file, &cfg, &req).unwrap().rows[0][3],
        Value::Text("800x600".into())
    );
    cfg.rendering.mode = RenderingMode::JavaScript;
    let rendered = images::detail_rows(&file, &cfg, &req).unwrap();
    assert_eq!(rendered.rows[0][3], Value::Text("800x600".into()));
    assert_eq!(rendered.rows[0][5], Value::Text("400x300".into()));
    let svg = data(&file, &cfg, &[UrlId(18)], ImageFilter::All, 0, 500);
    assert_eq!(svg.rows[0][4], Value::Null);
    let offscreen = data(&file, &cfg, &[UrlId(23)], ImageFilter::All, 0, 500);
    assert_eq!(offscreen.rows[0][4], Value::Null);
    assert_eq!(offscreen.rows[0][6], Value::Null);
    let extensions = serde_json::json!({"extensions":true});
    let extended = images::detail_rows(
        &file,
        &cfg,
        &DetailReq {
            params: &extensions,
            ..req
        },
    )
    .unwrap();
    assert_eq!(extended.columns.len(), 13);
    assert_eq!(extended.columns[11].0, "Alt Text Length");
    assert_eq!(extended.columns[12].0, "Source");
    req.offset = 50;
    let empty = images::detail_rows(&file, &cfg, &req).unwrap();
    assert_eq!(empty.total, 2);
    assert!(empty.rows.is_empty());
    req.url_ids = &[];
    assert_eq!(images::detail_rows(&file, &cfg, &req).unwrap().total, 0);
    let all = data(
        &file,
        &cfg,
        &[UrlId(1), UrlId(10)],
        ImageFilter::All,
        0,
        500,
    );
    let page1 = data(&file, &cfg, &[UrlId(1), UrlId(10)], ImageFilter::All, 0, 3);
    let page2 = data(
        &file,
        &cfg,
        &[UrlId(1), UrlId(10)],
        ImageFilter::All,
        3,
        500,
    );
    assert_eq!([page1.rows, page2.rows].concat(), all.rows);
    assert_eq!(all.total, 17);
}
