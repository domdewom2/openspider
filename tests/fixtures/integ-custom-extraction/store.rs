use os_model::{
    CrawlConfig, CrawlMode, CustomExtractionWrite, Extractor, IdSource, SeededIds, Timestamp,
    UrlId, WriteBatch,
};
use os_store::{CrawlFile, CrawlInit, CrawlSink, SqliteSink, StorageProfile};

pub fn create(path: &std::path::Path) -> CrawlFile {
    let mut cfg = CrawlConfig::default();
    cfg.custom_extraction.extractors = vec![
        Extractor {
            name: "h2".into(),
            expression: "//h2".into(),
            ..Default::default()
        },
        Extractor {
            expression: "//comment()".into(),
            ..Default::default()
        },
    ];
    let file = CrawlFile::create(
        path,
        &CrawlInit {
            config: cfg,
            crawl_id: SeededIds::new(15).new_crawl_id(Timestamp::UNIX_EPOCH),
            mode: CrawlMode::Spider,
            start_url: Some("https://extract.test/a".into()),
            list_source: None,
            project: "D15".into(),
            name: "Extraction".into(),
            created_at: Timestamp::UNIX_EPOCH,
        },
        StorageProfile::Balanced,
    )
    .unwrap();
    file.conn()
        .execute_batch(include_str!("store.sql"))
        .unwrap();
    let values: Vec<(i64, u32, u32, String)> =
        serde_json::from_str(include_str!("values.json")).unwrap();
    let mut writer = SqliteSink::open(path, StorageProfile::Balanced).unwrap();
    writer
        .write(WriteBatch {
            custom_extraction: values
                .into_iter()
                .map(|(id, def_index, number, value)| CustomExtractionWrite {
                    url_id: UrlId(id),
                    def_index,
                    number,
                    value,
                    error: None,
                })
                .collect(),
            ..Default::default()
        })
        .unwrap();
    writer.finish().unwrap();
    file
}
