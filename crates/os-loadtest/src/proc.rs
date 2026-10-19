//! Kindprozesse der Lasttests: Start, Zeilenprotokoll (`READY`/`PROGRESS`/`DONE`), hartes Beenden, Messung.
//!
//! Beide Kinder (`os-loadtest crawl-child` und `examples/crawl_child.rs` von os-engine) sprechen dasselbe
//! Protokoll, siehe [`crate::child`].

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use crate::rss::wait_with_peak_rss;

/// Ergebnis eines bis zum Ende beobachteten Kindes.
#[derive(Debug, Clone, PartialEq)]
pub struct RunResult {
    /// `crawled` der `DONE`-Zeile.
    pub crawled: u64,
    /// `state` der `DONE`-Zeile (`Completed`, `Paused` …).
    pub state: String,
    /// `end_reason` der `DONE`-Zeile (`-` = keiner).
    pub end_reason: String,
    /// Wanduhrzeit von Start bis Exit in Sekunden.
    pub seconds: f64,
    /// Spitzen-RSS des Kindes in Bytes.
    pub peak_rss_bytes: Option<u64>,
    /// Längste Zeit ohne Fortschritt (`PROGRESS` mit größerem `crawled`) in Sekunden.
    pub max_stall_s: f64,
    /// Messzähler des Kindes aus den `DIAG`-Zeilen (`<gruppe>_<name>` → Wert; M1-P30).
    pub diag: BTreeMap<String, u64>,
    /// Stützstellen des Fortschritts für PERF-002 (K-72): `(Sekunden seit READY, crawled)` je `PROGRESS`-Zeile mit
    /// dem Zeitpunkt des Empfangs, beginnend mit `(0, 0)` bei `READY` und endend mit dem Stand der `DONE`-Zeile.
    pub samples: Vec<crate::throughput::Sample>,
}

/// Ein laufendes Kind mit gelesenem stdout.
pub struct Proc {
    child: Child,
    lines: mpsc::Receiver<(Instant, String)>,
    started: Instant,
    stall_timeout: Duration,
    last_crawled: Option<u64>,
    peak_rss: Option<u64>,
}

/// Wert von `key=` in einer Protokollzeile.
pub fn field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    line.split_whitespace()
        .find_map(|w| w.strip_prefix(key)?.strip_prefix('='))
}

/// Zähler einer `DIAG`-Zeile (`DIAG <gruppe> <name>=<n> …`) als `<gruppe>_<name>` → Wert; andere Zeilen: leer.
pub fn parse_diag(line: &str) -> Vec<(String, u64)> {
    let Some(rest) = line.strip_prefix("DIAG ") else {
        return Vec::new();
    };
    let mut words = rest.split_whitespace();
    let Some(group) = words.next() else {
        return Vec::new();
    };
    words
        .filter_map(|w| {
            let (name, value) = w.split_once('=')?;
            Some((format!("{group}_{name}"), value.parse().ok()?))
        })
        .collect()
}

/// `crawled` einer `PROGRESS`-Zeile.
pub fn progress_crawled(line: &str) -> Option<u64> {
    line.strip_prefix("PROGRESS ")?;
    field(line, "crawled")?.parse().ok()
}

impl Proc {
    /// Startet das Kind; stdout wird zeilenweise in einem Thread gelesen, stderr geht an den Elternprozess.
    pub fn spawn(mut cmd: Command, stall_timeout: Duration) -> Result<Self, String> {
        cmd.stdout(Stdio::piped()).stderr(Stdio::inherit());
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Kindprozess startet nicht ({:?}): {e}", cmd.get_program()))?;
        let stdout = child.stdout.take().ok_or("kein stdout des Kindes")?;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                // der Zeitstempel entsteht beim Empfang, nicht erst, wenn der Elternprozess die Zeile verarbeitet
                if tx.send((Instant::now(), line)).is_err() {
                    break;
                }
            }
        });
        Ok(Proc {
            child,
            lines: rx,
            started: Instant::now(),
            stall_timeout,
            last_crawled: None,
            peak_rss: None,
        })
    }

    /// Nächste Zeile; `Ok(None)` bei Ende der Ausgabe, `Err` bei Stillstand länger als der Timeout (das Kind wird
    /// dann hart beendet).
    pub fn next_line(&mut self) -> Result<Option<String>, String> {
        Ok(self.next_stamped_line()?.map(|(_, line)| line))
    }

    /// Wie [`Proc::next_line`], mit dem Zeitpunkt, zu dem die Zeile eintraf.
    pub fn next_stamped_line(&mut self) -> Result<Option<(Instant, String)>, String> {
        match self.lines.recv_timeout(self.stall_timeout) {
            Ok(stamped) => Ok(Some(stamped)),
            Err(RecvTimeoutError::Disconnected) => Ok(None),
            Err(RecvTimeoutError::Timeout) => {
                let _ = self.child.kill();
                let _ = self.child.wait();
                Err(format!(
                    "das Kind meldet seit {} s nichts mehr (Stillstand, hart beendet)",
                    self.stall_timeout.as_secs()
                ))
            }
        }
    }

    /// Liest, bis `crawled` den Wert `target` erreicht (`Ok(Some(crawled))`) oder das Kind endet (`Ok(None)`).
    pub fn wait_until_crawled(&mut self, target: u64) -> Result<Option<u64>, String> {
        while let Some(line) = self.next_line()? {
            if let Some(n) = progress_crawled(&line) {
                self.last_crawled = Some(n);
                if n >= target {
                    return Ok(Some(n));
                }
            }
        }
        Ok(None)
    }

    /// Beendet das Kind hart (`SIGKILL` bzw. `TerminateProcess`), wartet darauf und liefert den letzten gemeldeten
    /// Stand von `crawled` (auch aus Zeilen, die erst nach dem Abbruchbefehl eintrafen).
    pub fn kill(mut self) -> std::io::Result<Option<u64>> {
        self.child.kill()?;
        self.child.wait()?;
        while let Ok((_, line)) = self.lines.recv() {
            if let Some(n) = progress_crawled(&line) {
                self.last_crawled = Some(n);
            }
        }
        Ok(self.last_crawled)
    }

    /// Liest bis zum Ende der Ausgabe, wartet auf den Exit und misst den Spitzen-RSS.
    pub fn finish(mut self) -> Result<RunResult, String> {
        let mut done: Option<(String, String, u64)> = None;
        let mut diag = BTreeMap::new();
        let (mut last_crawled, mut last_change) = (u64::MAX, Instant::now());
        let mut max_stall = Duration::ZERO;
        // Ursprung der Stützstellen: READY (die Crawl-Datei existiert, der Crawl läuft); ohne READY der Prozessstart
        let mut origin = self.started;
        let mut samples: Vec<crate::throughput::Sample> = vec![(0.0, 0)];
        while let Some((at, line)) = self.next_stamped_line()? {
            self.peak_rss = self.peak_rss.max(crate::rss::sample_child(&self.child));
            if line.starts_with("READY ") {
                origin = at;
                samples = vec![(0.0, 0)];
            } else if let Some(n) = progress_crawled(&line) {
                if n != last_crawled {
                    if last_crawled != u64::MAX {
                        max_stall = max_stall.max(at.saturating_duration_since(last_change));
                    }
                    last_crawled = n;
                    last_change = at;
                }
                samples.push((at.saturating_duration_since(origin).as_secs_f64(), n));
            } else if line.starts_with("DIAG ") {
                diag.extend(parse_diag(&line));
            } else if line.starts_with("DONE ") {
                let crawled = field(&line, "crawled")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                samples.push((at.saturating_duration_since(origin).as_secs_f64(), crawled));
                done = Some((
                    field(&line, "state").unwrap_or("?").to_string(),
                    field(&line, "end_reason").unwrap_or("?").to_string(),
                    crawled,
                ));
            }
        }
        let exit = wait_with_peak_rss(&mut self.child).map_err(|e| format!("warten: {e}"))?;
        let seconds = self.started.elapsed().as_secs_f64();
        if !exit.success {
            return Err(format!("Kind endete mit Code {:?}", exit.code));
        }
        let (state, end_reason, crawled) = done.ok_or("das Kind hat kein DONE gemeldet")?;
        Ok(RunResult {
            crawled,
            state,
            end_reason,
            seconds,
            peak_rss_bytes: exit.peak_rss_bytes.max(self.peak_rss),
            max_stall_s: max_stall.as_secs_f64(),
            diag,
            samples,
        })
    }
}

/// Pfad eines Beispielprogramms (`examples/<name>`) neben dem Programm `exe` (`target/<profil>/`); fehlt es, wird es
/// mit `cargo build --example <name> -p <package>` im selben Profil gebaut.
pub fn example_binary(exe: &Path, package: &str, name: &str) -> Result<PathBuf, String> {
    let profile_dir = exe
        .parent()
        .ok_or_else(|| format!("{} hat kein Verzeichnis", exe.display()))?;
    let path = profile_dir
        .join("examples")
        .join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    if path.exists() {
        return Ok(path);
    }
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut cmd = Command::new(cargo);
    cmd.args(["build", "--example", name, "-p", package, "--locked"]);
    if profile_dir.file_name().is_some_and(|n| n == "release") {
        cmd.arg("--release");
    }
    let status = cmd
        .status()
        .map_err(|e| format!("cargo startet nicht: {e}"))?;
    if !status.success() || !path.exists() {
        return Err(format!(
            "Beispiel `{name}` ({package}) lässt sich nicht bauen: {}",
            path.display()
        ));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_protocol_lines() {
        assert_eq!(
            progress_crawled("PROGRESS crawled=42 queued=7 discovered=49"),
            Some(42)
        );
        assert_eq!(progress_crawled("DONE state=Completed crawled=42"), None);
        assert_eq!(
            field("DONE state=Completed end_reason=- crawled=9", "end_reason"),
            Some("-")
        );
        assert_eq!(field("DONE state=Completed", "crawled"), None);
        assert_eq!(
            parse_diag("DIAG engine items=3 fetch_ns=40 junk"),
            vec![
                ("engine_items".to_string(), 3),
                ("engine_fetch_ns".to_string(), 40)
            ]
        );
        assert!(parse_diag("PROGRESS crawled=1").is_empty());
    }
}
