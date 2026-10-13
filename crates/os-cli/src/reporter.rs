//! Ausgaben nach stderr: Hinweise, Warnungen, Fortschritt (OUT-205).
//!
//! stdout bleibt Nutzdaten vorbehalten (`--list-crawls`, `--help`); alles andere geht nach stderr. Es gibt
//! keine ANSI-Sequenzen und keine Fortschrittsbalken. Mit `--json-progress` erscheint je Phase und höchstens
//! einmal je Sekunde eine JSON-Zeile. Warnungen werden gezählt (für `--strict`).

use std::io::Write;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use os_model::Progress;
use serde_json::json;

/// Gemeinsame Ausgabe des Laufs.
#[derive(Debug)]
pub struct Reporter {
    warnings: AtomicU32,
    json: bool,
}

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
        })
    }

    /// Mit `--json-progress`?
    pub fn json(&self) -> bool {
        self.json
    }

    /// Hinweis ohne Warnwirkung.
    pub fn info(&self, msg: impl AsRef<str>) {
        write_stderr(msg.as_ref());
    }

    /// Warnung; zählt für `--strict`.
    pub fn warn(&self, msg: impl AsRef<str>) {
        self.warnings.fetch_add(1, Ordering::SeqCst);
        write_stderr(&format!("Warnung: {}", msg.as_ref()));
    }

    /// Fehlermeldung (Export fehlgeschlagen o. Ä.); zählt nicht als Warnung.
    pub fn error(&self, msg: impl AsRef<str>) {
        write_stderr(&format!("Fehler: {}", msg.as_ref()));
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
