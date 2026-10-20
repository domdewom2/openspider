//! Anbindung an den Planer des Betriebssystems (OUT-166): macOS-LaunchAgent und Windows-Aufgabenplanung.
//!
//! Jede Aufgabe wird beim Betriebssystem so eingetragen, dass es `openspider --run-task <task-id>` zur
//! geplanten Zeit aufruft – auch bei geschlossener App. Die Backends erzeugen die Beschreibung (plist bzw.
//! Task-XML) als reine Funktion ([`launchd::plist`], [`windows::task_xml`]); der Zugriff auf das System
//! läuft über das Merkmal [`CommandRunner`]. Mit [`NoopRunner`] und einem Ordner für die Dateien ist jedes
//! Backend zugleich der Dry-Run für Tests; echte Registrierung gibt es nur im Betrieb ([`system`]).

pub mod launchd;
pub mod windows;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::tasks::Schedule;

/// Ausgabe eines Programmaufrufs.
#[derive(Debug, Clone, Default)]
pub struct CommandOutput {
    /// Exit-Code (`-1` = beendet durch Signal).
    pub status: i32,
    /// Standardausgabe.
    pub stdout: String,
    /// Fehlerausgabe.
    pub stderr: String,
}

/// Führt Programme des Betriebssystems aus (`launchctl`, `schtasks`). Nie über eine Shell.
pub trait CommandRunner: Send + Sync {
    /// Ruft `program` mit `args` auf.
    fn run(&self, program: &str, args: &[String]) -> std::io::Result<CommandOutput>;
}

/// Echte Aufrufe über `std::process::Command` (ohne Shell).
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&self, program: &str, args: &[String]) -> std::io::Result<CommandOutput> {
        let out = std::process::Command::new(program)
            .args(args)
            .stdin(std::process::Stdio::null())
            .output()?;
        Ok(CommandOutput {
            status: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }
}

/// Führt nichts aus und meldet Erfolg (Dry-Run).
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopRunner;

impl CommandRunner for NoopRunner {
    fn run(&self, _program: &str, _args: &[String]) -> std::io::Result<CommandOutput> {
        Ok(CommandOutput::default())
    }
}

/// Backend des Planers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// launchd (macOS).
    Launchd,
    /// Windows-Aufgabenplanung.
    TaskScheduler,
}

/// Was beim Planer eingetragen werden soll.
#[derive(Debug, Clone)]
pub struct PlannerSpec {
    /// Kennung der Aufgabe.
    pub task_id: String,
    /// Anzeigename.
    pub task_name: String,
    /// Zeitplan.
    pub schedule: Schedule,
    /// Pfad des `openspider`-Programms.
    pub binary: PathBuf,
    /// Abweichender Datenordner (`OPENSPIDER_HOME`), wird dem Aufruf mitgegeben.
    pub home: Option<PathBuf>,
    /// Verpasste Läufe nachholen (Standard an).
    pub run_if_missed: bool,
    /// Auch im Akkubetrieb ausführen (Standard an).
    pub on_battery: bool,
    /// [P2] Rechner zum Ausführen aufwecken (Windows).
    pub wake: bool,
}

/// Eine Datei, die ein Backend schreibt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannerFile {
    /// Pfad.
    pub path: PathBuf,
    /// Inhalt.
    pub content: String,
}

/// Vorschau (und Ergebnis) einer Registrierung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    /// Backend.
    pub backend: Backend,
    /// Dateien, die geschrieben werden.
    pub files: Vec<PlannerFile>,
    /// Aufruf, der registriert (Programm und Argumente).
    pub command: Vec<String>,
}

/// Fehler des Planers.
#[derive(Debug, thiserror::Error)]
pub enum PlannerError {
    /// Der Zeitplan lässt sich beim Backend nicht ausdrücken.
    #[error("Zeitplan wird vom Planer nicht unterstützt: {0}")]
    Unsupported(String),
    /// Ungültige Eingabe.
    #[error("{0}")]
    Invalid(String),
    /// Dateifehler.
    #[error("{path}: {source}")]
    Io {
        /// Pfad.
        path: PathBuf,
        /// Ursache.
        source: std::io::Error,
    },
    /// Das Systemprogramm schlug fehl.
    #[error("{program} endete mit Code {status}: {stderr}")]
    Command {
        /// Programm.
        program: String,
        /// Exit-Code.
        status: i32,
        /// Meldung.
        stderr: String,
    },
}

/// Der Planer des Betriebssystems.
pub trait OsPlanner: Send + Sync {
    /// Backend.
    fn backend(&self) -> Backend;
    /// Zeigt, was geschrieben würde, ohne etwas zu verändern (`dryRun`).
    fn preview(&self, spec: &PlannerSpec) -> Result<Preview, PlannerError>;
    /// Trägt die Aufgabe ein (ersetzt einen vorhandenen Eintrag).
    fn install(&self, spec: &PlannerSpec) -> Result<Preview, PlannerError>;
    /// Trägt die Aufgabe aus; ein fehlender Eintrag ist kein Fehler.
    fn uninstall(&self, task_id: &str) -> Result<(), PlannerError>;
    /// Ist die Aufgabe eingetragen?
    fn is_registered(&self, task_id: &str) -> bool;
    /// Kennungen aller eingetragenen OpenSpider-Aufgaben.
    fn registered_ids(&self) -> Vec<String>;
    /// Programm, auf das der Eintrag zeigt (für die Neuregistrierung nach einem Update).
    fn registered_binary(&self, task_id: &str) -> Option<PathBuf>;
}

/// Planer für das laufende Betriebssystem; `None` auf anderen Systemen.
pub fn system(runner: Arc<dyn CommandRunner>, xml_dir: &Path) -> Option<Arc<dyn OsPlanner>> {
    #[cfg(target_os = "macos")]
    {
        let _ = xml_dir;
        let dir = dirs::home_dir()?.join("Library").join("LaunchAgents");
        Some(Arc::new(launchd::Launchd::new(dir, runner)))
    }
    #[cfg(target_os = "windows")]
    {
        Some(Arc::new(windows::TaskScheduler::new(
            xml_dir.to_path_buf(),
            runner,
        )))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (runner, xml_dir);
        None
    }
}

/// Dry-Run-Planer: erzeugt die Dateien des gewünschten Backends in `dir`, ruft aber nichts auf.
pub fn dry_run(backend: Backend, dir: &Path) -> Arc<dyn OsPlanner> {
    match backend {
        Backend::Launchd => Arc::new(launchd::Launchd::new(
            dir.to_path_buf(),
            Arc::new(NoopRunner),
        )),
        Backend::TaskScheduler => Arc::new(windows::TaskScheduler::new(
            dir.to_path_buf(),
            Arc::new(NoopRunner),
        )),
    }
}

/// Trägt alle eingetragenen OpenSpider-Aufgaben aus (Deinstallation, OUT-166); Anzahl der entfernten Einträge.
pub fn uninstall_all(planner: &dyn OsPlanner) -> usize {
    planner
        .registered_ids()
        .iter()
        .filter(|id| planner.uninstall(id).is_ok())
        .count()
}

/// Das Backend dieses Betriebssystems (für Dry-Run-Anzeigen auf anderen Systemen: launchd).
pub fn native_backend() -> Backend {
    if cfg!(target_os = "windows") {
        Backend::TaskScheduler
    } else {
        Backend::Launchd
    }
}

/// Pfad des `openspider`-Programms: `OPENSPIDER_CLI`, sonst neben dem laufenden Programm.
pub fn cli_binary() -> PathBuf {
    if let Some(path) = std::env::var_os("OPENSPIDER_CLI").filter(|p| !p.is_empty()) {
        return PathBuf::from(path);
    }
    let name = if cfg!(windows) {
        "openspider.exe"
    } else {
        "openspider"
    };
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(name)))
        .unwrap_or_else(|| PathBuf::from(name))
}

pub(crate) fn write_file(path: &Path, bytes: &[u8]) -> Result<(), PlannerError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|source| PlannerError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
    }
    std::fs::write(path, bytes).map_err(|source| PlannerError::Io {
        path: path.to_path_buf(),
        source,
    })
}

pub(crate) fn xml_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if c.is_control() && c != '\n' && c != '\t' => {}
            c => out.push(c),
        }
    }
    out
}
