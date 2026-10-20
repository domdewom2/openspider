//! Aufgabenlauf im selben Prozess mit eigenen Erweiterungen (`TaskHooks`): API-Quellen, Vergleich,
//! Benachrichtigung, Looker-Studio-Zeitreihe, Vorlagen, Arbeitsmappe, Höchstlaufzeit
//! (OUT-164, OUT-165, OUT-171, OUT-210, OUT-211, OUT-212, OUT-075, OUT-007, INTEG-017).

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use calamine::{open_workbook, Reader, Xlsx};
use common::{file_names, site};
use os_analyze::analysis::CancelToken;
use os_cli::reporter::Reporter;
use os_cli::run_task::{
    run_in, series_table, DefaultHooks, RunCtx, SeriesHooks, SeriesSink, TaskHooks, TaskRun,
};
use os_cli::signals::Signals;
use os_export::timeseries::CellValue;
use os_fetch::MemorySecretStore;
use os_model::SystemClock;
use os_sched::history::{HistoryStore, RunRecord, RunResult};
use os_sched::tasks::{
    CrawlSpec, CustomSummary, Execution, ExportFormat, ExportOptions, Notify, Output, Schedule,
    Source, Task, TaskStore,
};

struct Fixture {
    home: tempfile::TempDir,
    secrets: Arc<MemorySecretStore>,
}

impl Fixture {
    fn new() -> Fixture {
        Fixture {
            home: tempfile::tempdir().unwrap(),
            secrets: Arc::new(MemorySecretStore::new()),
        }
    }

    fn ctx(&self) -> RunCtx {
        RunCtx {
            paths: os_store::paths::app_paths_with_home(Some(self.home.path().as_os_str())),
            clock: Arc::new(SystemClock),
            secrets: self.secrets.clone(),
            run_id: None,
            manual: true,
            log_path: None,
            export_dir: self.home.path().join("exports"),
            json_progress: false,
            heartbeat: Duration::from_millis(200),
            poll: Duration::from_millis(50),
            timeout_scale: 1.0,
        }
    }

    fn out(&self) -> PathBuf {
        self.home.path().join("out")
    }

    fn save(&self, task: &Task) {
        TaskStore::in_settings(self.home.path()).save(task.clone()).unwrap();
    }

    async fn run(&self, id: &str, hooks: &dyn TaskHooks) -> TaskRun {
        run_in(
            &self.ctx(),
            id,
            hooks,
            Signals::manual(CancelToken::new()),
            Reporter::for_task(false),
        )
        .await
    }

    fn history(&self) -> HistoryStore {
        HistoryStore::in_settings(self.home.path())
    }
}

fn task(name: &str, url: &str, out: &Path, items: &[(&str, &str)]) -> Task {
    Task {
        id: name.to_lowercase(),
        name: name.into(),
        project: "Shop".into(),
        description: String::new(),
        enabled: true,
        schedule: Schedule::Daily {
            time: "03:00".into(),
            from: None,
        },
        crawl: CrawlSpec {
            source: Source::Spider { url: url.into() },
            config: None,
            auth_profile: None,
            overrides: [("threads", "1"), ("robots.retries", "0")]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        },
        execution: Execution::default(),
        apis: Vec::new(),
        output: Output::default(),
        exports: Vec::new(),
        export_options: Some(ExportOptions {
            items: items
                .iter()
                .map(|(k, n)| (k.to_string(), n.to_string()))
                .collect(),
            format: ExportFormat::Csv,
            dir: out.to_string_lossy().into_owned(),
            single_workbook: false,
            skip_empty: false,
        }),
        preset: None,
        custom_summary: None,
        notify: Notify::default(),
    }
}

fn output_folder(run: &TaskRun) -> PathBuf {
    PathBuf::from(run.record.as_ref().unwrap().output_folder.clone().unwrap())
}

#[derive(Default)]
struct Calls {
    apis: Mutex<Vec<(Vec<String>, bool)>>,
    compare: Mutex<u32>,
    notified: Mutex<Vec<RunRecord>>,
}

/// Erweiterungen, die nur mitschreiben.
struct Recording {
    calls: Arc<Calls>,
    fail_summary: bool,
}

impl TaskHooks for Recording {
    fn fetch_apis(&self, task: &Task, crawl: Option<&Path>) -> Vec<String> {
        self.calls
            .apis
            .lock()
            .unwrap()
            .push((task.apis.clone(), crawl.is_some_and(Path::is_file)));
        Vec::new()
    }
    fn compare_with_previous(&self, _: &Task, _: Option<&Path>) -> Result<(), String> {
        *self.calls.compare.lock().unwrap() += 1;
        Ok(())
    }
    fn custom_summary(
        &self,
        _: &Task,
        _: &CustomSummary,
        _: Option<&Path>,
    ) -> Result<Vec<String>, String> {
        if self.fail_summary {
            Err("Drive quota exceeded".into())
        } else {
            Ok(Vec::new())
        }
    }
    fn notify(&self, _: &Task, record: &RunRecord) -> Result<(), String> {
        self.calls.notified.lock().unwrap().push(record.clone());
        Ok(())
    }
}

// spec: OUT-164
// spec: INTEG-017
// spec: OUT-152
// spec: OUT-281
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api_sources_comparison_and_notification_are_handed_to_the_hooks_after_the_crawl() {
    let site = site("out-basic-site").await;
    let fx = Fixture::new();
    fx.secrets.set("api/gsc", r#"{"account":"a@example.test"}"#);
    fx.secrets.set("api/psi", "key");
    fx.secrets.set("mail/work", "smtp");
    let mut t = task("Hooked", &site.base_url(), &fx.out(), &[("tab", "Internal:All")]);
    t.apis = vec!["gsc".into(), "psi".into()];
    t.execution.auto_compare = true;
    t.notify = Notify {
        on_success: true,
        on_failure: true,
        account: Some("work".into()),
        to: vec!["seo@example.test".into()],
    };
    fx.save(&t);
    let calls = Arc::new(Calls::default());
    let run = fx
        .run(
            "hooked",
            &Recording {
                calls: calls.clone(),
                fail_summary: false,
            },
        )
        .await;
    assert_eq!(run.exit.code(), 0, "{}", run.summary);
    let record = run.record.unwrap();
    assert_eq!(record.result, RunResult::Completed, "{record:?}");
    assert_eq!(
        calls.apis.lock().unwrap().as_slice(),
        [(vec!["gsc".to_string(), "psi".to_string()], true)]
    );
    assert_eq!(*calls.compare.lock().unwrap(), 1);
    let notified = calls.notified.lock().unwrap();
    assert_eq!(notified.len(), 1);
    assert_eq!(notified[0].result, RunResult::Completed);
    assert_eq!(notified[0].run_id, record.run_id);
}

// spec: OUT-164
// spec: OUT-165
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn without_extensions_requested_apis_end_as_completed_with_warnings_and_exports_stay() {
    let site = site("out-basic-site").await;
    let fx = Fixture::new();
    fx.secrets.set("api/gsc", "key");
    let mut t = task("Warned", &site.base_url(), &fx.out(), &[("tab", "Internal:All")]);
    t.apis = vec!["gsc".into()];
    fx.save(&t);
    let run = fx.run("warned", &DefaultHooks).await;
    assert_eq!(run.exit.code(), 0, "{}", run.summary);
    let record = run.record.clone().unwrap();
    assert_eq!(record.result, RunResult::CompletedWithWarnings);
    assert!(record.warnings.iter().any(|w| w.contains("API source 'gsc'")), "{:?}", record.warnings);
    assert!(output_folder(&run).join("internal_all.csv").is_file());
}

// spec: OUT-165
// spec: OUT-210
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failing_remote_target_leaves_local_exports_untouched_and_warns() {
    let site = site("out-basic-site").await;
    let fx = Fixture::new();
    fx.secrets.set("api/drive", r#"{"account":"work"}"#);
    let mut t = task("Remote", &site.base_url(), &fx.out(), &[("tab", "Internal:All")]);
    t.custom_summary = Some(CustomSummary {
        account: "work".into(),
        target: "Remote".into(),
        metrics: vec!["Internal:HTML".into()],
    });
    fx.save(&t);
    let calls = Arc::new(Calls::default());
    let run = fx
        .run("remote", &Recording { calls, fail_summary: true })
        .await;
    let record = run.record.clone().unwrap();
    assert_eq!(record.result, RunResult::CompletedWithWarnings, "{record:?}");
    assert!(record.warnings.iter().any(|w| w.contains("Drive quota exceeded")));
    assert!(output_folder(&run).join("internal_all.csv").is_file());
    assert_eq!(run.exit.code(), 0);
}

/// Speicher statt Google-Tabelle.
#[derive(Default)]
struct MemorySheets {
    tables: Mutex<BTreeMap<String, (Vec<String>, Vec<(Vec<String>, Vec<CellValue>)>)>>,
}

impl SeriesSink for MemorySheets {
    fn header(&self, table: &str) -> Result<Vec<String>, String> {
        Ok(self
            .tables
            .lock()
            .unwrap()
            .get(table)
            .map(|t| t.0.clone())
            .unwrap_or_default())
    }
    fn append(&self, table: &str, header: &[String], row: &[CellValue]) -> Result<(), String> {
        let mut tables = self.tables.lock().unwrap();
        let entry = tables.entry(table.to_string()).or_default();
        entry.0 = header.to_vec();
        entry.1.push((header.to_vec(), row.to_vec()));
        Ok(())
    }
}

fn cell<'a>(table: &'a (Vec<String>, Vec<(Vec<String>, Vec<CellValue>)>), row: usize, column: &str) -> Option<&'a CellValue> {
    let (header, cells) = &table.1[row];
    header.iter().position(|h| h == column).and_then(|i| cells.get(i))
}

// spec: OUT-210
// spec: OUT-211
// spec: OUT-212
// spec: OUT-212 [P2]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_time_series_has_one_numeric_row_per_run_stable_columns_and_a_new_table_after_a_rename() {
    let site = site("out-basic-site").await;
    let fx = Fixture::new();
    fx.secrets.set("api/drive", r#"{"account":"work"}"#);
    let mut t = task("Series", &site.base_url(), &fx.out(), &[]);
    t.custom_summary = Some(CustomSummary {
        account: "work".into(),
        target: "Series".into(),
        metrics: vec!["Internal:HTML".into(), "Response Codes:Client Error (4xx)".into()],
    });
    fx.save(&t);
    let sheets = Arc::new(MemorySheets::default());
    let hooks = SeriesHooks { sink: sheets.clone() };
    let table = series_table(&t);
    assert_eq!(table, "OpenSpider/Shop/Series_custom_summary_report");

    for _ in 0..3 {
        let run = fx.run("series", &hooks).await;
        assert_eq!(run.record.as_ref().unwrap().result, RunResult::Completed, "{}", run.summary);
    }
    {
        let tables = sheets.tables.lock().unwrap();
        let data = tables.get(&table).expect("Tabelle");
        assert_eq!(
            data.0,
            ["Date", "Start URL", "Internal:HTML", "Response Codes:Client Error (4xx)"]
        );
        assert_eq!(data.1.len(), 3, "drei Läufe, drei Datenzeilen");
        for row in 0..3 {
            assert!(matches!(cell(data, row, "Date"), Some(CellValue::Date(_))), "Datumswert");
            assert!(matches!(cell(data, row, "Start URL"), Some(CellValue::Text(_))));
            assert!(matches!(cell(data, row, "Internal:HTML"), Some(CellValue::Number(n)) if *n > 0.0));
            assert!(matches!(cell(data, row, "Response Codes:Client Error (4xx)"), Some(CellValue::Number(_))));
        }
        let dates: Vec<f64> = (0..3)
            .map(|r| match cell(data, r, "Date") {
                Some(CellValue::Date(v)) => *v,
                _ => panic!(),
            })
            .collect();
        assert!(dates[0] <= dates[1] && dates[1] <= dates[2], "chronologisch: {dates:?}");
    }

    // Umsortiert, eine Metrik entfernt, eine neue hinzugefügt, eine nicht (mehr) vorhandene gewählt.
    t.custom_summary.as_mut().unwrap().metrics = vec![
        "Response Codes:Client Error (4xx)".into(),
        "Internal:All".into(),
        "Bogus:Metric".into(),
    ];
    fx.save(&t);
    let run = fx.run("series", &hooks).await;
    let record = run.record.unwrap();
    assert_eq!(record.result, RunResult::CompletedWithWarnings, "{record:?}");
    assert!(
        record.warnings.iter().any(|w| w == "metric 'Bogus:Metric' is no longer populated"),
        "{:?}",
        record.warnings
    );
    let tables = sheets.tables.lock().unwrap();
    let data = tables.get(&table).unwrap();
    assert_eq!(
        data.0,
        [
            "Date",
            "Start URL",
            "Internal:HTML",
            "Response Codes:Client Error (4xx)",
            "Internal:All",
            "Bogus:Metric"
        ],
        "bestehende Spalten bleiben, neue kommen rechts dazu"
    );
    assert_eq!(data.1.len(), 4);
    assert!(matches!(cell(data, 3, "Internal:HTML"), Some(CellValue::Empty)), "entfernte Metrik: leere Zelle");
    assert!(matches!(cell(data, 3, "Internal:All"), Some(CellValue::Number(_))));
    assert!(matches!(cell(data, 3, "Bogus:Metric"), Some(CellValue::Empty)));
    drop(tables);

    // Umbenennen: neue Tabelle, die alte bleibt unverändert.
    let mut renamed = t.clone();
    renamed.id = "series2".into();
    renamed.name = "Series 2".into();
    fx.save(&renamed);
    let run = fx.run("series2", &hooks).await;
    assert!(run.record.is_some());
    let tables = sheets.tables.lock().unwrap();
    assert_eq!(tables.len(), 2);
    assert_eq!(tables.get(&table).unwrap().1.len(), 4);
    assert_eq!(tables.get(&series_table(&renamed)).unwrap().1.len(), 1);
}

// spec: OUT-075
// spec: OUT-007
// spec: OUT-012
// spec: OUT-165
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_preset_drives_one_workbook_skips_empty_tables_and_unknown_names_only_warn() {
    let site = site("out-basic-site").await;
    let fx = Fixture::new();
    let preset = os_export::types::ExportPreset {
        name: "weekly".into(),
        items: vec![
            os_export::types::ExportSelection {
                kind: "tab".into(),
                name: "Internal:All".into(),
            },
            os_export::types::ExportSelection {
                kind: "bulk".into(),
                name: "Links:All Inlinks".into(),
            },
            os_export::types::ExportSelection {
                kind: "tab".into(),
                name: "Response Codes:Server Error (5xx)".into(),
            },
            os_export::types::ExportSelection {
                kind: "tab".into(),
                name: "From Newer Version:Everything".into(),
            },
        ],
        single_workbook: true,
        skip_empty: true,
    };
    preset
        .save(&fx.home.path().join("export-presets").join("weekly.ospreset"))
        .unwrap();
    let mut t = task("Weekly", &site.base_url(), &fx.out(), &[]);
    t.preset = Some("weekly".into());
    t.export_options.as_mut().unwrap().format = ExportFormat::Xlsx;
    fx.save(&t);
    let run = fx.run("weekly", &DefaultHooks).await;
    let record = run.record.clone().unwrap();
    assert_eq!(record.result, RunResult::CompletedWithWarnings, "{record:?}");
    assert!(
        record.warnings.iter().any(|w| w.contains("From Newer Version")),
        "{:?}",
        record.warnings
    );
    let folder = output_folder(&run);
    let names = file_names(&folder);
    assert_eq!(names.len(), 1, "eine Arbeitsmappe statt Einzeldateien: {names:?}");
    assert!(names[0].ends_with("_export.xlsx"), "{names:?}");
    let book: Xlsx<_> = open_workbook(folder.join(&names[0])).unwrap();
    let sheets = book.sheet_names();
    assert_eq!(sheets.len(), 2, "leere Tabellen entfallen: {sheets:?}");
    assert!(sheets[0].contains("Internal"), "{sheets:?}");
    assert!(sheets[1].contains("Inlinks"), "{sheets:?}");
}

// spec: OUT-171 [P2]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reaching_the_maximum_runtime_stops_the_crawl_but_exports_the_state_so_far() {
    let site = site("out-queue").await;
    let fx = Fixture::new();
    let mut t = task("Capped", &site.base_url(), &fx.out(), &[("tab", "Internal:All")]);
    t.execution.timeout_min = Some(1);
    fx.save(&t);
    let mut ctx = fx.ctx();
    ctx.timeout_scale = 0.03; // 1 Minute -> 1,8 s
    let run = run_in(
        &ctx,
        "capped",
        &DefaultHooks,
        Signals::manual(CancelToken::new()),
        Reporter::for_task(false),
    )
    .await;
    let record = run.record.clone().unwrap();
    assert_eq!(record.result, RunResult::CompletedWithWarnings, "{record:?}");
    assert_eq!(run.exit.code(), 0);
    assert!(record.warnings.iter().any(|w| w.contains("max runtime of 1 min")), "{:?}", record.warnings);
    let csv = output_folder(&run).join("internal_all.csv");
    assert!(csv.is_file(), "Exporte des bisherigen Stands");
    let crawled = record.urls_crawled.expect("gecrawlte URLs");
    assert!((1..20).contains(&crawled), "Teilstand statt 21 Seiten: {crawled}");
    let crawls = os_store::registry::path_for(
        &os_store::paths::app_paths_with_home(Some(fx.home.path().as_os_str())).crawls,
        &record.crawl_id.as_deref().unwrap().parse().unwrap(),
    );
    let file = os_store::CrawlFile::open(&crawls, os_store::OpenMode::ReadOnly).unwrap();
    assert_eq!(file.meta().unwrap().state.as_str(), "Paused");
}

// spec: OUT-165
// spec: OUT-166
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_crawl_file_is_copied_next_to_the_exports_and_the_output_mode_is_respected() {
    let site = site("out-basic-site").await;
    let fx = Fixture::new();
    let mut t = task("Saved", &site.base_url(), &fx.out(), &[("tab", "Internal:All")]);
    t.output.save_crawl = true;
    t.output.mode = os_sched::tasks::OutputMode::Overwrite;
    fx.save(&t);
    for _ in 0..2 {
        let run = fx.run("saved", &DefaultHooks).await;
        assert_eq!(run.record.as_ref().unwrap().result, RunResult::Completed, "{}", run.summary);
    }
    // Überschreiben: derselbe Ordner <Out>/<Projekt>/<Aufgabe>, kein Zeitstempel-Unterordner.
    let folder = fx.out().join("Shop").join("Saved");
    let names = file_names(&folder);
    assert_eq!(names, ["crawl.ospider", "internal_all.csv"]);
    assert_eq!(fx.history().list(Some("saved"), 0, 10).len(), 2);
}
