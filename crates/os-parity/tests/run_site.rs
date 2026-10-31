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

fn ref_csv(port: u16) -> String {
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
            "site = \"site-a\"\nport = {port}\n\n[[export]]\nref_file = \"internal_all.csv\"\nos = \"tab:Internal:All\"\nmapping = \"internal_all\"\nstatus = \"present\"\n\n[[export]]\nref_file = \"links.csv\"\nos = \"bulk:Links:All Inlinks\"\nmapping = \"internal_all\"\nstatus = \"pending\"\n\n[[export]]\nref_file = \"psi.csv\"\nos = \"report:PageSpeed:Opportunities\"\nmapping = \"internal_all\"\nstatus = \"unavailable\"\n"
        ),
    );
    write(
        &r.join("tests/golden/parity/site-a/ref/internal_all.csv"),
        &ref_csv(port),
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
            .find(|c| c.ref_column == "Title 1")
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
    write(&stale, &ref_csv(repo.port));
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
        list_source: None,
        config: Default::default(),
        sitemap: None,
        crawl_path: None,
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
    write(&src.join("internal_all.csv"), &ref_csv(repo.port));
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
        list_source: None,
        config: Default::default(),
        sitemap: None,
        crawl_path: None,
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
    text.push_str("\n[[export]]\nref_file = \"chains.csv\"\nos = \"report:Redirects:Redirects to Errors\"\nmapping = \"internal_all\"\nstatus = \"present\"\nskip = \"OUT-084 ist P1\"\n");
    write(&manifest, &text);
    write(
        &repo
            .root()
            .join("tests/golden/parity/site-a/ref/chains.csv"),
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
        .find(|e| e.ref_file == "chains.csv")
        .unwrap();
    let Outcome::Skipped { reason } = &skipped.outcome else {
        panic!("{:?}", skipped.outcome);
    };
    assert!(reason == "OUT-084 ist P1", "{reason}");
}

#[test]
fn the_markdown_summary_lists_deviations_and_free_restrictions_separately() {
    let repo = repo();
    write(
        &repo.root().join("tests/parity/deviations.toml"),
        "[[deviation]]\nid = \"DEV-1\"\nspec = \"ONPAGE-001\"\nkind = \"Annahme\"\ncolumn = \"Title 1\"\nreason = \"Titel weicht ab\"\n[[deviation]]\nid = \"REF-FREE-01\"\nspec = \"TECH-210\"\nkind = \"Annahme\"\ncolumn = \"Status Code\"\nreason = \"Referenz-Free-Einschränkung (Spider-Konfiguration gesperrt): Test\"\n",
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
    let free = md.find("## Referenzeinschränkungen (Free)").expect(&md);
    assert!(plain < free);
    assert!(md[plain..free].contains("DEV-1") && !md[plain..free].contains("REF-FREE-01"));
    assert!(md[free..].contains("REF-FREE-01"));
}

// spec: CRAWL-029
// spec: OUT-015
#[test]
fn list_source_and_configuration_variants_are_separate_requests_with_cli_arguments() {
    let repo = repo();
    write(
        &repo.root().join("tests/fixtures/site-a/list.txt"),
        "http://127.0.0.1:8910/\n",
    );
    let manifest = repo.root().join("tests/golden/parity/site-a/manifest.toml");
    let mut text = std::fs::read_to_string(&manifest).unwrap();
    text = format!("list_source = \"list.txt\"\n{text}");
    text.push_str("\n[[export]]\nref_file = \"variant.csv\"\nos = \"tab:Internal:All\"\nmapping = \"internal_all\"\nos_file = \"internal_all.csv\"\nstatus = \"present\"\n[export.config]\n\"sitemap_gen.images.enabled\" = \"true\"\n");
    write(&manifest, &text);
    write(
        &repo
            .root()
            .join("tests/golden/parity/site-a/ref/variant.csv"),
        &ref_csv(repo.port).replace("Start", "Variant"),
    );
    let seen = std::sync::Mutex::new(Vec::new());
    let exporter = |req: &ExportRequest| {
        let args = CommandExporter::args(req);
        assert_eq!(args[0], "--crawl-list");
        assert!(req.list_source.as_ref().unwrap().is_file());
        assert!(!args.contains(&"--crawl".to_string()));
        seen.lock().unwrap().push(req.clone());
        fake(if req.config.is_empty() {
            "Start"
        } else {
            "Variant"
        })(req)
    };
    let summary = run_sites(&repo.opts(&["site-a"], false), &exporter).unwrap();
    assert!(summary.passed, "{:?}", summary.failures);
    let requests = seen.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_ne!(requests[0].out_dir, requests[1].out_dir);
    let args = CommandExporter::args(&requests[1]);
    assert!(args
        .windows(2)
        .any(|a| a == ["--set", "sitemap_gen.images.enabled=true"]));
}

// spec: OUT-023
#[test]
fn target_selection_preserves_steps_and_reports_missing_os_target_as_difference() {
    let repo = repo();
    let manifest = repo.root().join("tests/golden/parity/site-a/manifest.toml");
    write(&manifest, &format!("site = \"site-a\"\nport = {}\n[[export]]\nref_file = \"path.csv\"\nos = \"report:Crawl Path Report\"\nmapping = \"path\"\nstatus = \"present\"\n[export.select]\n\"Target Address\" = \"/deep/8.html\"\n", repo.port));
    write(&repo.root().join("tests/parity/mappings/path.toml"), "key = [\"Target Address\", \"Step\"]\n[columns]\n\"Target Address\" = \"Target Address\"\n\"Step\" = \"Step\"\n\"Address\" = \"Address\"\n");
    let data = format!("Target Address,Step,Address\nhttp://127.0.0.1:{0}/deep/8.html,0,http://127.0.0.1:{0}/deep/8.html\nhttp://127.0.0.1:{0}/deep/8.html,1,http://127.0.0.1:{0}/deep/7.html\n", repo.port);
    write(
        &repo.root().join("tests/golden/parity/site-a/ref/path.csv"),
        &data,
    );
    let exporter = |req: &ExportRequest| {
        let data = format!(
            "{data}http://127.0.0.1:{}/other.html,0,http://127.0.0.1:{}/other.html\n",
            repo.port, repo.port
        );
        write(&req.out_dir.join("crawl_path_report.csv"), &data);
        Ok(())
    };
    let summary = run_sites(&repo.opts(&["site-a"], false), &exporter).unwrap();
    assert!(summary.passed, "{:?}", summary.failures);
    let Outcome::Compared { report } = single_export(&summary) else {
        panic!();
    };
    assert_eq!(report.matched, 2);
    let missing = |req: &ExportRequest| {
        write(
            &req.out_dir.join("crawl_path_report.csv"),
            "Target Address,Step,Address\n",
        );
        Ok(())
    };
    let summary = run_sites(&repo.opts(&["site-a"], false), &missing).unwrap();
    assert!(!summary.passed);
    let Outcome::Compared { report } = single_export(&summary) else {
        panic!("Ein fehlendes OS-Ziel muss als Abweichung berichtet werden");
    };
    assert_eq!(report.missing_in_os.len(), 2);
    assert_eq!(report.agreement_pct, 0.0);
    assert!(!report.url_set_identical);
    assert_eq!(report.matched, 0);

    // Eine leere Referenzauswahl bleibt dagegen ein Konfigurationsfehler,
    // auch wenn die OS-Seite ebenfalls leer ist.
    write(
        &repo.root().join("tests/golden/parity/site-a/ref/path.csv"),
        &data.replace("/deep/8.html", "/other.html"),
    );
    let summary = run_sites(&repo.opts(&["site-a"], false), &missing).unwrap();
    assert!(matches!(single_export(&summary), Outcome::Error { message }
        if message.contains("Zielauswahl der Referenz hat keine Zeilen")));
}

// spec: OUT-120
// spec: OUT-123
// spec: OUT-126
// spec: OUT-133
#[test]
fn sitemap_xml_is_compared_by_urls_and_attributes_in_both_run_and_diff() {
    let repo = repo();
    let manifest = repo.root().join("tests/golden/parity/site-a/manifest.toml");
    write(&manifest, &format!("site = \"site-a\"\nport = {}\n[[export]]\nref_file = \"sitemap.xml\"\nos = \"sitemap:default\"\nmapping = \"sitemap_xml\"\nstatus = \"present\"\n", repo.port));
    let mapping = repo.root().join("tests/parity/mappings/sitemap_xml.toml");
    write(&mapping, "format = \"xml\"\nkey = [\"Address\"]\n[columns]\n\"Address\" = \"Address\"\n\"Priority\" = \"Priority\"\n\"Images\" = \"Images\"\n");
    let reference = repo
        .root()
        .join("tests/golden/parity/site-a/ref/sitemap.xml");
    let xml = |priority: &str, address: &str| {
        format!("<urlset xmlns='http://www.sitemaps.org/schemas/sitemap/0.9'><url><loc>http://127.0.0.1:{}/{address}</loc><priority>{priority}</priority></url></urlset>", repo.port)
    };
    write(&reference, &xml("0.9", "a.html"));
    let make_exporter = |priority: &'static str, address: &'static str| {
        let xml = xml(priority, address);
        move |req: &ExportRequest| {
            assert_eq!(req.sitemap.as_deref(), Some("default"));
            assert!(CommandExporter::args(req).contains(&"--create-sitemap".to_string()));
            write(&req.out_dir.join("sitemap.xml"), &xml);
            Ok(())
        }
    };
    let summary = run_sites(
        &repo.opts(&["site-a"], false),
        &make_exporter("0.9", "a.html"),
    )
    .unwrap();
    assert!(summary.passed, "{:?}", summary.failures);
    for (priority, address) in [("0.8", "a.html"), ("0.9", "b.html")] {
        let summary = run_sites(
            &repo.opts(&["site-a"], false),
            &make_exporter(priority, address),
        )
        .unwrap();
        assert!(!summary.passed, "changed attribute or URL must fail");
    }
    let output = repo.root().join("same.xml");
    write(&output, &xml("0.9", "a.html"));
    assert!(os_parity::diff::run(&os_parity::diff::DiffArgs {
        reference,
        os: output,
        mapping,
        ref_base: format!("http://127.0.0.1:{}", repo.port),
        os_base: format!("http://127.0.0.1:{}", repo.port),
        out: None,
        deviations: None,
        thresholds: None,
        site: None
    })
    .unwrap());
}

// spec: OUT-123
#[test]
fn sitemap_crawl_dates_normalize_but_real_lastmod_differences_fail() {
    let repo = repo();
    let manifest = repo.root().join("tests/golden/parity/site-a/manifest.toml");
    let today = os_model::Timestamp::now_utc().to_db()[..10].to_string();
    // Die Referenz liegt absichtlich an einem anderen Tag als der OS-Crawl.
    let reference_date = if today == "2026-10-30" {
        "2026-10-29"
    } else {
        "2026-10-30"
    };
    write(
        &repo.root().join("tests/parity/mappings/sitemap_xml.toml"),
        "format = \"xml\"\nkey = [\"Address\"]\n[columns]\n\"Address\" = \"Address\"\n\"Lastmod\" = \"Lastmod\"\n",
    );
    let xml = |crawl_date: &str, lastmod: &str| {
        format!("<urlset xmlns='http://www.sitemaps.org/schemas/sitemap/0.9'><url><loc>http://127.0.0.1:{}/fallback</loc><lastmod>{crawl_date}</lastmod></url><url><loc>http://127.0.0.1:{}/header</loc><lastmod>{lastmod}</lastmod></url></urlset>", repo.port, repo.port)
    };
    for (kind, explicit_server) in [("default", false), ("default", true), ("images", false)] {
        write(&manifest, &format!("site = \"site-a\"\nport = {}\nexported_at = \"{reference_date}\"\n[[export]]\nref_file = \"sitemap.xml\"\nos = \"sitemap:{kind}\"\nmapping = \"sitemap_xml\"\nstatus = \"present\"\n", repo.port));
        if explicit_server {
            let mut text = std::fs::read_to_string(&manifest).unwrap();
            text.push_str("\n[export.config]\n\"sitemap_gen.lastmod.source\" = \"server\"\n");
            write(&manifest, &text);
        }
        write(
            &repo
                .root()
                .join("tests/golden/parity/site-a/ref/sitemap.xml"),
            &xml(reference_date, "2001-01-01"),
        );
        for lastmod in ["2001-01-01", "2001-01-02", reference_date] {
            let output = xml(&today, lastmod);
            let exporter = |req: &ExportRequest| {
                let name = if kind == "images" {
                    "image-sitemap.xml"
                } else {
                    "sitemap.xml"
                };
                write(&req.out_dir.join(name), &output);
                Ok(())
            };
            let result = run_sites(&repo.opts(&["site-a"], false), &exporter).unwrap();
            assert_eq!(
                result.passed,
                lastmod == "2001-01-01",
                "{:?}",
                result.failures
            );
            assert!(matches!(single_export(&result), Outcome::Compared { .. }));
        }
    }
    // Benutzerdefinierte Datumswerte dürfen selbst bei Gleichheit mit den
    // seitenspezifischen Crawl-Daten nicht normalisiert werden.
    let mut text = std::fs::read_to_string(&manifest).unwrap();
    text.push_str("\n[export.config]\n\"sitemap_gen.lastmod.source\" = \"custom\"\n");
    write(&manifest, &text);
    let exporter = |req: &ExportRequest| {
        write(
            &req.out_dir.join("image-sitemap.xml"),
            &xml(&today, "2001-01-01"),
        );
        Ok(())
    };
    let result = run_sites(&repo.opts(&["site-a"], false), &exporter).unwrap();
    assert!(!result.passed);
    assert!(matches!(single_export(&result), Outcome::Compared { .. }));
}

// spec: OUT-015
#[test]
fn ordered_mapping_rejects_a_permutation_even_when_every_cell_and_duplicate_matches() {
    let repo = repo();
    write(
        &repo.root().join("tests/parity/mappings/internal_all.toml"),
        &(String::from("ordered = true\n") + MAPPING),
    );
    let exporter = |req: &ExportRequest| {
        let base = req.base_url.trim_end_matches('/');
        write(
            &req.out_dir.join("internal_all.csv"),
            &format!("Address,Status Code,Title 1\n{base}/a.html,404,\n{base}/,200,Start\n"),
        );
        Ok(())
    };
    let summary = run_sites(&repo.opts(&["site-a"], false), &exporter).unwrap();
    let Outcome::Compared { report } = single_export(&summary) else {
        panic!();
    };
    assert_eq!(report.agreement_pct, 100.0);
    assert!(report.url_set_identical);
    assert_eq!(report.row_order_identical, Some(false));
    assert!(!summary.passed);
    assert!(summary.failures[0].contains("Upload-Reihenfolge"));
}

// spec: OUT-080
#[test]
fn m2_presence_gate_activates_only_when_m2_exports_are_present() {
    let repo = repo_with_thresholds(
        "min_sites_present = 1\nmin_legacy_sites_present = 1\nmin_m2_sites_present = 2\n",
    );
    let site = "parity-m2-example";
    let manifest = repo
        .root()
        .join(format!("tests/golden/parity/{site}/manifest.toml"));
    let text = format!("site = \"{site}\"\nport = {}\n[[export]]\nref_file = \"internal_all.csv\"\nos = \"tab:Internal:All\"\nmapping = \"internal_all\"\nstatus = \"pending\"\n", free_port());
    write(&manifest, &text);
    let opts = repo.opts(&["site-a", site], true);
    let summary = run_sites(&opts, &fake("Start")).unwrap();
    assert!(summary.passed, "{:?}", summary.failures);
    assert_eq!(summary.sites_present, 1);

    write(
        &repo
            .root()
            .join(format!("tests/fixtures/{site}/index.html")),
        "<title>Start</title>",
    );
    let m = os_parity::manifest::Manifest::load(&manifest).unwrap();
    write(
        &repo
            .root()
            .join(format!("tests/golden/parity/{site}/ref/internal_all.csv")),
        &ref_csv(m.port),
    );
    write(&manifest, &text.replace("pending", "present"));
    let summary = run_sites(&opts, &fake("Start")).unwrap();
    assert_eq!(summary.sites_present, 2);
    assert!(!summary.passed);
    assert!(summary
        .failures
        .iter()
        .any(|f| f.contains("nur 1 M2-Sites")));

    write(
        &repo.root().join("tests/parity/thresholds.toml"),
        "min_sites_present = 1\nmin_legacy_sites_present = 1\nmin_m2_sites_present = 1\n",
    );
    let summary = run_sites(&opts, &fake("Start")).unwrap();
    assert!(summary.passed, "{:?}", summary.failures);
    let summary = run_sites(&repo.opts(&[site], true), &fake("Start")).unwrap();
    assert!(!summary.passed);
    assert!(summary
        .failures
        .iter()
        .any(|f| f.contains("nur 0 M1-Sites")));
}

// spec: OUT-008
#[test]
fn tab_and_bulk_outputs_with_the_same_cli_filename_cannot_overwrite_each_other() {
    let repo = repo();
    let manifest = repo.root().join("tests/golden/parity/site-a/manifest.toml");
    let mut text = std::fs::read_to_string(&manifest).unwrap();
    text.push_str("\n[[export]]\nref_file = \"bulk_internal_all.csv\"\nos = \"bulk:Internal:All\"\nmapping = \"internal_all\"\nstatus = \"present\"\n");
    write(&manifest, &text);
    write(
        &repo
            .root()
            .join("tests/golden/parity/site-a/ref/bulk_internal_all.csv"),
        &ref_csv(repo.port).replace("Start", "Bulk"),
    );
    let seen = std::sync::Mutex::new(Vec::new());
    let exporter = |req: &ExportRequest| {
        seen.lock().unwrap().push(req.out_dir.clone());
        assert!(req.tabs.is_empty() || req.bulk.is_empty());
        fake(if req.bulk.is_empty() { "Start" } else { "Bulk" })(req)
    };
    let summary = run_sites(&repo.opts(&["site-a"], false), &exporter).unwrap();
    assert!(summary.passed, "{:?}", summary.failures);
    let dirs = seen.lock().unwrap();
    assert_eq!(dirs.len(), 2);
    assert_ne!(dirs[0], dirs[1]);
}

// spec: OUT-080
#[test]
fn unknown_report_does_not_break_other_exports() {
    let repo = repo();
    let path = repo.root().join("tests/golden/parity/site-a/manifest.toml");
    let mut manifest = std::fs::read_to_string(&path).unwrap();
    for (file, name) in [
        ("overview.csv", "Crawl Overview"),
        ("unknown.csv", "Unknown Report"),
    ] {
        manifest.push_str(&format!("\n[[export]]\nref_file = {file:?}\nos = \"report:{name}\"\nmapping = \"internal_all\"\nos_file = {file:?}\nstatus = \"present\"\n"));
        write(
            &repo
                .root()
                .join("tests/golden/parity/site-a/ref")
                .join(file),
            &ref_csv(repo.port),
        );
    }
    write(&path, &manifest);
    let exporter = |req: &ExportRequest| {
        if req.reports.iter().any(|r| r == "Unknown Report") {
            return Err(ParityError::Invalid("unknown report".into()));
        }
        fake("Start")(req)?;
        if req.reports == ["Crawl Overview"] {
            std::fs::copy(
                req.out_dir.join("internal_all.csv"),
                req.out_dir.join("overview.csv"),
            )?;
        }
        Ok(())
    };
    let result = run_sites(&repo.opts(&["site-a"], false), &exporter).unwrap();
    let exports = &result.sites[0].exports;
    assert!(matches!(exports[0].outcome, Outcome::Compared { .. }));
    assert!(matches!(exports[3].outcome, Outcome::Compared { .. }));
    assert!(matches!(exports[4].outcome, Outcome::Error { .. }));
}

// spec: OUT-023
#[test]
fn crawl_path_uses_selection_flag_without_report_menu() {
    let req = ExportRequest {
        list_source: None,
        config: Default::default(),
        sitemap: None,
        site: "fixture".into(),
        base_url: "http://127.0.0.1:12345/".into(),
        profile: None,
        out_dir: PathBuf::from("out"),
        tabs: vec![],
        bulk: vec![],
        reports: vec![],
        crawl_path: Some("http://127.0.0.1:12345/deep/8.html".into()),
    };
    let args = CommandExporter::args(&req);
    assert!(args
        .windows(2)
        .any(|pair| pair == ["--export-crawl-path", "http://127.0.0.1:12345/deep/8.html"]));
    assert!(!args.iter().any(|arg| arg == "--save-report"));
}
