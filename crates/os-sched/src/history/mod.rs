//! Laufhistorie je Aufgabe (OUT-169, OUT-167, OUT-171).
//!
//! Jeder Lauf ist eine eigene Datei `schedules/history/<task-id>/<run-id>.json`; die Datei wird atomar
//! überschrieben (Beginn, Herzschlag, Ende). So können beliebig viele Prozesse (OS-Planer, App) gleichzeitig
//! schreiben, ohne sich zu sperren. Ein laufender Lauf erneuert seinen Herzschlag alle paar Sekunden;
//! [`HistoryStore::reconcile`] markiert Läufe ohne frischen Herzschlag als `Failed` („unexpected
//! termination“). Je Aufgabe bleiben die letzten 100 Läufe erhalten.
//!
//! Abbruch: Wer einen Lauf abbrechen will, legt die Marke `<run-id>.cancel` neben die Laufdatei
//! ([`HistoryStore::request_cancel`]); der Lauf prüft sie regelmäßig.

use std::path::{Path, PathBuf};

use os_model::Timestamp;
use serde::{Deserialize, Serialize};

/// Höchstzahl der aufbewahrten Läufe je Aufgabe.
pub const KEEP_RUNS: usize = 100;
/// Ein Lauf gilt als abgestürzt, wenn sein Herzschlag älter ist.
pub const STALE_AFTER_SECS: i64 = 60;
/// Abstand der Herzschläge.
pub const HEARTBEAT_SECS: u64 = 5;

/// Ergebnis eines Laufs (OUT-169).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunResult {
    /// Läuft noch.
    Running,
    /// Abgeschlossen.
    Completed,
    /// Abgeschlossen mit Warnungen.
    CompletedWithWarnings,
    /// Fehlgeschlagen.
    Failed,
    /// Übersprungen.
    Skipped,
    /// Abgebrochen.
    Aborted,
}

impl RunResult {
    /// Anzeigetext der Spec (englisch, kanonisch).
    pub fn label(self) -> &'static str {
        match self {
            RunResult::Running => "Running",
            RunResult::Completed => "Completed",
            RunResult::CompletedWithWarnings => "Completed with warnings",
            RunResult::Failed => "Failed",
            RunResult::Skipped => "Skipped",
            RunResult::Aborted => "Aborted",
        }
    }
}

/// Ein Lauf einer Aufgabe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRecord {
    /// Kennung des Laufs.
    pub run_id: String,
    /// Aufgabe.
    pub task_id: String,
    /// Name der Aufgabe zum Zeitpunkt des Laufs.
    pub task_name: String,
    /// Beginn (UTC, Datenbankform).
    pub started_at: String,
    /// Ende.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    /// Letzter Herzschlag.
    pub heartbeat_at: String,
    /// Prozesskennung.
    pub pid: u32,
    /// Ergebnis.
    pub result: RunResult,
    /// Exit-Code des Prozesses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// Entstandener Crawl.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crawl_id: Option<String>,
    /// Anzahl gecrawlter URLs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub urls_crawled: Option<u64>,
    /// Tatsächlicher Ausgabeordner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_folder: Option<String>,
    /// Erzeugte Dateien.
    #[serde(default)]
    pub exports: Vec<String>,
    /// Warnungen und übersprungene Positionen.
    #[serde(default)]
    pub warnings: Vec<String>,
    /// Meldung (Fehlergrund, „Skipped: previous run still active“ …), redigiert.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Lauf-Logdatei.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_path: Option<String>,
    /// Manuell gestartet („Jetzt ausführen“) statt vom Planer.
    #[serde(default)]
    pub manual: bool,
}

impl RunRecord {
    /// Neuer laufender Lauf.
    pub fn start(run_id: &str, task_id: &str, task_name: &str, now: Timestamp) -> RunRecord {
        RunRecord {
            run_id: run_id.to_string(),
            task_id: task_id.to_string(),
            task_name: task_name.to_string(),
            started_at: now.to_db(),
            ended_at: None,
            heartbeat_at: now.to_db(),
            pid: std::process::id(),
            result: RunResult::Running,
            exit_code: None,
            crawl_id: None,
            urls_crawled: None,
            output_folder: None,
            exports: Vec::new(),
            warnings: Vec::new(),
            message: None,
            log_path: None,
            manual: false,
        }
    }

    /// Beendet den Lauf.
    pub fn finish(&mut self, result: RunResult, exit_code: Option<i32>, now: Timestamp) {
        self.result = result;
        self.exit_code = exit_code;
        self.ended_at = Some(now.to_db());
        self.heartbeat_at = now.to_db();
    }

    /// Läuft der Lauf noch (Zustand `Running` und frischer Herzschlag)?
    pub fn is_alive(&self, now: Timestamp) -> bool {
        self.result == RunResult::Running
            && Timestamp::from_db(&self.heartbeat_at)
                .map(|t| (now.0 - t.0).whole_seconds() <= STALE_AFTER_SECS)
                .unwrap_or(false)
    }
}

/// Ist `id` als Datei-/Ordnername zulässig?
fn safe(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Ablage der Laufdateien.
#[derive(Debug, Clone)]
pub struct HistoryStore {
    dir: PathBuf,
}

impl HistoryStore {
    /// Ablage in `dir`.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        HistoryStore { dir: dir.into() }
    }

    /// Ablage `<settings>/schedules/history`.
    pub fn in_settings(settings: &Path) -> Self {
        Self::new(settings.join("schedules").join("history"))
    }

    /// Ordner.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn task_dir(&self, task_id: &str) -> std::io::Result<PathBuf> {
        if !safe(task_id) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "ungültige Aufgabenkennung",
            ));
        }
        Ok(self.dir.join(task_id))
    }

    fn run_path(&self, task_id: &str, run_id: &str) -> std::io::Result<PathBuf> {
        if !safe(run_id) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "ungültige Laufkennung",
            ));
        }
        Ok(self.task_dir(task_id)?.join(format!("{run_id}.json")))
    }

    /// Schreibt (oder ersetzt) einen Lauf atomar.
    pub fn write(&self, record: &RunRecord) -> std::io::Result<()> {
        let path = self.run_path(&record.task_id, &record.run_id)?;
        let dir = path.parent().expect("Laufdatei hat einen Ordner");
        std::fs::create_dir_all(dir)?;
        let tmp = dir.join(format!(".{}.{}.tmp", record.run_id, std::process::id()));
        let bytes = serde_json::to_vec_pretty(record).map_err(std::io::Error::other)?;
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(&tmp, &path).inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })
    }

    /// Liest einen Lauf.
    pub fn get(&self, task_id: &str, run_id: &str) -> Option<RunRecord> {
        let path = self.run_path(task_id, run_id).ok()?;
        serde_json::from_slice(&std::fs::read(path).ok()?).ok()
    }

    /// Sucht einen Lauf über alle Aufgaben.
    pub fn find(&self, run_id: &str) -> Option<RunRecord> {
        if !safe(run_id) {
            return None;
        }
        for entry in std::fs::read_dir(&self.dir).ok()?.flatten() {
            let candidate = entry.path().join(format!("{run_id}.json"));
            if let Some(record) = std::fs::read(&candidate)
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
            {
                return Some(record);
            }
        }
        None
    }

    fn read_task(&self, task_id: &str) -> Vec<RunRecord> {
        let Ok(dir) = self.task_dir(task_id) else {
            return Vec::new();
        };
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        entries
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
            .filter_map(|e| std::fs::read(e.path()).ok())
            .filter_map(|b| serde_json::from_slice::<RunRecord>(&b).ok())
            .collect()
    }

    fn task_ids(&self) -> Vec<String> {
        std::fs::read_dir(&self.dir)
            .map(|d| {
                d.flatten()
                    .filter(|e| e.path().is_dir())
                    .filter_map(|e| e.file_name().into_string().ok())
                    .filter(|n| safe(n))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Läufe, neueste zuerst; `task_id = None` über alle Aufgaben.
    pub fn list(&self, task_id: Option<&str>, offset: usize, limit: usize) -> Vec<RunRecord> {
        let mut all: Vec<RunRecord> = match task_id {
            Some(id) => self.read_task(id),
            None => self
                .task_ids()
                .iter()
                .flat_map(|id| self.read_task(id))
                .collect(),
        };
        all.sort_by(|a, b| {
            b.started_at
                .cmp(&a.started_at)
                .then_with(|| b.run_id.cmp(&a.run_id))
        });
        all.into_iter().skip(offset).take(limit).collect()
    }

    /// Der jüngste Lauf einer Aufgabe.
    pub fn last(&self, task_id: &str) -> Option<RunRecord> {
        self.list(Some(task_id), 0, 1).into_iter().next()
    }

    /// Laufende Läufe einer Aufgabe mit frischem Herzschlag (Überlappungsprüfung, OUT-167).
    pub fn active(&self, task_id: &str, now: Timestamp) -> Vec<RunRecord> {
        self.read_task(task_id)
            .into_iter()
            .filter(|r| r.is_alive(now))
            .collect()
    }

    /// Markiert Läufe, die ohne Abschluss geendet haben, als `Failed` (OUT-169). Liefert die korrigierten
    /// Läufe.
    pub fn reconcile(&self, now: Timestamp) -> Vec<RunRecord> {
        let mut fixed = Vec::new();
        for id in self.task_ids() {
            for mut record in self.read_task(&id) {
                if record.result == RunResult::Running && !record.is_alive(now) {
                    record.message = Some("unexpected termination".to_string());
                    let at = Timestamp::from_db(&record.heartbeat_at).unwrap_or(now);
                    record.finish(RunResult::Failed, None, at);
                    if self.write(&record).is_ok() {
                        fixed.push(record);
                    }
                }
            }
        }
        fixed
    }

    /// Verwirft die ältesten Läufe über `keep` hinaus; laufende bleiben.
    pub fn prune(&self, task_id: &str, keep: usize) -> usize {
        let mut finished: Vec<RunRecord> = self
            .read_task(task_id)
            .into_iter()
            .filter(|r| r.result != RunResult::Running)
            .collect();
        finished.sort_by(|a, b| {
            b.started_at
                .cmp(&a.started_at)
                .then_with(|| b.run_id.cmp(&a.run_id))
        });
        let mut removed = 0;
        for old in finished.into_iter().skip(keep) {
            if let Ok(path) = self.run_path(&old.task_id, &old.run_id) {
                if std::fs::remove_file(path).is_ok() {
                    removed += 1;
                }
                let _ = std::fs::remove_file(self.cancel_path(&old.task_id, &old.run_id));
            }
        }
        removed
    }

    /// Entfernt die gesamte Historie einer Aufgabe.
    pub fn delete_task(&self, task_id: &str) {
        if let Ok(dir) = self.task_dir(task_id) {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    fn cancel_path(&self, task_id: &str, run_id: &str) -> PathBuf {
        self.dir.join(task_id).join(format!("{run_id}.cancel"))
    }

    /// Bittet einen laufenden Lauf, sich zu beenden (OUT-171).
    pub fn request_cancel(&self, task_id: &str, run_id: &str) -> std::io::Result<()> {
        let path = self.run_path(task_id, run_id)?.with_extension("cancel");
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, b"cancel")
    }

    /// Wurde der Abbruch angefordert?
    pub fn cancel_requested(&self, task_id: &str, run_id: &str) -> bool {
        self.run_path(task_id, run_id)
            .map(|p| p.with_extension("cancel").exists())
            .unwrap_or(false)
    }

    /// Entfernt die Abbruchmarke.
    pub fn clear_cancel(&self, task_id: &str, run_id: &str) {
        if let Ok(p) = self.run_path(task_id, run_id) {
            let _ = std::fs::remove_file(p.with_extension("cancel"));
        }
    }
}
