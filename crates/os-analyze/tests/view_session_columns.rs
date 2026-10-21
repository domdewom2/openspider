//! A01-01: Spalten mit benannten SQL-Parametern (Content-Spalten im Tab „Internal“ und „Content“) in der Sitzungsansicht.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
#[path = "framework_common.rs"]
mod common;

use os_analyze::views::session::{Budget, RowsReq, SessionSpec, ViewSession};
use os_analyze::views::ColumnSelection;
use os_model::CrawlConfig;

const PARAM_COLUMNS: [&str; 7] = [
    "Flesch Reading Ease Score",
    "Readability",
    "Closest Similarity Match",
    "No. Near Duplicates",
    "Spelling Errors",
    "Grammar Errors",
    "Language",
];

// spec: UI-004
// spec: UI-007
#[test]
fn a01_01_internal_and_content_rows_open_with_every_catalog_column() {
    let _serial = SERIAL.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = common::create(dir.path(), "params.ospider", 5);
    common::insert(
        &file,
        &[
            common::row(1, "https://s.test/a").int("status_code", 200),
            common::row(2, "https://s.test/b").int("status_code", 200),
        ],
    );
    file.conn()
        .execute("UPDATE urls SET content_class = 'HTML'", [])
        .unwrap();
    common::set_change_seq(&file, 3);
    file.set_meta("state", "Completed").unwrap();
    let cfg = CrawlConfig::default();
    for tab in ["Internal", "Content"] {
        let catalog = os_analyze::tabs::configured_for_file(&cfg, &file)
            .into_iter()
            .find(|t| t.id == tab)
            .unwrap();
        // Alle Katalogspalten wie die Oberfläche (controller.ts `view.open`), dann nur die sieben mit Parametern.
        let mut all: Vec<String> = catalog.columns.iter().map(|c| c.key.to_string()).collect();
        all.dedup();
        for columns in [
            all.clone(),
            PARAM_COLUMNS
                .iter()
                .filter(|c| all.iter().any(|a| a == *c))
                .map(|c| c.to_string())
                .collect(),
        ] {
            assert!(!columns.is_empty());
            let spec = SessionSpec {
                tab: tab.into(),
                filter: "All".into(),
                search: None,
                sort: vec![],
                columns: ColumnSelection::Named(columns.clone()),
                restrict: vec![],
            };
            let session = ViewSession::open(&file, &cfg, spec, Budget::default()).unwrap();
            let rows = session
                .rows(
                    &file,
                    RowsReq {
                        version: session.version(),
                        offset: 0,
                        limit: 10,
                    },
                )
                .unwrap_or_else(|e| panic!("{tab}: {e}"));
            assert_eq!(rows.rows.len(), 2, "{tab}");
            assert!(rows.columns.len() >= columns.len(), "{tab}");
        }
    }
}
