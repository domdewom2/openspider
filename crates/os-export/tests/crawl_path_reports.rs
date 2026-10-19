#[path = "sidebar_support.rs"]
mod support;
use os_analyze::details::{DetailKind, DetailReq};
use os_export::{
    catalog::{self, ExportKind, TableJob},
    detail_export, Format,
};
use os_model::{CrawlConfig, UrlId, Value};

// spec: OUT-023
#[test]
fn selection_exports_first_discovery_redirect_sitemap_and_list_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let file = support::file(dir.path(), "paths.sql");
    let cfg = CrawlConfig::default();
    let job = TableJob {
        entry_name: "Crawl Path Report",
        tabs: &[],
        file: &file,
        cfg: &cfg,
    };
    let ids = [
        UrlId(4),
        UrlId(6),
        UrlId(9),
        UrlId(11),
        UrlId(12),
        UrlId(14),
    ];
    let params = serde_json::json!({});
    let req = DetailReq {
        url_ids: &ids,
        params: &params,
        sort: &[],
        offset: 10,
        limit: 1,
    };
    let path = dir.path().join("paths.csv");
    let info = detail_export::export_detail(DetailKind::CrawlPath, &job, &req, &path, Format::Csv)
        .unwrap();
    assert_eq!(info.rows, 14);
    assert!(info
        .warnings
        .iter()
        .any(|n| n.contains("nicht über Links erreicht")));
    support::golden(&path, "crawl_path.csv");
    assert!(!catalog::list(ExportKind::Report)
        .iter()
        .any(|n| n.contains("Crawl Path")));
    let page = os_store::queries::crawl_path::detail_rows(&file, &ids, 2, 3).unwrap();
    assert_eq!(page.total, 14);
    assert_eq!(page.rows.len(), 3);
    assert_eq!(page.rows[0][2], Value::Text("https://site.test/a".into()));
    let path2 = dir.path().join("again.csv");
    detail_export::export_detail(DetailKind::CrawlPath, &job, &req, &path2, Format::Csv).unwrap();
    assert_eq!(std::fs::read(path).unwrap(), std::fs::read(path2).unwrap());
}

// spec: OUT-023
#[test]
fn legacy_bfs_has_warning_and_cycles_do_not_loop() {
    let dir = tempfile::tempdir().unwrap();
    let file = support::file(dir.path(), "paths.sql");
    file.conn()
        .execute_batch("DELETE FROM discovery_edges")
        .unwrap();
    let page = os_store::queries::crawl_path::paths(&file, &[UrlId(9)], 0, 100).unwrap();
    assert_eq!(page.data.rows.len(), 3);
    assert_eq!(
        page.data.rows[1][2],
        Value::Text("https://site.test/l".into())
    );
    assert!(page.notes[0].contains("Herkunftsdaten fehlen"));
    file.conn().execute_batch("INSERT INTO discovery_edges(seq,target_id,source_id,kind,link_type,depth) VALUES(1,9,7,'link','Hyperlink',2),(2,7,9,'link','Hyperlink',1)").unwrap();
    let page = os_store::queries::crawl_path::paths(&file, &[UrlId(9)], 0, 100).unwrap();
    assert!(page.data.rows.is_empty());
    assert!(page.notes.iter().any(|n| n.contains("Zyklus")));
}

// spec: OUT-023
#[test]
fn xlsx_path_preserves_numeric_step_status_and_depth() {
    use calamine::{open_workbook, Data, Reader, Xlsx};
    let dir = tempfile::tempdir().unwrap();
    let file = support::file(dir.path(), "paths.sql");
    let cfg = CrawlConfig::default();
    let job = TableJob {
        entry_name: "Crawl Path Report",
        tabs: &[],
        file: &file,
        cfg: &cfg,
    };
    let params = serde_json::json!({});
    let req = DetailReq {
        url_ids: &[UrlId(6)],
        params: &params,
        sort: &[],
        offset: 0,
        limit: 1,
    };
    let path = dir.path().join("path.xlsx");
    detail_export::export_detail(DetailKind::CrawlPath, &job, &req, &path, Format::Xlsx).unwrap();
    let mut book: Xlsx<_> = open_workbook(path).unwrap();
    let range = book.worksheet_range_at(0).unwrap().unwrap();
    assert_eq!(range.get_value((1, 1)), Some(&Data::Float(0.)));
    assert_eq!(range.get_value((2, 3)), Some(&Data::Float(301.)));
    assert_eq!(range.get_value((1, 6)), Some(&Data::Float(2.)));
}
