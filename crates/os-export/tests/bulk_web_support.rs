#![allow(dead_code)]
use os_export::{export_bulk, Format};
use os_model::time::{IdSource, SeededIds};
use os_model::{CrawlConfig, CrawlMode, Timestamp};
use os_store::{CrawlFile, CrawlInit, StorageProfile};
use std::path::{Path, PathBuf};

pub fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/ui/fx-ui-headers")
}
pub fn config() -> CrawlConfig {
    let mut cfg = CrawlConfig::default();
    cfg.extraction.http_headers = true;
    cfg.extraction.cookies = true;
    cfg.extraction.store_html = true;
    cfg.export.utf8_bom = false;
    cfg.export.line_ending = os_model::LineEnding::Lf;
    cfg
}
pub fn file(directory: &Path) -> CrawlFile {
    let at = Timestamp::from_db("2030-01-01T00:00:00.000Z").unwrap();
    let file = CrawlFile::create(
        &directory.join("fixture.ospider"),
        &CrawlInit {
            crawl_id: SeededIds::new(18).new_crawl_id(at),
            mode: CrawlMode::Spider,
            start_url: Some("https://www.site.test/".into()),
            list_source: None,
            project: "D18".into(),
            name: "Web".into(),
            config: config(),
            created_at: at,
        },
        StorageProfile::Balanced,
    )
    .unwrap();
    file.conn()
        .execute_batch(&std::fs::read_to_string(fixture_dir().join("data.sql")).unwrap())
        .unwrap();
    file
}
pub fn bulk(file: &CrawlFile, cfg: &CrawlConfig, name: &str, out: &Path) -> String {
    let files = export_bulk(file, cfg, name, out, Format::Csv).unwrap();
    std::fs::read_to_string(&files.last().unwrap().path).unwrap()
}
pub fn records(text: &str) -> Vec<Vec<String>> {
    csv::Reader::from_reader(text.trim_start_matches('\u{feff}').as_bytes())
        .records()
        .map(|r| r.unwrap().iter().map(str::to_owned).collect())
        .collect()
}
pub fn golden(name: &str, text: &str) {
    os_testsite::golden::assert_golden(fixture_dir().join("golden").join(name), text);
}

pub fn cookie_dates(file: &CrawlFile) -> Vec<(String, Option<String>)> {
    let cases: Vec<(String, Option<String>)> = serde_json::from_str(
        &std::fs::read_to_string(fixture_dir().join("cookie-dates.json")).unwrap(),
    )
    .unwrap();
    for (i, (raw, _)) in cases.iter().enumerate() {
        file.conn().execute("INSERT INTO cookies(url_id,position,origin,name,value,expires,secure,http_only,raw) VALUES(1,?1,'http',?2,'value',?3,0,0,?4)",
            (100 + i as i64, format!("date-{i}"), raw, format!("date-{i}=value; Expires={raw}"))).unwrap();
    }
    cases
}
