use os_model::{
    CrawlConfig, CrawlMode, CustomJsWrite, IdSource, JsSnippet, JsSnippetKind, RenderingMode,
    SeededIds, Timestamp, UrlId, WriteBatch,
};
use os_store::{CrawlFile, CrawlInit, CrawlSink, SqliteSink, StorageProfile};

pub fn create(path: &std::path::Path) -> CrawlFile {
    let mut cfg = CrawlConfig::default();
    cfg.rendering.mode = RenderingMode::JavaScript;
    cfg.custom_js.snippets = vec![
        JsSnippet {
            kind: JsSnippetKind::Action,
            timeout_s: 1,
            name: "action".into(),
            ..Default::default()
        },
        JsSnippet::default(),
        JsSnippet {
            name: "empty".into(),
            ..Default::default()
        },
    ];
    let file = CrawlFile::create(
        path,
        &CrawlInit {
            config: cfg,
            crawl_id: SeededIds::new(22).new_crawl_id(Timestamp::UNIX_EPOCH),
            mode: CrawlMode::Spider,
            start_url: Some("https://extract.test/a".into()),
            list_source: None,
            project: "D22".into(),
            name: "JavaScript".into(),
            created_at: Timestamp::UNIX_EPOCH,
        },
        StorageProfile::Balanced,
    )
    .unwrap();
    file.conn()
        .execute_batch(include_str!("store.sql"))
        .unwrap();
    let mut writer = SqliteSink::open(path, StorageProfile::Balanced).unwrap();
    writer
        .write(WriteBatch {
            custom_js: vec![
                CustomJsWrite {
                    url_id: UrlId(1),
                    def_index: 1,
                    number: 0,
                    value: Some("Alpha".into()),
                    error: None,
                },
                CustomJsWrite {
                    url_id: UrlId(1),
                    def_index: 1,
                    number: 1,
                    value: Some("Beta".into()),
                    error: None,
                },
                CustomJsWrite {
                    url_id: UrlId(1),
                    def_index: 1,
                    number: 2,
                    value: Some("Gamma".into()),
                    error: None,
                },
                CustomJsWrite {
                    url_id: UrlId(2),
                    def_index: 1,
                    number: 0,
                    value: None,
                    error: Some("Timeout".into()),
                },
                CustomJsWrite {
                    url_id: UrlId(3),
                    def_index: 2,
                    number: 0,
                    value: None,
                    error: None,
                },
            ],
            ..Default::default()
        })
        .unwrap();
    writer.finish().unwrap();
    file
}
