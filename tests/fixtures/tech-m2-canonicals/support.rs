// Gemeinsame lokale Store-Fixture für die D04-Tests, keine Netzabhängigkeit.
pub struct Fixture {
    pub dir: tempfile::TempDir,
    pub file: os_store::CrawlFile,
    pub cfg: os_model::CrawlConfig,
}
pub fn fixture() -> Fixture {
    use os_model::time::{IdSource, SeededIds};
    let dir = tempfile::tempdir().unwrap();
    let at = os_model::Timestamp::from_db("2026-10-18T10:00:00.000Z").unwrap();
    let cfg = os_model::CrawlConfig::default();
    let file = os_store::CrawlFile::create(&dir.path().join("canonical.ospider"), &os_store::CrawlInit {
        crawl_id: SeededIds::new(4).new_crawl_id(at), mode: os_model::CrawlMode::Spider,
        start_url: Some("https://site.test/home".into()), list_source: None,
        project: "D04".into(), name: "Canonicals".into(), config: cfg.clone(), created_at: at,
    }, os_store::StorageProfile::Balanced).unwrap();
    file.conn().execute_batch(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/tech-m2-canonicals/graph.sql"))).unwrap();
    Fixture {dir,file,cfg}
}
pub fn analyse(fx: &Fixture) {
    os_analyze::analysis::run(&fx.file, &fx.cfg, &os_model::CancelToken::new(), &|_,_| {}).unwrap();
}
