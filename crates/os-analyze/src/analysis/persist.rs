//! Persistenz der Teilschritte in `analysis_steps` und `crawl_meta` (CRAWL-141, Punkt 5 und 6).
//!
//! `analysis_steps`: eine Zeile je Teilschritt (`state` = `StepState::as_str()`, `finished_at` als
//! `Timestamp::to_db`, `config_hash` = Hash der für den Schritt relevanten Konfiguration).
//! `crawl_meta`:
//! - `analysis_state`: `not_started` (Schlüssel fehlt) | `running` | `completed` | `incomplete` | `stale`
//! - `analysis_at`: Zeitpunkt des letzten abgeschlossenen oder abgebrochenen Laufs
//! - `analysis_stale`: `0` oder eine mit Komma getrennte Liste der veralteten Teilschritte (`1` = alle)

use std::collections::{BTreeMap, BTreeSet};

use os_model::{AnalysisStep, StepState, Timestamp};
use os_store::CrawlFile;
use rusqlite::params;

use crate::error::AnalyzeError;

/// Gespeicherter Zustand eines Teilschritts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepRecord {
    /// Zustand.
    pub state: StepState,
    /// Zeitpunkt des Abschlusses (`Timestamp::to_db`).
    pub finished_at: Option<String>,
    /// Hash der relevanten Konfiguration zum Zeitpunkt des Laufs.
    pub config_hash: Option<String>,
}

/// Zustand der Analyse insgesamt (`crawl_meta.analysis_state`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    /// Noch nie gelaufen.
    NotStarted,
    /// Läuft (oder ist abgestürzt, bevor sie ihren Zustand schreiben konnte).
    Running,
    /// Alle Teilschritte sind entschieden.
    Completed,
    /// Abgebrochen oder Teilschritte stehen noch aus.
    Incomplete,
    /// Nach „Neu crawlen“, „Entfernen“ oder Konfigurationsänderung veraltet.
    Stale,
}

impl RunState {
    /// Wert in `crawl_meta`.
    pub fn as_str(&self) -> &'static str {
        match self {
            RunState::NotStarted => "not_started",
            RunState::Running => "running",
            RunState::Completed => "completed",
            RunState::Incomplete => "incomplete",
            RunState::Stale => "stale",
        }
    }

    fn parse(s: &str) -> RunState {
        match s {
            "running" => RunState::Running,
            "completed" => RunState::Completed,
            "incomplete" => RunState::Incomplete,
            "stale" => RunState::Stale,
            _ => RunState::NotStarted,
        }
    }
}

/// Menge der als veraltet markierten Teilschritte.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StaleSet {
    all: bool,
    steps: BTreeSet<AnalysisStep>,
}

impl StaleSet {
    /// Ist der Teilschritt veraltet?
    pub fn contains(&self, step: AnalysisStep) -> bool {
        self.all || self.steps.contains(&step)
    }

    /// Ist nichts veraltet?
    pub fn is_empty(&self) -> bool {
        !self.all && self.steps.is_empty()
    }

    fn parse(s: &str) -> StaleSet {
        let mut set = StaleSet::default();
        for part in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            match part {
                "0" => {}
                "1" | "all" => set.all = true,
                name => {
                    if let Ok(step) = name.parse::<AnalysisStep>() {
                        set.steps.insert(step);
                    }
                }
            }
        }
        set
    }

    fn render(&self) -> String {
        if self.all {
            return "1".to_string();
        }
        if self.steps.is_empty() {
            return "0".to_string();
        }
        self.steps
            .iter()
            .map(AnalysisStep::as_str)
            .collect::<Vec<_>>()
            .join(",")
    }
}

/// Liest alle gespeicherten Teilschritt-Zeilen (unbekannte Schritte oder Zustände werden ignoriert).
pub fn read_steps(file: &CrawlFile) -> Result<BTreeMap<AnalysisStep, StepRecord>, AnalyzeError> {
    let mut stmt = file
        .conn()
        .prepare("SELECT step, state, finished_at, config_hash FROM analysis_steps")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, Option<String>>(3)?,
        ))
    })?;
    let mut out = BTreeMap::new();
    for row in rows {
        let (step, state, finished_at, config_hash) = row?;
        if let (Ok(step), Ok(state)) = (step.parse::<AnalysisStep>(), state.parse::<StepState>()) {
            out.insert(
                step,
                StepRecord {
                    state,
                    finished_at,
                    config_hash,
                },
            );
        }
    }
    Ok(out)
}

/// Schreibt (ersetzt) die Zeile eines Teilschritts.
pub fn write_step(
    file: &CrawlFile,
    step: AnalysisStep,
    state: StepState,
    finished_at: Option<&Timestamp>,
    config_hash: Option<&str>,
) -> Result<(), AnalyzeError> {
    file.conn().execute(
        "INSERT INTO analysis_steps (step, state, finished_at, config_hash) VALUES (?1, ?2, ?3, ?4) \
         ON CONFLICT(step) DO UPDATE SET state = excluded.state, finished_at = excluded.finished_at, \
         config_hash = excluded.config_hash",
        params![
            step.as_str(),
            state.as_str(),
            finished_at.map(Timestamp::to_db),
            config_hash
        ],
    )?;
    Ok(())
}

/// Setzt einen Teilschritt auf „ausstehend“ (vor dem Lauf: ein abgebrochener Schritt zeigt nie Altwerte
/// als gültig).
pub fn begin_step(file: &CrawlFile, step: AnalysisStep) -> Result<(), AnalyzeError> {
    write_step(file, step, StepState::Pending, None, None)
}

/// Liest `crawl_meta.analysis_state`.
pub fn read_run_state(file: &CrawlFile) -> Result<RunState, AnalyzeError> {
    Ok(file
        .get_meta("analysis_state")?
        .map_or(RunState::NotStarted, |v| RunState::parse(&v)))
}

/// Schreibt `analysis_state` (und, wenn angegeben, `analysis_at`).
pub fn write_run_state(
    file: &CrawlFile,
    state: RunState,
    at: Option<&Timestamp>,
) -> Result<(), AnalyzeError> {
    file.set_meta("analysis_state", state.as_str())?;
    if let Some(at) = at {
        file.set_meta("analysis_at", &at.to_db())?;
    }
    Ok(())
}

/// Liest die Menge der veralteten Teilschritte.
pub fn read_stale(file: &CrawlFile) -> Result<StaleSet, AnalyzeError> {
    Ok(file
        .get_meta("analysis_stale")?
        .map_or_else(StaleSet::default, |v| StaleSet::parse(&v)))
}

fn write_stale(file: &CrawlFile, set: &StaleSet) -> Result<(), AnalyzeError> {
    file.set_meta("analysis_stale", &set.render())?;
    Ok(())
}

/// Nimmt einen Teilschritt aus der Menge der veralteten heraus (nach erfolgreichem Lauf).
pub fn clear_stale(file: &CrawlFile, step: AnalysisStep) -> Result<(), AnalyzeError> {
    let mut set = read_stale(file)?;
    if set.all {
        set.all = false;
        set.steps = AnalysisStep::ALL.iter().copied().collect();
    }
    if set.steps.remove(&step) {
        write_stale(file, &set)?;
    }
    Ok(())
}

/// Markiert Teilschritte als veraltet (z. B. nach „Neu crawlen“, „Entfernen“, „URLs hinzufügen“). Die
/// Analyse wird nicht automatisch neu gestartet (CRAWL-141, Punkt 6). War sie abgeschlossen oder
/// unvollständig, wechselt `analysis_state` auf `stale`.
pub fn mark_stale(file: &CrawlFile, steps: &[AnalysisStep]) -> Result<(), AnalyzeError> {
    if steps.is_empty() {
        return Ok(());
    }
    let mut set = read_stale(file)?;
    if !set.all {
        set.steps.extend(steps.iter().copied());
        if AnalysisStep::ALL.iter().all(|s| set.steps.contains(s)) {
            set.all = true;
            set.steps.clear();
        }
    }
    write_stale(file, &set)?;
    if matches!(
        read_run_state(file)?,
        RunState::Completed | RunState::Incomplete
    ) {
        write_run_state(file, RunState::Stale, None)?;
    }
    Ok(())
}
