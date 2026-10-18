#![allow(dead_code)]
use os_model::{
    time::{IdSource, SeededIds},
    CrawlConfig, CrawlMode, Timestamp,
};
use os_store::{CrawlFile, CrawlInit, StorageProfile};
use std::path::{Path, PathBuf};
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
pub fn file(dir: &Path, fixture: &str) -> CrawlFile {
    let at = Timestamp::from_db("2026-10-18T08:30:00.000Z").unwrap();
    let file = CrawlFile::create(
        &dir.join("crawl.ospider"),
        &CrawlInit {
            crawl_id: SeededIds::new(17).new_crawl_id(at),
            mode: CrawlMode::Spider,
            start_url: Some("https://site.test/".into()),
            list_source: None,
            project: "Fixture".into(),
            name: "D17".into(),
            config: CrawlConfig::default(),
            created_at: at,
        },
        StorageProfile::Balanced,
    )
    .unwrap();
    file.set_meta("started_at", &at.to_db()).unwrap();
    if !fixture.is_empty() {
        file.conn()
            .execute_batch(
                &std::fs::read_to_string(
                    root().join("tests/fixtures/out-m2-reports").join(fixture),
                )
                .unwrap(),
            )
            .unwrap();
    }
    file
}
pub fn golden(path: &Path, name: &str) {
    let text = std::fs::read_to_string(path).unwrap();
    let normalized = text.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    let golden = root().join("tests/golden/out-m2-reports").join(name);
    if std::env::var_os("UPDATE_D17_GOLDENS").is_some() {
        std::fs::write(&golden, &normalized).unwrap();
    }
    assert_eq!(normalized, std::fs::read_to_string(golden).unwrap());
}
pub fn csv(path: &Path) -> Vec<Vec<String>> {
    let text = std::fs::read_to_string(path).unwrap();
    csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(text.trim_start_matches('\u{feff}').as_bytes())
        .records()
        .map(|r| r.unwrap().iter().map(str::to_owned).collect())
        .collect()
}
