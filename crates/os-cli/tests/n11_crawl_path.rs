mod common;

use common::{read_csv, resolve, site, Env};

// spec: OUT-023 [M2]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn loaded_crawl_exports_selection_in_argument_order_like_surface_export() {
    let site = site("out-basic-site").await;
    let env = Env::new();
    let run = env
        .run_async(vec![
            "--crawl".into(),
            "http://site.test/".into(),
            "--resolve".into(),
            resolve(&site),
            "--set".into(),
            "threads=1".into(),
        ])
        .await;
    assert_eq!(run.code, 0, "{}", run.dump());
    let path = env.crawl_files().remove(0);
    let file = os_store::CrawlFile::open(&path, os_store::OpenMode::ReadWrite).unwrap();
    let cfg = file.config().unwrap();
    let selected: Vec<(i64, String)> = file
        .conn()
        .prepare(
            "SELECT id,url FROM urls WHERE is_internal=1 AND state='done' ORDER BY id DESC LIMIT 2",
        )
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(selected.len(), 2);
    let ids: Vec<_> = selected
        .iter()
        .map(|(id, _)| os_model::UrlId(*id))
        .collect();
    let params = serde_json::json!({});
    let req = os_analyze::details::DetailReq {
        url_ids: &ids,
        params: &params,
        sort: &[],
        offset: 0,
        limit: u64::MAX,
    };
    let job = os_export::catalog::TableJob {
        entry_name: "Crawl Path Report",
        tabs: &[],
        file: &file,
        cfg: &cfg,
    };
    let expected = env.work.path().join("surface.csv");
    os_export::detail_export::export_detail(
        os_analyze::details::DetailKind::CrawlPath,
        &job,
        &req,
        &expected,
        os_export::Format::Csv,
    )
    .unwrap();
    drop(file);
    let requests = site.total_requests();
    let run = env
        .run_async(vec![
            "--load-crawl".into(),
            path.to_string_lossy().into_owned(),
            "--export-crawl-path".into(),
            selected
                .iter()
                .map(|(_, u)| u.as_str())
                .collect::<Vec<_>>()
                .join(","),
            "--output-folder".into(),
            "out".into(),
        ])
        .await;
    assert_eq!(run.code, 0, "{}", run.dump());
    assert_eq!(site.total_requests(), requests);
    assert_eq!(
        std::fs::read(expected).unwrap(),
        std::fs::read(env.out().join("crawl_path_report.csv")).unwrap()
    );
    let run = env
        .run_async(vec![
            "--load-crawl".into(),
            path.to_string_lossy().into_owned(),
            "--export-crawl-path".into(),
            "http://site.test/missing-n11".into(),
            "--output-folder".into(),
            "missing".into(),
        ])
        .await;
    assert_eq!(run.code, 0, "{}", run.dump());
    assert!(run.stderr.contains("missing-n11"), "{}", run.dump());
    assert_eq!(
        read_csv(&env.work.path().join("missing/crawl_path_report.csv")).len(),
        1
    );
    let run = env
        .run_async(vec![
            "--load-crawl".into(),
            path.to_string_lossy().into_owned(),
            "--export-crawl-path".into(),
            selected[0].1.clone(),
            "--export-format".into(),
            "xlsx".into(),
            "--output-folder".into(),
            "xlsx".into(),
        ])
        .await;
    assert_eq!(run.code, 0, "{}", run.dump());
    assert!(env
        .work
        .path()
        .join("xlsx/crawl_path_report.xlsx")
        .is_file());
}

// spec: OUT-203 [P1]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_path_selection_is_rejected_before_crawl() {
    let site = site("out-basic-site").await;
    for selection in ["", "https://site.test/,", "https://[bad/"] {
        let env = Env::new();
        let run = env
            .run_async(vec![
                "--crawl".into(),
                "http://site.test/".into(),
                "--resolve".into(),
                resolve(&site),
                "--export-crawl-path".into(),
                selection.into(),
            ])
            .await;
        assert_eq!(run.code, 2, "{}", run.dump());
    }
    assert_eq!(site.total_requests(), 0);
}
