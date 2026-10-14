//! Tests von `os-parity run`: Site starten, Exporter aufrufen (Fake bzw. Hilfsbinär), vergleichen, Exit-Logik.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};

use os_parity::error::ParityError;
use os_parity::run::{
    discover_sites, resolve_binary, run, run_sites, CommandExporter, ExportRequest, Exporter,
    Outcome, RunArgs, RunOptions, RunSummary,
};

const MAPPING: &str = "key = [\"Address\"]\n[columns]\n\"Address\" = \"Address\"\n\"Status Code\" = \"Status Code\"\n\"Title 1\" = \"Title 1\"\n";

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

struct Repo {
    dir: tempfile::TempDir,
    port: u16,
}

impl Repo {
    fn root(&self) -> &Path {
        self.dir.path()
    }
    fn out(&self) -> PathBuf {
        self.dir.path().join("out")
    }
    fn opts(&self, sites: &[&str], all: bool) -> RunOptions {
        RunOptions {
            root: self.root().to_path_buf(),
            sites: sites.iter().map(|s| s.to_string()).collect(),
            out: self.out(),
            require_min_sites: all,
        }
    }
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn sf_csv(port: u16) -> String {
    format!(
        "Address,Status Code,Title 1\nhttp://127.0.0.1:{port}/,200,Start\nhttp://127.0.0.1:{port}/a.html,404,\n"
    )
}

/// Legt ein Repository mit Site `site-a` an: ein Export `present`, einer `pending`, einer `unavailable`.
fn repo() -> Repo {
    let dir = tempfile::tempdir().unwrap();
    let port = free_port();
    let r = dir.path();
    write(
        &r.join("tests/fixtures/site-a/index.html"),
        "<html><title>Start</title></html>",
    );
    write(
        &r.join("tests/golden/parity/site-a/manifest.toml"),
        &format!(
            "site = \"site-a\"\nport = {port}\n\n[[export]]\nsf_file = \"internal_all.csv\"\nos = \"tab:Internal:All\"\nmapping = \"internal_all\"\nstatus = \"present\"\n\n[[export]]\nsf_file = \"links.csv\"\nos = \"bulk:Links:All Inlinks\"\nmapping = \"internal_all\"\nstatus = \"pending\"\n\n[[export]]\nsf_file = \"psi.csv\"\nos = \"report:PageSpeed:Opportunities\"\nmapping = \"internal_all\"\nstatus = \"unavailable\"\n"
        ),
    );
    write(
        &r.join("tests/golden/parity/site-a/sf/internal_all.csv"),
        &sf_csv(port),
    );
    write(&r.join("tests/parity/mappings/internal_all.toml"), MAPPING);
    write(
        &r.join("tests/parity/profiles/parity.osconfig"),
        "schema_version = 1\n",
    );
    Repo { dir, port }
}

fn http_status(base: &str) -> String {
    let addr = base.trim_start_matches("http://").trim_end_matches('/');
    let mut s = TcpStream::connect(addr).unwrap();
    s.write_all(b"GET /index.html HTTP/1.0\r\nHost: x\r\n\r\n")
        .unwrap();
    let mut buf = String::new();
    s.read_to_string(&mut buf).unwrap();
    buf.lines().next().unwrap_or("").to_string()
}

/// Fake-Exporter: schreibt `internal_all.csv` mit den URLs des laufenden Servers.
fn fake(title_of_start: &'static str) -> impl Fn(&ExportRequest) -> Result<(), ParityError> {
    move |req: &ExportRequest| {
        let status = http_status(&req.base_url);
        assert!(status.contains("200"), "Site muss laufen: {status}");
        let base = req.base_url.trim_end_matches('/');
        std::fs::create_dir_all(&req.out_dir)?;
        std::fs::write(
            req.out_dir.join("internal_all.csv"),
            format!(
                "Address,Status Code,Title 1\n{base}/,200,{title_of_start}\n{base}/a.html,404,\n"
            ),
        )?;
        Ok(())
    }
}

fn single_export(s: &RunSummary) -> &Outcome {
    &s.sites[0].exports[0].outcome
}

#[test]
fn matching_export_passes_and_writes_reports() {
    let repo = repo();
    let seen = std::sync::Mutex::new(None);
    let inner = fake("Start");
    let exporter = |req: &ExportRequest| {
        *seen.lock().unwrap() = Some(req.clone());
        inner(req)
    };
    let s = run_sites(&repo.opts(&["site-a"], false), &exporter).unwrap();
    assert!(s.passed, "{:?}", s.failures);
    assert_eq!(s.sites_present, 1);
    let Outcome::Compared { report } = single_export(&s) else {
        panic!("{:?}", single_export(&s));
    };
    assert_eq!(report.matched, 2);
    assert_eq!(report.agreement_pct, 100.0);
    assert!(report.url_set_identical);
    // Auftrag an den Exporter.
    let req = seen.lock().unwrap().clone().unwrap();
    assert_eq!(req.tabs, ["Internal:All"]);
    assert!(
        req.bulk.is_empty() && req.reports.is_empty(),
        "nur present-Exporte werden erzeugt"
    );
    assert!(req.base_url.ends_with('/'));
    assert!(req.profile.as_ref().unwrap().ends_with("parity.osconfig"));
    // Berichte.
    assert!(repo.out().join("report.json").is_file());
    assert!(repo.out().join("report.md").is_file());
    let per_export = repo.out().join("site-a/internal_all");
    assert!(per_export.join("report.json").is_file());
    assert!(per_export.join("report.md").is_file());
    // pending/unavailable erscheinen als übersprungen, mit Hinweis.
    assert_eq!(s.sites[0].exports.len(), 3);
    assert!(s.sites[0]
        .exports
        .iter()
        .skip(1)
        .all(|e| matches!(e.outcome, Outcome::Skipped { .. })));
    assert!(s.warnings.iter().any(|w| w.contains("links.csv")));
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(repo.out().join("report.json")).unwrap())
            .unwrap();
    assert_eq!(json["passed"], true);
}

#[test]
fn differing_export_fails_the_run() {
    let repo = repo();
    let s = run_sites(&repo.opts(&["site-a"], false), &fake("Anderer Titel")).unwrap();
    assert!(!s.passed);
    let Outcome::Compared { report } = single_export(&s) else {
        panic!();
    };
    assert!(!report.passed);
    assert_eq!(
        report
            .columns
            .iter()
            .find(|c| c.sf_column == "Title 1")
            .unwrap()
            .errors,
        1
    );
    assert!(s
        .failures
        .iter()
        .any(|f| f.contains("site-a/internal_all.csv")));
}

#[test]
fn deviations_and_thresholds_files_are_applied() {
    let repo = repo();
    write(
        &repo.root().join("tests/parity/deviations.toml"),
        "[[deviation]]\nid = \"DEV-1\"\nspec = \"ONPAGE-001\"\nkind = \"Annahme\"\nexport = \"internal_all\"\ncolumn = \"Title 1\"\nnote = \"Titel weicht ab\"\n",
    );
    let s = run_sites(&repo.opts(&["site-a"], false), &fake("Anderer Titel")).unwrap();
    assert!(s.passed, "{:?}", s.failures);
    let Outcome::Compared { report } = single_export(&s) else {
        panic!();
    };
    assert_eq!(report.applied_deviations.len(), 1);
    assert_eq!(report.applied_deviations[0].spec, "ONPAGE-001");

    // thresholds.toml: Schwelle 100 % für diesen Export.
    let repo = repo_with_thresholds("[agreement_pct]\n\"internal_all\" = 100.0\n");
    let s = run_sites(&repo.opts(&["site-a"], false), &fake("Anderer Titel")).unwrap();
    let Outcome::Compared { report } = single_export(&s) else {
        panic!();
    };
    assert_eq!(report.threshold_pct, 100.0);
    assert!(!s.passed);
}

fn repo_with_thresholds(toml: &str) -> Repo {
    let repo = repo();
    write(&repo.root().join("tests/parity/thresholds.toml"), toml);
    repo
}

#[test]
fn all_mode_requires_enough_present_sites() {
    let repo = repo();
    let s = run_sites(&repo.opts(&["site-a"], true), &fake("Start")).unwrap();
    assert!(!s.passed);
    assert_eq!(s.min_sites_present, Some(4));
    assert!(s.failures.iter().any(|f| f.contains("nur 1 Sites")));

    let repo = repo_with_thresholds("min_sites_present = 1\n");
    let s = run_sites(&repo.opts(&["site-a"], true), &fake("Start")).unwrap();
    assert!(s.passed, "{:?}", s.failures);
}

#[test]
fn site_without_present_exports_does_not_crawl_and_fails_single_run() {
    let repo = repo();
    let manifest = repo.root().join("tests/golden/parity/site-a/manifest.toml");
    let text = std::fs::read_to_string(&manifest)
        .unwrap()
        .replace("\"present\"", "\"pending\"");
    std::fs::write(&manifest, text).unwrap();
    let called = std::cell::Cell::new(false);
    let exporter = |_: &ExportRequest| {
        called.set(true);
        Ok(())
    };
    let s = run_sites(&repo.opts(&["site-a"], false), &exporter).unwrap();
    assert!(!called.get(), "ohne present-Export kein Crawl");
    assert!(!s.passed);
    assert!(!s.sites[0].present);
    assert!(s
        .failures
        .iter()
        .any(|f| f.contains("keine Exporte mit Status present")));
}

#[test]
fn exporter_failure_marks_all_exports_as_errors() {
    let repo = repo();
    let exporter = |_: &ExportRequest| Err(ParityError::Invalid("kaputt".into()));
    let s = run_sites(&repo.opts(&["site-a"], false), &exporter).unwrap();
    assert!(!s.passed);
    assert!(s.sites[0].error.as_deref().unwrap().contains("kaputt"));
    assert!(
        matches!(single_export(&s), Outcome::Error { .. })
            || s.sites[0]
                .exports
                .iter()
                .any(|e| matches!(e.outcome, Outcome::Error { .. }))
    );
}

#[test]
fn missing_output_file_is_an_error_and_stale_files_do_not_count() {
    let repo = repo();
    // Veraltete Datei früherer Läufe, die zufällig passen würde.
    let stale = repo.out().join("site-a/os/internal_all.csv");
    write(&stale, &sf_csv(repo.port));
    let exporter = |_: &ExportRequest| Ok(());
    let s = run_sites(&repo.opts(&["site-a"], false), &exporter).unwrap();
    assert!(!s.passed);
    let err = s.sites[0]
        .exports
        .iter()
        .find_map(|e| match &e.outcome {
            Outcome::Error { message } => Some(message.clone()),
            _ => None,
        })
        .expect("Fehler erwartet");
    assert!(err.contains("internal_all.csv"), "{err}");
}

#[test]
fn unknown_mapping_or_site_is_reported() {
    let repo = repo();
    std::fs::remove_file(repo.root().join("tests/parity/mappings/internal_all.toml")).unwrap();
    let s = run_sites(&repo.opts(&["site-a"], false), &fake("Start")).unwrap();
    assert!(!s.passed);
    assert!(s.failures.iter().any(|f| f.contains("Mapping")));
    assert!(run_sites(&repo.opts(&["gibt-es-nicht"], false), &fake("Start")).is_err());
}

#[test]
fn site_start_failure_is_reported() {
    let repo = repo();
    std::fs::remove_dir_all(repo.root().join("tests/fixtures/site-a")).unwrap();
    let s = run_sites(&repo.opts(&["site-a"], false), &fake("Start")).unwrap();
    assert!(!s.passed);
    assert!(s.sites[0]
        .error
        .as_deref()
        .unwrap()
        .contains("nicht startbar"));
}

#[test]
fn discover_sites_lists_directories_with_manifest() {
    let repo = repo();
    write(
        &repo.root().join("tests/golden/parity/zz/manifest.toml"),
        "site = \"zz\"\n",
    );
    write(
        &repo
            .root()
            .join("tests/golden/parity/ohne-manifest/readme.txt"),
        "x",
    );
    assert_eq!(discover_sites(repo.root()).unwrap(), ["site-a", "zz"]);
}

#[test]
fn binary_resolution_prefers_argument_over_environment() {
    let arg = PathBuf::from("/x/openspider");
    assert_eq!(
        resolve_binary(Some(&arg), Some("/env/openspider".into())).unwrap(),
        arg
    );
    assert_eq!(
        resolve_binary(None, Some("/env/openspider".into())).unwrap(),
        PathBuf::from("/env/openspider")
    );
    assert!(resolve_binary(None, None).is_err());
    assert!(resolve_binary(None, Some("".into())).is_err());
}

#[test]
fn run_rejects_missing_binary_and_missing_site_selection() {
    let repo = repo();
    let mut args = RunArgs {
        site: Some("site-a".into()),
        all: false,
        openspider: Some(repo.root().join("gibt-es-nicht")),
        out: repo.out(),
        root: repo.root().to_path_buf(),
    };
    let err = run(&args).unwrap_err().to_string();
    assert!(err.contains("existiert nicht"), "{err}");
    args.site = None;
    assert!(run(&args).is_err());
}

#[test]
fn command_line_for_openspider_follows_the_cli_spec() {
    let req = ExportRequest {
        site: "s".into(),
        base_url: "http://127.0.0.1:8902/".into(),
        profile: Some(PathBuf::from("p.osconfig")),
        out_dir: PathBuf::from("o"),
        tabs: vec![
            "Internal:All".into(),
            "Response Codes:Client Error (4xx)".into(),
        ],
        bulk: vec!["Links:All Inlinks".into()],
        reports: vec!["Crawl Overview".into()],
    };
    assert_eq!(
        CommandExporter::args(&req),
        [
            "--crawl",
            "http://127.0.0.1:8902/",
            "--config",
            "p.osconfig",
            "--output-folder",
            "o",
            "--overwrite",
            "--export-tabs",
            "Internal:All,Response Codes:Client Error (4xx)",
            "--bulk-export",
            "Links:All Inlinks",
            "--save-report",
            "Crawl Overview",
        ]
    );
    let minimal = ExportRequest {
        profile: None,
        tabs: vec!["Internal:All".into()],
        bulk: vec![],
        reports: vec![],
        ..req
    };
    let a = CommandExporter::args(&minimal);
    assert!(!a.contains(&"--config".to_string()));
    assert!(!a.contains(&"--bulk-export".to_string()));
}

#[test]
fn command_exporter_runs_the_binary_end_to_end() {
    let repo = repo();
    let fake_bin = PathBuf::from(env!("CARGO_BIN_EXE_os-parity"));
    let src = repo.root().join("fake-src");
    write(&src.join("internal_all.csv"), &sf_csv(repo.port));
    let mut exporter = CommandExporter::new(fake_bin);
    exporter
        .prefix_args
        .extend(["fake-openspider".into(), "--".into()]);
    exporter
        .env
        .push(("FAKE_OPENSPIDER_SRC".into(), src.into()));
    let s = run_sites(&repo.opts(&["site-a"], false), &exporter).unwrap();
    assert!(s.passed, "{:?}", s.failures);
    let args = std::fs::read_to_string(repo.out().join("site-a/os/args.txt")).unwrap();
    let lines: Vec<&str> = args.lines().collect();
    assert_eq!(
        lines[0], "--crawl",
        "Unterbefehl gehört nicht zur Kommandozeile von openspider"
    );
    assert!(lines[1].starts_with("http://127.0.0.1:"));
    assert!(lines.contains(&"--export-tabs"));
    assert!(lines.contains(&"Internal:All"));
}

#[test]
fn command_exporter_reports_nonzero_exit() {
    let repo = repo();
    let mut exporter = CommandExporter::new(PathBuf::from(env!("CARGO_BIN_EXE_os-parity")));
    exporter
        .prefix_args
        .extend(["fake-openspider".into(), "--".into()]);
    exporter
        .env
        .push(("FAKE_OPENSPIDER_EXIT".into(), "3".into()));
    let s = run_sites(&repo.opts(&["site-a"], false), &exporter).unwrap();
    assert!(!s.passed);
    let msg = s.sites[0].error.clone().unwrap();
    assert!(msg.contains("Exit-Code 3"), "{msg}");
    assert!(
        msg.contains("erzwungener Fehler"),
        "stderr wird übernommen: {msg}"
    );
    // Nicht startbares Binär.
    let bad = CommandExporter::new(repo.root().join("nix"));
    let req = ExportRequest {
        site: "s".into(),
        base_url: "http://x/".into(),
        profile: None,
        out_dir: repo.root().join("o"),
        tabs: vec![],
        bulk: vec![],
        reports: vec![],
    };
    assert!(bad.export(&req).is_err());
}

#[test]
fn a_present_export_with_a_skip_reason_is_not_requested_and_reported_as_out_of_scope() {
    let repo = repo();
    let manifest = repo.root().join("tests/golden/parity/site-a/manifest.toml");
    let mut text = std::fs::read_to_string(&manifest).unwrap();
    text.push_str("\n[[export]]\nsf_file = \"chains.csv\"\nos = \"report:Redirects:Redirects to Errors\"\nmapping = \"internal_all\"\nstatus = \"present\"\nskip = \"OUT-084 ist P1\"\n");
    write(&manifest, &text);
    write(
        &repo.root().join("tests/golden/parity/site-a/sf/chains.csv"),
        "Address\nhttp://x/\n",
    );
    let seen = std::sync::Mutex::new(None);
    let inner = fake("Start");
    let exporter = |req: &ExportRequest| {
        *seen.lock().unwrap() = Some(req.clone());
        inner(req)
    };
    let s = run_sites(&repo.opts(&["site-a"], false), &exporter).unwrap();
    assert!(s.passed, "{:?}", s.failures);
    let req = seen.lock().unwrap().clone().unwrap();
    assert!(
        req.reports.is_empty(),
        "übersprungene Exporte werden nicht angefordert: {req:?}"
    );
    let skipped = s.sites[0]
        .exports
        .iter()
        .find(|e| e.sf_file == "chains.csv")
        .unwrap();
    let Outcome::Skipped { reason } = &skipped.outcome else {
        panic!("{:?}", skipped.outcome);
    };
    assert!(
        reason.contains("außerhalb des M1-Umfangs") && reason.contains("OUT-084"),
        "{reason}"
    );
}

#[test]
fn the_markdown_summary_lists_deviations_and_free_restrictions_separately() {
    let repo = repo();
    write(
        &repo.root().join("tests/parity/deviations.toml"),
        "[[deviation]]\nid = \"DEV-1\"\nspec = \"ONPAGE-001\"\nkind = \"Annahme\"\ncolumn = \"Title 1\"\nreason = \"Titel weicht ab\"\n[[deviation]]\nid = \"SF-FREE-01\"\nspec = \"TECH-210\"\nkind = \"Annahme\"\ncolumn = \"Status Code\"\nreason = \"SF-Free-Einschränkung (Spider-Konfiguration gesperrt): Test\"\n",
    );
    // Titel und Statuscode weichen ab: beide Abweichungen greifen.
    let exporter = |req: &ExportRequest| {
        let base = req.base_url.trim_end_matches('/');
        std::fs::create_dir_all(&req.out_dir)?;
        std::fs::write(
            req.out_dir.join("internal_all.csv"),
            format!("Address,Status Code,Title 1\n{base}/,201,Anders\n{base}/a.html,404,\n"),
        )?;
        Ok(())
    };
    let s = run_sites(&repo.opts(&["site-a"], false), &exporter).unwrap();
    let md = os_parity::run::summary_markdown(&s);
    let plain = md.find("## Begründete Paritätsabweichungen").expect(&md);
    let free = md.find("## Referenzeinschränkungen SF Free").expect(&md);
    assert!(plain < free);
    assert!(md[plain..free].contains("DEV-1") && !md[plain..free].contains("SF-FREE-01"));
    assert!(md[free..].contains("SF-FREE-01"));
}
