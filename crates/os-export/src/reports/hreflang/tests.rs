//! Fixture: out-hreflang, hosted under the package's tech-m2-hreflang directory (K2-24).
use super::*;
use os_analyze::{details::DetailReq, tabs};
use os_model::{
    time::{IdSource, SeededIds},
    AnalysisStep, CancelToken, CrawlConfig, CrawlMode, StepState, Timestamp, UrlId,
};
use os_store::{CrawlFile, CrawlInit, StorageProfile};

fn fixture() -> (tempfile::TempDir, CrawlFile, CrawlConfig) {
    let dir = tempfile::tempdir().unwrap();
    let cfg = CrawlConfig::default();
    let at = Timestamp::from_db("2026-10-18T12:00:00.000Z").unwrap();
    let init = CrawlInit {
        crawl_id: SeededIds::new(7).new_crawl_id(at),
        mode: CrawlMode::Spider,
        start_url: Some("https://site.test/1".into()),
        list_source: None,
        project: "test".into(),
        name: "hreflang".into(),
        config: cfg.clone(),
        created_at: at,
    };
    let file = CrawlFile::create(
        &dir.path().join("test.ospider"),
        &init,
        StorageProfile::Balanced,
    )
    .unwrap();
    file.conn()
        .execute_batch(include_str!(
            "../../../../../tests/fixtures/tech-m2-hreflang/relations.sql"
        ))
        .unwrap();
    (dir, file, cfg)
}
fn csv_report(index: usize) -> (Vec<String>, Vec<Vec<String>>) {
    let (dir, file, cfg) = fixture();
    // Only OUT-092 needs the post-crawl flags; OUT-090 and the other reports use live facts.
    if index == 2 {
        os_analyze::analysis::steps::hreflang::run(&file, &cfg, &CancelToken::new()).unwrap();
        os_analyze::analysis::persist::write_step(
            &file,
            AnalysisStep::Hreflang,
            StepState::Done,
            None,
            None,
        )
        .unwrap();
    }
    let tabs = tabs::all();
    let entry = &entries()[index];
    let path = dir.path().join("report.csv");
    let job = TableJob {
        entry_name: entry.name,
        tabs: &tabs,
        file: &file,
        cfg: &cfg,
    };
    crate::catalog::export_table(entry, &job, &path, Format::Csv).unwrap();
    let text = std::fs::read_to_string(path).unwrap();
    let mut reader = csv::Reader::from_reader(text.trim_start_matches('\u{feff}').as_bytes());
    let header = reader
        .headers()
        .unwrap()
        .iter()
        .map(str::to_string)
        .collect();
    let rows = reader
        .records()
        .map(|r| r.unwrap().iter().map(str::to_string).collect())
        .collect();
    (header, rows)
}
// spec: OUT-090
#[test]
fn all_reports_every_annotation_and_source() {
    let (header, rows) = csv_report(0);
    assert_eq!(header, entries()[0].header);
    assert_eq!(rows.len(), 20);
    assert!(rows.iter().any(|r| r[3] == "XML Sitemap"));
    assert!(rows.iter().any(|r| r[3] == "HTTP Header"));
    assert_eq!(
        rows[0],
        [
            "https://site.test/1",
            "en-GB",
            "https://site.test/1",
            "HTML",
            "200",
            "Indexable",
            "True"
        ]
    );
}
// spec: OUT-091
// spec: TECH-088
#[test]
fn non200_only_contains_the_failed_relationship() {
    let (header, rows) = csv_report(1);
    assert_eq!(header, entries()[1].header);
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0],
        [
            "https://site.test/1",
            "it",
            "https://site.test/4",
            "HTML",
            "404",
            "Non-Indexable",
            "False",
            "Not Found"
        ]
    );
}
// spec: OUT-092
#[test]
fn unlinked_emits_each_unlinked_relationship_after_analysis() {
    let (_, rows) = csv_report(2);
    assert_eq!(
        rows.iter().map(|r| r[2].as_str()).collect::<Vec<_>>(),
        [
            "https://site.test/3",
            "https://site.test/4",
            "https://site.test/5",
            "https://site.test/8",
            "https://site.test/16"
        ]
    );
}
// spec: OUT-093
#[test]
fn missing_obeys_live_filter_and_accepts_sitemap_returns() {
    let (header, rows) = csv_report(3);
    assert_eq!(header, entries()[3].header);
    let from_one: Vec<_> = rows
        .iter()
        .filter(|r| r[0] == "https://site.test/1")
        .map(|r| r[2].as_str())
        .collect();
    assert_eq!(
        from_one,
        [
            "https://site.test/3",
            "https://site.test/6",
            "https://site.test/16"
        ]
    );
    assert!(!rows.iter().any(|r| r[2] == "https://site.test/4")); // K2-46: filter wins over old OUT-093
}
// spec: OUT-094
#[test]
fn inconsistent_uses_filter_direction_and_resolves_alias() {
    let (header, rows) = csv_report(4);
    assert_eq!(header, entries()[4].header);
    assert_eq!(
        rows,
        [vec![
            "https://site.test/1",
            "https://site.test/7",
            "en-GB",
            "en"
        ]]
    );
    let (dir, file, cfg) = fixture();
    let result = crate::pipeline::export_report(
        &file,
        &cfg,
        " hreflang : inconsistent LANGUAGE return links ",
        dir.path(),
        Format::Csv,
    )
    .unwrap();
    assert_eq!(result.files[0].rows, 1);
    let actual = std::fs::read_to_string(&result.files[0].path).unwrap();
    assert!(actual.contains("https://site.test/1,https://site.test/7,en-GB,en"));
    assert_eq!(
        crate::catalog::resolve(
            crate::catalog::ExportKind::Report,
            "Hreflang:Inconsistent Language Return Links"
        )
        .unwrap(),
        crate::catalog::ExportRef::Report(entries()[4].name.into())
    );
}
// spec: OUT-095
#[test]
fn noncanonical_reports_the_alternates_canonical() {
    let (header, rows) = csv_report(5);
    assert_eq!(header, entries()[5].header);
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|r| r[3] == "https://site.test/2"));
}
// spec: OUT-096
#[test]
fn noindex_reports_effective_indexability_status() {
    let (header, rows) = csv_report(6);
    assert_eq!(header, entries()[6].header);
    assert_eq!(
        rows,
        [vec![
            "https://site.test/1",
            "es",
            "https://site.test/6",
            "noindex"
        ]]
    );
}
// spec: TECH-089
#[test]
fn detail_csv_and_xlsx_export_selection_and_truncation_warning() {
    use calamine::{open_workbook_auto, Reader};
    let (dir, file, cfg) = fixture();
    let tabs = tabs::all();
    let job = TableJob {
        entry_name: "Hreflang Details",
        tabs: &tabs,
        file: &file,
        cfg: &cfg,
    };
    let req = DetailReq {
        url_ids: &[UrlId(1), UrlId(2)],
        params: &serde_json::Value::Null,
        sort: &[],
        offset: 4,
        limit: 1,
    };
    for format in [Format::Csv, Format::Xlsx] {
        let path = dir.path().join(format!("detail.{}", format.extension()));
        let info = export_detail(&job, &req, &path, format).unwrap();
        assert_eq!(info.rows, 12);
        assert_eq!(info.warnings, ["Hreflang Truncated: 5"]);
        if format == Format::Xlsx {
            let mut book = open_workbook_auto(path).unwrap();
            let range = book.worksheet_range_at(0).unwrap().unwrap();
            assert_eq!(range.height(), 13);
            assert_eq!(range.get_value((0, 0)).unwrap().to_string(), "Address");
            assert_eq!(range.get_value((1, 6)).unwrap().to_string(), "Self");
        } else {
            let text = std::fs::read_to_string(path).unwrap();
            assert!(text.contains("Confirmation"));
            assert!(text.contains("Not Applicable"));
        }
    }
}
// spec: OUT-092
#[test]
fn unlinked_report_before_analysis_is_header_only_and_empty_selection_has_header() {
    let (dir, file, cfg) = fixture();
    let tabs = tabs::all();
    let entry = &entries()[2];
    let job = TableJob {
        entry_name: entry.name,
        tabs: &tabs,
        file: &file,
        cfg: &cfg,
    };
    let path = dir.path().join("pending.csv");
    assert_eq!(
        crate::catalog::export_table(entry, &job, &path, Format::Csv)
            .unwrap()
            .rows,
        0
    );
    let req = DetailReq {
        url_ids: &[],
        params: &serde_json::Value::Null,
        sort: &[],
        offset: 0,
        limit: 1,
    };
    assert_eq!(
        export_detail(&job, &req, &dir.path().join("empty.csv"), Format::Csv)
            .unwrap()
            .rows,
        0
    );
}
// spec: TECH-089
#[test]
fn detail_export_pages_past_the_viewport_limit() {
    let (dir, file, cfg) = fixture();
    // Duplicate to more than one SQL page without changing the first 500-annotation semantics of a URL.
    file.conn().execute_batch("INSERT INTO hreflang (url_id,source,position,code_raw,code_norm,code_valid,href_raw,url,target_id,in_head) SELECT 14,'sitemap',value,'en','en',1,'/2','https://site.test/2',2,1 FROM json_each('[' || (WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<500) SELECT group_concat(x) FROM n) || ']');").unwrap();
    let tabs = tabs::all();
    let job = TableJob {
        entry_name: "Hreflang Details",
        tabs: &tabs,
        file: &file,
        cfg: &cfg,
    };
    let req = DetailReq {
        url_ids: &[UrlId(1), UrlId(14)],
        params: &serde_json::Value::Null,
        sort: &[],
        offset: 0,
        limit: 1,
    };
    let info = export_detail(&job, &req, &dir.path().join("pages.csv"), Format::Csv).unwrap();
    assert_eq!(info.rows, 510);
    let entry = &entries()[0];
    let job = TableJob {
        entry_name: entry.name,
        ..job
    };
    let info = crate::catalog::export_table(
        entry,
        &job,
        &dir.path().join("report-pages.csv"),
        Format::Csv,
    )
    .unwrap();
    assert_eq!(info.rows, 520);
}

// spec: TECH-089
#[test]
fn detail_csv_matches_fixture_golden() {
    let (dir, file, cfg) = fixture();
    let tabs = tabs::all();
    let job = TableJob {
        entry_name: "Hreflang Details",
        tabs: &tabs,
        file: &file,
        cfg: &cfg,
    };
    let req = DetailReq {
        url_ids: &[UrlId(1)],
        params: &serde_json::Value::Null,
        sort: &[],
        offset: 0,
        limit: 500,
    };
    let path = dir.path().join("detail.csv");
    export_detail(&job, &req, &path, Format::Csv).unwrap();
    let actual = std::fs::read_to_string(path).unwrap();
    assert_eq!(
        actual.trim_start_matches('\u{feff}').replace("\r\n", "\n"),
        include_str!("../../../../../tests/fixtures/tech-m2-hreflang/detail.csv")
    );
}
