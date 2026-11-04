//! B3-DIFF (Nutzerentscheidung 04.11.2026): Report „Crawl Changes“ in der App (OUT-154). `export.suggestName` und
//! `export.report` laufen über die Vergleichssitzung des Fensters – auch ohne geöffneten Crawl – und schreiben dieselbe
//! Datei wie die CLI (Golden `tests/golden/out-b3-diff/crawl_changes.csv`); ohne Vergleich: `Unsupported` mit Grund.
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]

#[path = "../../os-analyze/tests/compare_common.rs"]
mod common;

use common::*;
use os_app::dto::*;
use os_app::{App, AppConfig, CallCtx};
use os_store::StorageProfile;
use serde_json::json;

fn text(path: &std::path::Path) -> String {
    let raw = std::fs::read_to_string(path).unwrap();
    raw.strip_prefix('\u{feff}')
        .unwrap_or(&raw)
        .replace("\r\n", "\n")
}

// spec: OUT-154
// spec: OUT-112
#[tokio::test(flavor = "multi_thread")]
async fn crawl_changes_report_runs_from_the_comparison_session_of_the_window() {
    let old = crawl_at(
        "out-b3-diff/v1",
        "http://www.site.test/",
        cfg(),
        "2026-01-01T00:00:00.000Z",
        61,
        StorageProfile::Balanced,
    )
    .await;
    let new = crawl_at(
        "out-b3-diff/v2",
        "http://www.site.test/",
        cfg(),
        "2026-02-01T00:00:00.000Z",
        62,
        StorageProfile::Balanced,
    )
    .await;
    let home = tempfile::tempdir().unwrap();
    let app = App::new(AppConfig::new(home.path())).unwrap();
    let ctx = || CallCtx::for_window("main");
    let out = tempfile::tempdir().unwrap();
    let report = json!({"name":"Crawl Changes","dir":out.path(),"format":"csv","singleWorkbook":false});

    // Ohne Vergleich: nicht verfügbar, mit dem Grund der CLI.
    let err = app
        .dispatch(ctx(), "export.report", report.clone())
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Unsupported);
    assert!(format!("{err:?}").contains("requires a crawl comparison"), "{err:?}");

    app.dispatch(
        ctx(),
        "compare.open",
        json!({"current": new.file.path(), "previous": old.file.path(), "mapping": [], "elements": [], "contentThresholdPct": null}),
    )
    .await
    .unwrap();
    let name: String = serde_json::from_value(
        app.dispatch(
            ctx(),
            "export.suggestName",
            json!({"target":{"kind":"report","name":"Crawl Changes","format":"csv"}}),
        )
        .await
        .unwrap(),
    )
    .unwrap();
    assert!(name.contains("crawl_changes") && name.ends_with(".csv"), "{name}");

    let op: OpHandle =
        serde_json::from_value(app.dispatch(ctx(), "export.report", report).await.unwrap())
            .unwrap();
    let start = std::time::Instant::now();
    let status = loop {
        let status = app.ops().status(&op.op_id).unwrap();
        if status.state != OpState::Running {
            break status;
        }
        assert!(start.elapsed() < std::time::Duration::from_secs(60));
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    };
    assert_eq!(status.state, OpState::Done, "{:?}", status.error);
    let file: FileResult = serde_json::from_value(status.result.unwrap()).unwrap();
    let golden = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/golden/out-b3-diff/crawl_changes.csv");
    assert_eq!(text(std::path::Path::new(&file.path)), text(&golden));
}
