//! Ausgaben nach stderr: Hinweise, Warnungen, Fortschritt (OUT-205).
//!
//! stdout bleibt Nutzdaten vorbehalten (`--list-crawls`, `--help`); alles andere geht nach stderr. Es gibt
//! keine ANSI-Sequenzen und keine Fortschrittsbalken. Mit `--json-progress` erscheint je Phase und höchstens
//! einmal je Sekunde eine JSON-Zeile. Warnungen werden gezählt (für `--strict`).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use os_model::Progress;
use serde_json::json;

/// Gemeinsame Ausgabe des Laufs.
#[derive(Debug)]
pub struct Reporter {
    warnings: AtomicU32,
    json: bool,
    /// Meldungen zusätzlich als `tracing`-Ereignisse ins Lauf-Log schreiben (geplante Läufe, OUT-240).
    log_events: AtomicBool,
    /// Text der Warnungen (für die Laufhistorie, OUT-169); höchstens [`MAX_KEPT`].
    kept_warnings: Mutex<Vec<String>>,
    /// Geschriebene Exportdateien (für die Laufhistorie).
    exported: Mutex<Vec<PathBuf>>,
}

/// Höchstzahl gemerkter Warnungen.
const MAX_KEPT: usize = 200;

fn write_stderr(text: &str) {
    let mut err = std::io::stderr().lock();
    // Ein geschlossenes stderr darf den Lauf nicht beenden.
    let _ = writeln!(err, "{text}");
}

impl Reporter {
    /// Neuer Berichterstatter; `json` = `--json-progress`.
    pub fn new(json: bool) -> Arc<Self> {
        Arc::new(Reporter {
            warnings: AtomicU32::new(0),
            json,
            log_events: AtomicBool::new(false),
            kept_warnings: Mutex::new(Vec::new()),
            exported: Mutex::new(Vec::new()),
        })
    }

    /// Wie [`Reporter::new`], schreibt Hinweise, Warnungen und Fehler zusätzlich ins Lauf-Log.
    pub fn for_task(json: bool) -> Arc<Self> {
        let rep = Reporter::new(json);
        rep.log_events.store(true, Ordering::SeqCst);
        rep
    }

    /// Merkt eine geschriebene Exportdatei.
    pub fn note_export(&self, path: &Path) {
        self.exported
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(path.to_path_buf());
    }

    /// Die geschriebenen Exportdateien in Reihenfolge.
    pub fn exported_files(&self) -> Vec<PathBuf> {
        self.exported
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// Der Text der bisherigen Warnungen (ohne das Präfix „Warnung:“).
    pub fn warning_messages(&self) -> Vec<String> {
        self.kept_warnings
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// Mit `--json-progress`?
    pub fn json(&self) -> bool {
        self.json
    }

    /// Hinweis ohne Warnwirkung.
    pub fn info(&self, msg: impl AsRef<str>) {
        write_stderr(msg.as_ref());
        if self.log_events.load(Ordering::SeqCst) {
            tracing::info!(target: "os_cli::task", "{}", msg.as_ref());
        }
    }

    /// Warnung; zählt für `--strict`.
    pub fn warn(&self, msg: impl AsRef<str>) {
        self.warnings.fetch_add(1, Ordering::SeqCst);
        write_stderr(&format!("Warnung: {}", msg.as_ref()));
        {
            let mut kept = self.kept_warnings.lock().unwrap_or_else(|p| p.into_inner());
            if kept.len() < MAX_KEPT {
                kept.push(msg.as_ref().to_string());
            }
        }
        if self.log_events.load(Ordering::SeqCst) {
            tracing::warn!(target: "os_cli::task", "{}", msg.as_ref());
        }
    }

    /// Fehlermeldung (Export fehlgeschlagen o. Ä.); zählt nicht als Warnung.
    pub fn error(&self, msg: impl AsRef<str>) {
        write_stderr(&format!("Fehler: {}", msg.as_ref()));
        if self.log_events.load(Ordering::SeqCst) {
            tracing::error!(target: "os_cli::task", "{}", msg.as_ref());
        }
    }

    /// Anzahl der Warnungen bisher.
    pub fn warnings(&self) -> u32 {
        self.warnings.load(Ordering::SeqCst)
    }

    /// Beginn einer Phase (`validate`, `crawl`, `analysis`, `export`, `notify`, `done`), nur mit
    /// `--json-progress`.
    pub fn phase(&self, name: &str) {
        if self.json {
            write_stderr(&json!({ "phase": name }).to_string());
        }
    }

    /// Fortschrittszeile des Crawls: JSON mit `--json-progress`, sonst eine Textzeile.
    pub fn progress(&self, phase: &str, p: &Progress, errors: Option<u64>) {
        if self.json {
            let line = json!({
                "phase": phase,
                "crawled": p.crawled,
                "queued": p.queued,
                "errors": errors,
                "elapsed_s": p.elapsed_s,
                "eta_s": p.eta_s,
                "percent": p.percent,
                "rate": (p.rate * 10.0).round() / 10.0,
                "state": p.state.as_str(),
            });
            write_stderr(&line.to_string());
        } else {
            write_stderr(&format!(
                "Crawl: {} gecrawlt, {} in der Warteschlange ({:.1} URLs/s, {} s)",
                p.crawled, p.queued, p.rate, p.elapsed_s
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warnings_are_counted() {
        let rep = Reporter::new(false);
        assert_eq!(rep.warnings(), 0);
        rep.warn("a");
        rep.warn("b");
        rep.info("c");
        rep.error("d");
        assert_eq!(rep.warnings(), 2);
    }
}
