//! `os-parity run`: startet die Fixture-Site, erzeugt die Exporte laut Manifest und vergleicht alle
//! (Abschnitt 5.3/5.4).
//!
//! Verzeichnislayout unter `--root` (Standard: aktuelles Verzeichnis = Repository-Wurzel):
//!
//! ```text
//! tests/fixtures/<site>/                    Fixture (os-testsite)
//! tests/golden/parity/<site>/manifest.toml  Manifest
//! tests/golden/parity/<site>/sf/<datei>.csv Referenz-Exporte (vom Nutzer abgelegt)
//! tests/parity/mappings/<mapping>.toml      Spaltenmappings
//! tests/parity/deviations.toml              dokumentierte Abweichungen (optional)
//! tests/parity/thresholds.toml              Schwellen (optional, sonst Standardwerte im Code)
//! tests/parity/profiles/parity.osconfig     Konfiguration für Paritätsläufe
//! ```
//!
//! Der eigentliche Export läuft über den Trait [`Exporter`]: [`CommandExporter`] ruft das
//! `openspider`-Binär auf, Tests setzen einen Fake (Funktionszeiger oder Closure) ein.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use crate::csvio::read_csv;
use crate::deviations::Deviations;
use crate::diff::{compare, evaluate, CompareInput};
use crate::error::ParityError;
use crate::manifest::{ExportEntry, ExportStatus, Manifest, OsExport};
use crate::mapping::Mapping;
use crate::normalize::BaseUrls;
use crate::report::{render_markdown, write_report, Report};
use crate::thresholds::Thresholds;

/// Name der Umgebungsvariable mit dem Pfad des `openspider`-Binärs.
pub const BIN_ENV: &str = "OPENSPIDER_BIN";

/// Argumente von `os-parity run`.
#[derive(Debug, Clone, clap::Args)]
pub struct RunArgs {
    /// Name der Site (z. B. `out-basic-site`).
    #[arg(long, conflicts_with = "all")]
    pub site: Option<String>,
    /// Alle Sites mit Manifest.
    #[arg(long)]
    pub all: bool,
    /// Pfad des `openspider`-Binärs (sonst Umgebungsvariable `OPENSPIDER_BIN`).
    #[arg(long)]
    pub openspider: Option<PathBuf>,
    /// Ausgabeordner der Berichte.
    #[arg(long, default_value = "target/parity")]
    pub out: PathBuf,
    /// Wurzel des Repositories (enthält `tests/`).
    #[arg(long, default_value = ".")]
    pub root: PathBuf,
}

/// Was der Exporter erzeugen soll.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportRequest {
    /// Name der Site.
    pub site: String,
    /// Start-URL des Crawls (`http://127.0.0.1:<port>/`).
    pub base_url: String,
    /// Konfigurationsdatei für den Lauf, falls vorhanden.
    pub profile: Option<PathBuf>,
    /// Ausgabeordner für die CSV-Dateien.
    pub out_dir: PathBuf,
    /// Werte für `--export-tabs` (Kommas bereits maskiert).
    pub tabs: Vec<String>,
    /// Werte für `--bulk-export`.
    pub bulk: Vec<String>,
    /// Werte für `--save-report`.
    pub reports: Vec<String>,
}

/// Erzeugt die OpenSpider-Exporte einer Site.
pub trait Exporter {
    /// Crawlt `req.base_url` und legt die Exporte in `req.out_dir` ab.
    fn export(&self, req: &ExportRequest) -> Result<(), ParityError>;
}

impl<F> Exporter for F
where
    F: Fn(&ExportRequest) -> Result<(), ParityError>,
{
    fn export(&self, req: &ExportRequest) -> Result<(), ParityError> {
        self(req)
    }
}

/// Ruft das `openspider`-Binär auf.
#[derive(Debug, Clone)]
pub struct CommandExporter {
    /// Pfad des Binärs.
    pub binary: PathBuf,
    /// Argumente vor der eigentlichen Kommandozeile (z. B. ein Unterbefehl, wenn das Binär selbst ein Fake ist).
    pub prefix_args: Vec<OsString>,
    /// Zusätzliche Umgebungsvariablen für den Aufruf.
    pub env: Vec<(OsString, OsString)>,
}

impl CommandExporter {
    /// Exporter für das Binär unter `binary`.
    pub fn new(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: binary.into(),
            prefix_args: Vec::new(),
            env: Vec::new(),
        }
    }
}

impl CommandExporter {
    /// Kommandozeile (ohne Programmnamen) für einen Auftrag.
    pub fn args(req: &ExportRequest) -> Vec<String> {
        let mut a = vec!["--crawl".to_string(), req.base_url.clone()];
        if let Some(p) = &req.profile {
            a.push("--config".into());
            a.push(p.to_string_lossy().into_owned());
        }
        a.push("--output-folder".into());
        a.push(req.out_dir.to_string_lossy().into_owned());
        a.push("--overwrite".into());
        for (flag, values) in [
            ("--export-tabs", &req.tabs),
            ("--bulk-export", &req.bulk),
            ("--save-report", &req.reports),
        ] {
            if !values.is_empty() {
                a.push(flag.into());
                a.push(values.join(","));
            }
        }
        a
    }
}

impl Exporter for CommandExporter {
    fn export(&self, req: &ExportRequest) -> Result<(), ParityError> {
        std::fs::create_dir_all(&req.out_dir)?;
        let output = Command::new(&self.binary)
            .args(&self.prefix_args)
            .args(Self::args(req))
            .envs(self.env.iter().map(|(k, v)| (k, v)))
            .output()
            .map_err(|e| {
                ParityError::Invalid(format!("{} nicht startbar: {e}", self.binary.display()))
            })?;
        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail: Vec<&str> = stderr.lines().rev().take(5).collect();
        Err(ParityError::Invalid(format!(
            "openspider endete mit {}: {}",
            output
                .status
                .code()
                .map_or_else(|| "Signal".to_string(), |c| format!("Exit-Code {c}")),
            tail.into_iter().rev().collect::<Vec<_>>().join(" | ")
        )))
    }
}

/// Bestimmt das `openspider`-Binär: Argument vor Umgebungsvariable.
pub fn resolve_binary(arg: Option<&Path>, env: Option<OsString>) -> Result<PathBuf, ParityError> {
    if let Some(p) = arg {
        return Ok(p.to_path_buf());
    }
    match env {
        Some(v) if !v.is_empty() => Ok(PathBuf::from(v)),
        _ => Err(ParityError::Invalid(format!(
            "kein openspider-Binär angegeben (--openspider <pfad> oder Umgebungsvariable {BIN_ENV})"
        ))),
    }
}

/// Ergebnis eines Exports.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    /// Verglichen (Bericht enthält `passed`).
    Compared {
        /// Bericht.
        report: Box<Report>,
    },
    /// Nicht verglichen (Status `pending`/`unavailable`).
    Skipped {
        /// Grund.
        reason: String,
    },
    /// Fehler (Datei fehlt, Export fehlgeschlagen ...).
    Error {
        /// Meldung.
        message: String,
    },
}

/// Ergebnis eines Exports einer Site.
#[derive(Debug, Clone, Serialize)]
pub struct ExportResult {
    /// Dateiname der Referenz.
    pub sf_file: String,
    /// Mapping-Name.
    pub mapping: String,
    /// OpenSpider-Seite laut Manifest.
    pub os: String,
    /// Stand laut Manifest.
    pub status: ExportStatus,
    /// Ergebnis.
    #[serde(flatten)]
    pub outcome: Outcome,
}

impl ExportResult {
    fn ok(&self) -> bool {
        match &self.outcome {
            Outcome::Compared { report } => report.passed,
            Outcome::Skipped { .. } => true,
            Outcome::Error { .. } => false,
        }
    }
}

/// Ergebnis einer Site.
#[derive(Debug, Clone, Serialize)]
pub struct SiteResult {
    /// Name.
    pub site: String,
    /// Mindestens ein Export hat Status `present`.
    pub present: bool,
    /// Fehler beim Start der Site oder beim Export.
    pub error: Option<String>,
    /// Ergebnisse je Export.
    pub exports: Vec<ExportResult>,
}

/// Gesamtergebnis eines Laufs.
#[derive(Debug, Clone, Serialize)]
pub struct RunSummary {
    /// Ergebnisse je Site.
    pub sites: Vec<SiteResult>,
    /// Sites mit Status `present`.
    pub sites_present: usize,
    /// Geforderte Mindestzahl (nur bei `--all`).
    pub min_sites_present: Option<usize>,
    /// Alle Schwellen erfüllt.
    pub passed: bool,
    /// Gründe für ein Nichtbestehen.
    pub failures: Vec<String>,
    /// Hinweise (z. B. Warnungen zu `deviations.toml`, offene Exporte).
    pub warnings: Vec<String>,
}

/// Eingaben von [`run_sites`].
#[derive(Debug, Clone)]
pub struct RunOptions {
    /// Wurzel des Repositories.
    pub root: PathBuf,
    /// Zu prüfende Sites.
    pub sites: Vec<String>,
    /// Ausgabeordner.
    pub out: PathBuf,
    /// Bei `--all`: Mindestzahl vorhandener Sites prüfen.
    pub require_min_sites: bool,
}

impl RunOptions {
    fn parity_dir(&self) -> PathBuf {
        self.root.join("tests").join("parity")
    }
    fn golden_dir(&self, site: &str) -> PathBuf {
        self.root
            .join("tests")
            .join("golden")
            .join("parity")
            .join(site)
    }
    fn fixture_dir(&self, site: &str) -> PathBuf {
        self.root.join("tests").join("fixtures").join(site)
    }
}

/// Sites mit Manifest unter `tests/golden/parity`, alphabetisch.
pub fn discover_sites(root: &Path) -> Result<Vec<String>, ParityError> {
    let dir = root.join("tests").join("golden").join("parity");
    let mut sites = Vec::new();
    for entry in std::fs::read_dir(&dir)
        .map_err(|e| ParityError::Invalid(format!("{} nicht lesbar: {e}", dir.display())))?
    {
        let entry = entry?;
        if entry.path().join("manifest.toml").is_file() {
            sites.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    sites.sort();
    Ok(sites)
}

/// Ergebnisse in der Reihenfolge des Manifests (übersprungene und verglichene gemischt).
fn in_manifest_order(mut results: Vec<ExportResult>, manifest: &Manifest) -> Vec<ExportResult> {
    results.sort_by_key(|r| {
        manifest
            .export
            .iter()
            .position(|e| e.sf_file == r.sf_file)
            .unwrap_or(usize::MAX)
    });
    results
}

struct Prepared {
    entry: ExportEntry,
    os: OsExport,
    mapping: Mapping,
}

fn find_file(dir: &Path, name: &str, depth: u32) -> Option<PathBuf> {
    let direct = dir.join(name);
    if direct.is_file() {
        return Some(direct);
    }
    if depth == 0 {
        return None;
    }
    let mut subdirs: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    subdirs.sort();
    subdirs.iter().find_map(|d| find_file(d, name, depth - 1))
}

fn candidate_names(p: &Prepared) -> Vec<String> {
    match &p.entry.os_file {
        Some(f) => vec![f.clone()],
        None => p.os.file_candidates(),
    }
}

/// Führt die Vergleiche für die Sites aus `opts` aus und schreibt Berichte nach `opts.out`.
pub fn run_sites(opts: &RunOptions, exporter: &dyn Exporter) -> Result<RunSummary, ParityError> {
    let thresholds = Thresholds::load(&opts.parity_dir().join("thresholds.toml"))?;
    let deviations = Deviations::load(&opts.parity_dir().join("deviations.toml"))?;
    let mut warnings: Vec<String> = deviations.warnings.clone();
    let profile = {
        let p = opts.parity_dir().join("profiles").join("parity.osconfig");
        p.is_file().then_some(p)
    };
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;

    let mut sites = Vec::new();
    for site in &opts.sites {
        let result = run_site(
            opts,
            site,
            exporter,
            &thresholds,
            &deviations,
            profile.as_deref(),
            &rt,
            &mut warnings,
        )?;
        sites.push(result);
    }

    let sites_present = sites.iter().filter(|s| s.present).count();
    let mut failures = Vec::new();
    for s in &sites {
        if let Some(e) = &s.error {
            failures.push(format!("{}: {e}", s.site));
        }
        for x in s.exports.iter().filter(|x| !x.ok()) {
            let why = match &x.outcome {
                Outcome::Compared { report } => report.failures.join("; "),
                Outcome::Error { message } => message.clone(),
                Outcome::Skipped { .. } => String::new(),
            };
            failures.push(format!("{}/{}: {why}", s.site, x.sf_file));
        }
        if !s.present && !opts.require_min_sites {
            failures.push(format!(
                "{}: keine Exporte mit Status present (nichts zu vergleichen)",
                s.site
            ));
        }
    }
    let min = opts
        .require_min_sites
        .then_some(thresholds.min_sites_present);
    if let Some(min) = min {
        if sites_present < min {
            failures.push(format!(
                "nur {sites_present} Sites mit Status present, gefordert sind {min}"
            ));
        }
    }
    let summary = RunSummary {
        sites,
        sites_present,
        min_sites_present: min,
        passed: failures.is_empty(),
        failures,
        warnings,
    };
    write_summary(&opts.out, &summary)?;
    Ok(summary)
}

#[allow(clippy::too_many_arguments)]
fn run_site(
    opts: &RunOptions,
    site: &str,
    exporter: &dyn Exporter,
    thresholds: &Thresholds,
    deviations: &Deviations,
    profile: Option<&Path>,
    rt: &tokio::runtime::Runtime,
    warnings: &mut Vec<String>,
) -> Result<SiteResult, ParityError> {
    let manifest = Manifest::load(&opts.golden_dir(site).join("manifest.toml"))?;
    let mut results: Vec<ExportResult> = Vec::new();
    let mut prepared: Vec<Prepared> = Vec::new();
    let result_of = |e: &ExportEntry, outcome: Outcome| ExportResult {
        sf_file: e.sf_file.clone(),
        mapping: e.mapping.clone(),
        os: e.os.clone(),
        status: e.status,
        outcome,
    };
    for e in &manifest.export {
        match e.status {
            ExportStatus::Present => {
                let prep = OsExport::parse(&e.os).and_then(|os| {
                    let mapping = Mapping::load(
                        &opts
                            .parity_dir()
                            .join("mappings")
                            .join(format!("{}.toml", e.mapping)),
                    )?;
                    Ok(Prepared {
                        entry: e.clone(),
                        os,
                        mapping,
                    })
                });
                match prep {
                    Ok(p) => prepared.push(p),
                    Err(err) => results.push(result_of(
                        e,
                        Outcome::Error {
                            message: err.to_string(),
                        },
                    )),
                }
            }
            ExportStatus::Pending => {
                warnings.push(format!(
                    "{site}/{}: Referenz-Export noch nicht abgelegt (pending)",
                    e.sf_file
                ));
                results.push(result_of(
                    e,
                    Outcome::Skipped {
                        reason: "pending".into(),
                    },
                ));
            }
            ExportStatus::Unavailable => results.push(result_of(
                e,
                Outcome::Skipped {
                    reason: "in der Referenz nicht exportierbar (unavailable)".into(),
                },
            )),
        }
    }
    let present = manifest.present().next().is_some();
    if prepared.is_empty() {
        return Ok(SiteResult {
            site: site.to_string(),
            present,
            error: None,
            exports: in_manifest_order(results, &manifest),
        });
    }

    let os_dir = opts.out.join(site).join("os");
    std::fs::create_dir_all(&os_dir)?;
    // Veraltete Ausgaben früherer Läufe dürfen keinen Treffer vortäuschen.
    for p in &prepared {
        for name in candidate_names(p) {
            let _ = std::fs::remove_file(os_dir.join(name));
        }
    }

    let site_error = |results: &mut Vec<ExportResult>, prepared: &[Prepared], msg: &str| {
        for p in prepared {
            results.push(result_of(
                &p.entry,
                Outcome::Error {
                    message: msg.to_string(),
                },
            ));
        }
    };

    let started = rt.block_on(os_testsite::TestSite::start_dir_with(
        opts.fixture_dir(site),
        os_testsite::SiteOptions {
            port: manifest.port,
            ..Default::default()
        },
    ));
    let test_site = match started {
        Ok(s) => s,
        Err(e) => {
            let msg = format!("Site {site} nicht startbar: {e}");
            site_error(&mut results, &prepared, &msg);
            return Ok(SiteResult {
                site: site.to_string(),
                present,
                error: Some(msg),
                exports: in_manifest_order(results, &manifest),
            });
        }
    };
    let os_base = test_site.base_url();
    let sf_base = if manifest.port != 0 {
        format!("http://127.0.0.1:{}", manifest.port)
    } else {
        os_base.clone()
    };

    let mut req = ExportRequest {
        site: site.to_string(),
        base_url: format!("{os_base}/"),
        profile: profile.map(Path::to_path_buf),
        out_dir: os_dir.clone(),
        tabs: Vec::new(),
        bulk: Vec::new(),
        reports: Vec::new(),
    };
    for p in &prepared {
        let list = match p.os {
            OsExport::Tab { .. } => &mut req.tabs,
            OsExport::Bulk(_) => &mut req.bulk,
            OsExport::Report(_) => &mut req.reports,
        };
        let v = p.os.cli_value();
        if !list.contains(&v) {
            list.push(v);
        }
    }
    if let Err(e) = exporter.export(&req) {
        let msg = format!("OpenSpider-Export fehlgeschlagen: {e}");
        site_error(&mut results, &prepared, &msg);
        return Ok(SiteResult {
            site: site.to_string(),
            present,
            error: Some(msg),
            exports: in_manifest_order(results, &manifest),
        });
    }
    drop(test_site);

    let sf_bases = BaseUrls::from_base(&sf_base);
    let os_bases = BaseUrls::from_base(&os_base);
    for p in &prepared {
        let outcome = compare_one(
            opts, site, p, &os_dir, &sf_bases, &os_bases, thresholds, deviations,
        );
        let outcome = match outcome {
            Ok(report) => Outcome::Compared {
                report: Box::new(report),
            },
            Err(e) => Outcome::Error {
                message: e.to_string(),
            },
        };
        if let Outcome::Compared { report } = &outcome {
            let stem = Path::new(&p.entry.sf_file)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.entry.mapping.clone());
            write_report(&opts.out.join(site).join(stem), report)?;
        }
        results.push(result_of(&p.entry, outcome));
    }
    Ok(SiteResult {
        site: site.to_string(),
        present,
        error: None,
        exports: in_manifest_order(results, &manifest),
    })
}

#[allow(clippy::too_many_arguments)]
fn compare_one(
    opts: &RunOptions,
    site: &str,
    p: &Prepared,
    os_dir: &Path,
    sf_bases: &BaseUrls,
    os_bases: &BaseUrls,
    thresholds: &Thresholds,
    deviations: &Deviations,
) -> Result<Report, ParityError> {
    let names = candidate_names(p);
    let os_path = names
        .iter()
        .find_map(|n| find_file(os_dir, n, 3))
        .ok_or_else(|| {
            ParityError::Invalid(format!(
                "OpenSpider-Ausgabe für {:?} nicht gefunden (gesucht: {})",
                p.entry.os,
                names.join(", ")
            ))
        })?;
    let sf_path = opts.golden_dir(site).join("sf").join(&p.entry.sf_file);
    let sf = read_csv(&sf_path, &p.mapping.drop_sf_columns)?;
    let os = read_csv(&os_path, &[])?;
    let devs = deviations.for_export(Some(site), &p.entry.mapping);
    let mut report = compare(&CompareInput {
        export: &p.entry.mapping,
        site: Some(site),
        sf: &sf,
        os: &os,
        mapping: &p.mapping,
        sf_bases,
        os_bases,
        deviations: &devs,
    })?;
    evaluate(&mut report, thresholds);
    Ok(report)
}

fn write_summary(out: &Path, s: &RunSummary) -> Result<(), ParityError> {
    std::fs::create_dir_all(out)?;
    let json = serde_json::to_string_pretty(s)
        .map_err(|e| ParityError::Invalid(format!("Bericht nicht serialisierbar: {e}")))?;
    std::fs::write(out.join("report.json"), json + "\n")?;
    std::fs::write(out.join("report.md"), summary_markdown(s))?;
    Ok(())
}

/// Markdown-Übersicht eines Laufs.
pub fn summary_markdown(s: &RunSummary) -> String {
    use std::fmt::Write as _;
    let mut m = String::new();
    let _ = writeln!(
        m,
        "# Paritätslauf: {}\n",
        if s.passed {
            "bestanden"
        } else {
            "nicht bestanden"
        }
    );
    let _ = writeln!(m, "Sites mit Status present: {}", s.sites_present);
    if let Some(min) = s.min_sites_present {
        let _ = writeln!(m, "(gefordert: {min})");
    }
    m.push('\n');
    let _ = writeln!(
        m,
        "| Site | Export | Mapping | Zellen | Übereinstimmung | Schwelle | Ergebnis |\n|---|---|---|---:|---:|---:|---|"
    );
    for site in &s.sites {
        for x in &site.exports {
            let (cells, agree, thr, res) = match &x.outcome {
                Outcome::Compared { report } => (
                    report.cells_compared.to_string(),
                    format!("{:.3} %", report.agreement_pct),
                    format!("{:.1} %", report.threshold_pct),
                    if report.passed { "ok" } else { "FEHLER" }.to_string(),
                ),
                Outcome::Skipped { reason } => (
                    "-".into(),
                    "-".into(),
                    "-".into(),
                    format!("übersprungen ({reason})"),
                ),
                Outcome::Error { message } => (
                    "-".into(),
                    "-".into(),
                    "-".into(),
                    format!("FEHLER: {message}"),
                ),
            };
            let _ = writeln!(
                m,
                "| {} | {} | {} | {} | {} | {} | {} |",
                site.site,
                x.sf_file,
                x.mapping,
                cells,
                agree,
                thr,
                res.replace('|', "/")
            );
        }
    }
    m.push('\n');
    if !s.failures.is_empty() {
        let _ = writeln!(m, "## Fehler\n");
        for f in &s.failures {
            let _ = writeln!(m, "- {f}");
        }
        m.push('\n');
    }
    if !s.warnings.is_empty() {
        let _ = writeln!(m, "## Hinweise\n");
        for w in &s.warnings {
            let _ = writeln!(m, "- {w}");
        }
        m.push('\n');
    }
    for site in &s.sites {
        for x in &site.exports {
            if let Outcome::Compared { report } = &x.outcome {
                if !report.passed {
                    m.push_str(&render_markdown(report));
                }
            }
        }
    }
    m
}

/// Führt den Lauf aus; `Ok(true)` = alle Schwellen erfüllt.
pub fn run(args: &RunArgs) -> Result<bool, ParityError> {
    let (sites, all) = match (&args.site, args.all) {
        (Some(s), false) => (vec![s.clone()], false),
        (None, true) => (discover_sites(&args.root)?, true),
        _ => {
            return Err(ParityError::Invalid(
                "entweder --site <name> oder --all angeben".into(),
            ))
        }
    };
    let binary = resolve_binary(args.openspider.as_deref(), std::env::var_os(BIN_ENV))?;
    if !binary.is_file() {
        return Err(ParityError::Invalid(format!(
            "openspider-Binär {} existiert nicht",
            binary.display()
        )));
    }
    let summary = run_sites(
        &RunOptions {
            root: args.root.clone(),
            sites,
            out: args.out.clone(),
            require_min_sites: all,
        },
        &CommandExporter::new(binary),
    )?;
    print!("{}", summary_markdown(&summary));
    Ok(summary.passed)
}
