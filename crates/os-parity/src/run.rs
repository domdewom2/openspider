//! `os-parity run`: startet die Fixture-Site, erzeugt die Exporte laut Manifest und vergleicht alle
//! (Abschnitt 5.3/5.4).
//!
//! Verzeichnislayout unter `--root` (Standard: aktuelles Verzeichnis = Repository-Wurzel):
//!
//! ```text
//! tests/fixtures/<site>/                    Fixture (os-testsite)
//! tests/golden/parity/<site>/manifest.toml  Manifest
//! tests/golden/parity/<site>/ref/<datei>.csv Referenz-Exporte (vom Nutzer abgelegt)
//! tests/parity/mappings/<mapping>.toml      Spaltenmappings
//! tests/parity/deviations.toml              dokumentierte Abweichungen (optional)
//! tests/parity/thresholds.toml              Schwellen (optional, sonst Standardwerte im Code)
//! tests/parity/profiles/parity.osconfig     Konfiguration für Paritätsläufe
//! ```
//!
//! Der eigentliche Export läuft über den Trait [`Exporter`]: [`CommandExporter`] ruft das
//! `openspider`-Binär auf, Tests setzen einen Fake (Funktionszeiger oder Closure) ein.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use crate::csvio::{read_output, read_reference, Table};
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
    /// Liste statt Spider-Start, absoluter Pfad zur lokalen Fixture.
    pub list_source: Option<PathBuf>,
    /// Konfigurationsvariante (`--set`, gleiche Werte wie im Manifest).
    pub config: BTreeMap<String, String>,
    /// Sitemap-Art (`default` oder `images`).
    pub sitemap: Option<String>,
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
        let mut a = if let Some(list) = &req.list_source {
            vec![
                "--crawl-list".to_string(),
                list.to_string_lossy().into_owned(),
            ]
        } else {
            vec!["--crawl".to_string(), req.base_url.clone()]
        };
        for (key, value) in &req.config {
            a.extend(["--set".into(), format!("{key}={value}")]);
        }
        if let Some(kind) = &req.sitemap {
            a.push(
                if kind == "images" {
                    "--create-images-sitemap"
                } else {
                    "--create-sitemap"
                }
                .into(),
            );
        }
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
    pub ref_file: String,
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
            .position(|e| e.ref_file == r.ref_file)
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
    // OUT-008: historical mappings flatten hyphens to underscores, while the
    // exporter preserves them. Accept this additive spelling variant only when
    // the current directory has exactly one matching file; never guess on a
    // collision or search a different configuration variant.
    let normalized = name.replace('-', "_");
    let mut aliases = std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .is_some_and(|file| file.to_string_lossy().replace('-', "_") == normalized)
        });
    if let Some(alias) = aliases.next() {
        if aliases.next().is_none() {
            return Some(alias);
        }
        return None;
    }
    if depth == 0 {
        return None;
    }
    let mut subdirs: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.is_dir()
                && !p
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("variant-"))
        })
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
            failures.push(format!("{}/{}: {why}", s.site, x.ref_file));
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
    if opts.require_min_sites {
        let m2_present = sites
            .iter()
            .filter(|s| s.present && s.site.starts_with("parity-m2-"))
            .count();
        let legacy_present = sites_present - m2_present;
        // Vor den M2-Referenzexporten bleibt A3 allein aktiv; danach gilt auch B3(d).
        for (group, count, need) in [
            ("M2", m2_present, thresholds.min_m2_sites_present),
            ("M1", legacy_present, thresholds.min_legacy_sites_present),
        ] {
            if count < need && (group != "M2" || m2_present > 0) {
                failures.push(format!(
                    "nur {count} {group}-Sites mit Status present, gefordert sind {need}"
                ));
            }
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
        ref_file: e.ref_file.clone(),
        mapping: e.mapping.clone(),
        os: e.os.clone(),
        status: e.status,
        outcome,
    };
    for e in &manifest.export {
        if let (ExportStatus::Present, Some(why)) = (e.status, &e.skip) {
            results.push(result_of(
                e,
                Outcome::Skipped {
                    reason: why.clone(),
                },
            ));
            continue;
        }
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
                    e.ref_file
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
                    reason: e.skip.clone().unwrap_or_else(|| {
                        "in der Referenz nicht exportierbar (unavailable)".into()
                    }),
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
    // Vollständig frische Ausgabe: auch verschachtelte CLI-Ausgaben früherer Läufe entfernen.
    if os_dir.exists() {
        std::fs::remove_dir_all(&os_dir)?;
    }
    std::fs::create_dir_all(&os_dir)?;
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
    let ref_base = if manifest.port != 0 {
        format!("http://127.0.0.1:{}", manifest.port)
    } else {
        os_base.clone()
    };

    let list_source = manifest
        .list_source
        .as_ref()
        .map(|rel| {
            let path = opts.fixture_dir(site).join(rel);
            std::fs::canonicalize(path).map_err(ParityError::from)
        })
        .transpose()?;
    // Standardausgaben gemeinsam, Konfigurations-/Sitemap-Varianten getrennt: kein Überschreiben.
    let mut requests: Vec<ExportRequest> = Vec::new();
    let mut destinations = Vec::new();
    for p in &prepared {
        let sitemap = match &p.os {
            OsExport::Sitemap(kind) => Some(kind.clone()),
            _ => None,
        };
        let index = requests
            .iter()
            .position(|r| {
                r.config == p.entry.config
                    && r.sitemap == sitemap
                    && match &p.os {
                        OsExport::Tab { .. } => !r.tabs.is_empty(),
                        OsExport::Bulk(_) => !r.bulk.is_empty(),
                        OsExport::Report(_) => !r.reports.is_empty(),
                        OsExport::Sitemap(_) => r.sitemap.is_some(),
                    }
            })
            .unwrap_or_else(|| {
                let index = requests.len();
                requests.push(ExportRequest {
                    site: site.to_string(),
                    base_url: format!("{os_base}/"),
                    list_source: list_source.clone(),
                    config: p.entry.config.clone(),
                    sitemap,
                    profile: profile.map(Path::to_path_buf),
                    out_dir: if index == 0 {
                        os_dir.clone()
                    } else {
                        os_dir.join(format!("variant-{index}"))
                    },
                    tabs: Vec::new(),
                    bulk: Vec::new(),
                    reports: Vec::new(),
                });
                index
            });
        destinations.push(index);
        let req = &mut requests[index];
        let list = match &p.os {
            OsExport::Tab { .. } => Some(&mut req.tabs),
            OsExport::Bulk(_) => Some(&mut req.bulk),
            OsExport::Report(_) => Some(&mut req.reports),
            OsExport::Sitemap(_) => None,
        };
        if let Some(list) = list {
            let value = p.os.cli_value();
            if !list.contains(&value) {
                list.push(value);
            }
        }
    }
    let mut errors: Vec<_> = requests
        .iter()
        .map(|req| exporter.export(req).err())
        .collect();
    // Nur fehlgeschlagene Namensgruppen einzeln wiederholen. Erfolgreiche Gruppen
    // brauchen keinen weiteren Crawl; jeder Fehler bleibt seinem Export zugeordnet.
    for (item, (p, destination)) in prepared.iter().zip(&mut destinations).enumerate() {
        let req = &requests[*destination];
        if errors[*destination].is_none() || req.bulk.len() + req.reports.len() <= 1 {
            continue;
        }
        let mut retry = req.clone();
        match &p.os {
            OsExport::Bulk(_) => retry.bulk = vec![p.os.cli_value()],
            OsExport::Report(_) => retry.reports = vec![p.os.cli_value()],
            _ => continue,
        }
        retry.out_dir = req.out_dir.join(format!("fallback-{item}"));
        *destination = requests.len();
        errors.push(exporter.export(&retry).err());
        requests.push(retry);
    }
    let site_failure = destinations
        .iter()
        .find_map(|i| errors[*i].as_ref())
        .map(|e| format!("OpenSpider-Export fehlgeschlagen: {e}"));
    drop(test_site);

    let ref_bases = BaseUrls::from_base(&ref_base);
    let os_bases = BaseUrls::from_base(&os_base);
    for (p, index) in prepared.iter().zip(destinations) {
        let outcome = if let Some(error) = &errors[index] {
            Err(ParityError::Invalid(format!(
                "OpenSpider-Export fehlgeschlagen: {error}"
            )))
        } else {
            compare_one(
                opts,
                site,
                p,
                &requests[index].out_dir,
                &ref_bases,
                &os_bases,
                thresholds,
                deviations,
            )
        };
        let outcome = match outcome {
            Ok(report) => Outcome::Compared {
                report: Box::new(report),
            },
            Err(e) => Outcome::Error {
                message: e.to_string(),
            },
        };
        if let Outcome::Compared { report } = &outcome {
            let stem = Path::new(&p.entry.ref_file)
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
        error: site_failure,
        exports: in_manifest_order(results, &manifest),
    })
}

#[allow(clippy::too_many_arguments)]
fn compare_one(
    opts: &RunOptions,
    site: &str,
    p: &Prepared,
    os_dir: &Path,
    ref_bases: &BaseUrls,
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
    let ref_path = opts.golden_dir(site).join("ref").join(&p.entry.ref_file);
    let reference = select_rows(
        read_reference(&ref_path, &p.mapping)?,
        &p.entry.select,
        ref_bases,
    )?;
    if !p.entry.select.is_empty() && reference.rows.is_empty() {
        return Err(ParityError::Invalid(
            "Zielauswahl der Referenz hat keine Zeilen".into(),
        ));
    }
    let os = select_rows(
        read_output(&os_path, &p.mapping)?,
        &p.entry.select,
        os_bases,
    )?;
    let devs = deviations.for_file(Some(site), &p.entry.mapping, Some(&p.entry.ref_file));
    let mut report = compare(&CompareInput {
        export: &p.entry.mapping,
        site: Some(site),
        reference: &reference,
        os: &os,
        mapping: &p.mapping,
        ref_bases,
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

/// Angewandte Abweichungen aller Berichte, getrennt nach begründeten Paritätsabweichungen und
/// Referenzeinschränkungen der Free-Version (K-53).
fn summary_deviations(m: &mut String, s: &RunSummary) {
    use std::collections::BTreeMap;
    use std::fmt::Write as _;
    // Kennung → (Spec, Kennzeichen, Begründung, Anzahl, Sites)
    let mut agg: BTreeMap<
        String,
        (
            String,
            String,
            String,
            u64,
            std::collections::BTreeSet<String>,
        ),
    > = BTreeMap::new();
    for site in &s.sites {
        for x in &site.exports {
            if let Outcome::Compared { report } = &x.outcome {
                for d in &report.applied_deviations {
                    let e = agg.entry(d.id.clone()).or_insert_with(|| {
                        (
                            d.spec.clone(),
                            d.kind.clone(),
                            d.note.clone(),
                            0,
                            Default::default(),
                        )
                    });
                    e.3 += d.count;
                    e.4.insert(site.site.clone());
                }
            }
        }
    }
    for (title, free) in [
        ("Begründete Paritätsabweichungen", false),
        ("Referenzeinschränkungen (Free)", true),
    ] {
        let list: Vec<_> = agg
            .iter()
            .filter(|(id, _)| crate::report::is_ref_free(id) == free)
            .collect();
        if list.is_empty() {
            continue;
        }
        let _ = writeln!(m, "## {title}\n");
        let _ = writeln!(
            m,
            "| Kennung | Spec | Kennzeichen | Zellen/Zeilen | Sites | Begründung |\n|---|---|---|---:|---|---|"
        );
        for (id, (spec, kind, note, count, sites)) in list {
            let _ = writeln!(
                m,
                "| {id} | {spec} | {kind} | {count} | {} | {} |",
                sites.iter().cloned().collect::<Vec<_>>().join(", "),
                note.replace('|', "/")
            );
        }
        m.push('\n');
    }
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
                x.ref_file,
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
    summary_deviations(&mut m, s);
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

/// Zielauswahl wird auf beiden Seiten vor dem Vergleich angewandt; ein fehlendes Ziel ist ein Fehler.
fn select_rows(
    mut table: Table,
    select: &BTreeMap<String, String>,
    bases: &BaseUrls,
) -> Result<Table, ParityError> {
    for (column, value) in select {
        let index = table
            .index_of(column)
            .ok_or_else(|| ParityError::Invalid(format!("Zielspalte {column} fehlt")))?;
        let rules = crate::normalize::CellRules {
            base_url: true,
            ..Default::default()
        };
        let expected = if value.starts_with('/') {
            format!("{{BASE}}{value}")
        } else {
            rules.prepare(value, bases)
        };
        table
            .rows
            .retain(|r| rules.prepare(&r[index], bases) == expected);
    }
    Ok(table)
}

#[cfg(test)]
mod filename_format_tests {
    use super::*;

    // spec: OUT-008
    #[test]
    fn hyphen_alias_is_unique_and_stays_in_its_configuration_variant() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("security_unsafe_cross-origin_links.csv");
        std::fs::write(&file, "Address\n").unwrap();
        assert_eq!(
            find_file(dir.path(), "security_unsafe_cross_origin_links.csv", 3),
            Some(file)
        );
        let variant = dir.path().join("variant-1");
        std::fs::create_dir(&variant).unwrap();
        std::fs::write(variant.join("only_here.csv"), "Address\n").unwrap();
        assert_eq!(find_file(dir.path(), "only_here.csv", 3), None);
        std::fs::write(
            dir.path().join("security-unsafe_cross_origin_links.csv"),
            "Address\n",
        )
        .unwrap();
        assert_eq!(
            find_file(dir.path(), "security_unsafe_cross_origin_links.csv", 3),
            None
        );
    }
}
