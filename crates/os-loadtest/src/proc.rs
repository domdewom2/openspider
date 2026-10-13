//! Kindprozesse der Lasttests: Start, Zeilenprotokoll (`READY`/`PROGRESS`/`DONE`), hartes Beenden, Messung.
//!
//! Beide Kinder (`os-loadtest crawl-child` und `examples/crawl_child.rs` von os-engine) sprechen dasselbe
//! Protokoll, siehe [`crate::child`].

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
}

/// Ein laufendes Kind mit gelesenem stdout.
pub struct Proc {
    child: Child,
    lines: mpsc::Receiver<String>,
    started: Instant,
    stall_timeout: Duration,
}

/// Wert von `key=` in einer Protokollzeile.
pub fn field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    line.split_whitespace()
        .find_map(|w| w.strip_prefix(key)?.strip_prefix('='))
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
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        Ok(Proc {
            child,
            lines: rx,
            started: Instant::now(),
            stall_timeout,
        })
    }

    /// Nächste Zeile; `Ok(None)` bei Ende der Ausgabe, `Err` bei Stillstand länger als der Timeout (das Kind wird
    /// dann hart beendet).
    pub fn next_line(&mut self) -> Result<Option<String>, String> {
        match self.lines.recv_timeout(self.stall_timeout) {
            Ok(line) => Ok(Some(line)),
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
                if n >= target {
                    return Ok(Some(n));
                }
            }
        }
        Ok(None)
    }

    /// Beendet das Kind hart (`SIGKILL` bzw. `TerminateProcess`) und wartet darauf.
    pub fn kill(mut self) -> std::io::Result<()> {
        self.child.kill()?;
        self.child.wait().map(|_| ())
    }

    /// Liest bis zum Ende der Ausgabe, wartet auf den Exit und misst den Spitzen-RSS.
    pub fn finish(mut self) -> Result<RunResult, String> {
        let mut done: Option<(String, String, u64)> = None;
        let (mut last_crawled, mut last_change) = (u64::MAX, Instant::now());
        let mut max_stall = Duration::ZERO;
        while let Some(line) = self.next_line()? {
            if let Some(n) = progress_crawled(&line) {
                if n != last_crawled {
                    if last_crawled != u64::MAX {
                        max_stall = max_stall.max(last_change.elapsed());
                    }
                    last_crawled = n;
                    last_change = Instant::now();
                }
            } else if line.starts_with("DONE ") {
                done = Some((
                    field(&line, "state").unwrap_or("?").to_string(),
                    field(&line, "end_reason").unwrap_or("?").to_string(),
                    field(&line, "crawled")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0),
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
            peak_rss_bytes: exit.peak_rss_bytes,
            max_stall_s: max_stall.as_secs_f64(),
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
    }
}
