//! Sitzungsmarke der App (OUT-246 `unclean_exit`): `crashes/session-<pid>.json` mit PID, Prozess-Startzeit und
//! Version. Die App legt sie beim Start an und löscht sie beim geordneten Beenden. Findet ein Start eine Marke, deren
//! Prozess (gleiche PID **und** gleiche Startzeit) nicht mehr läuft, entsteht nachträglich ein Bericht.

use super::shared::{self, Context, Report};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Inhalt der Marke.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Marker {
    pub pid: u32,
    /// Prozess-Startzeit in Sekunden seit 1970 (Betriebssystem).
    pub start_time: u64,
    pub version: String,
    #[serde(default)]
    pub build: String,
    /// Byte-Versatz in `openspider.log`, ab dem das Log zu diesem Lauf gehört (Rückfall).
    #[serde(default)]
    pub log_offset: u64,
    /// Kennung der Startzeile im Log (`App-Sitzung gestartet`); findet das Log-Ende des Laufs auch nach Rotation.
    #[serde(default)]
    pub session: String,
    /// Zustand beim letzten Wechsel (für `crawl_running`/`rendering_active` des nachträglichen Berichts).
    #[serde(default)]
    pub crawl_running: bool,
    #[serde(default)]
    pub rendering_active: bool,
    /// Der Lauf hat bereits einen `panic`-Bericht geschrieben.
    #[serde(default)]
    pub panic_report: bool,
}

/// Prozessliste (im Test gemockt).
pub trait ProcessTable {
    /// Startzeit des laufenden Prozesses `pid`; `None`, wenn kein solcher Prozess läuft.
    fn start_time(&self, pid: u32) -> Option<u64>;
}

/// Prozessliste des Betriebssystems (plattformneutral über `sysinfo`).
pub struct SystemProcesses;

impl ProcessTable for SystemProcesses {
    fn start_time(&self, pid: u32) -> Option<u64> {
        use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
        let pid = Pid::from_u32(pid);
        let mut system = System::new();
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[pid]),
            true,
            ProcessRefreshKind::nothing(),
        );
        system.process(pid).map(|p| p.start_time())
    }
}

/// Pfad der Marke eines Prozesses.
pub fn marker_path(dir: &Path, pid: u32) -> PathBuf {
    dir.join(format!("session-{pid}.json"))
}

/// Schreibt die Marke (atomar).
pub fn write_marker(dir: &Path, marker: &Marker) -> std::io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let path = marker_path(dir, marker.pid);
    let text = serde_json::to_vec_pretty(marker).map_err(std::io::Error::other)?;
    shared::write_atomic(&path, &text)?;
    Ok(path)
}

/// Marken im Ordner.
pub fn markers(dir: &Path) -> Vec<(PathBuf, Option<Marker>)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<(PathBuf, Option<Marker>)> = entries
        .flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.starts_with("session-") && name.ends_with(".json")
        })
        .map(|e| {
            let marker = fs::read_to_string(e.path())
                .ok()
                .and_then(|t| serde_json::from_str::<Marker>(&t).ok());
            (e.path(), marker)
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Ergebnis der Prüfung beim Start.
#[derive(Debug, Default)]
pub struct Scan {
    /// Neu geschriebene `unclean_exit`-Berichte.
    pub reports: Vec<Report>,
    /// Entfernte Marken.
    pub removed: Vec<PathBuf>,
}

/// Prüft alle Marken: läuft ihr Prozess (gleiche PID und Startzeit), bleibt die Marke unangetastet. Sonst entsteht –
/// wenn der Lauf keinen `panic`-Bericht hatte – ein `unclean_exit`-Bericht mit dem Log jenes Laufs; die Marke wird
/// entfernt. Eine unlesbare Marke wird ohne Bericht entfernt.
pub fn scan(ctx: &Context, processes: &dyn ProcessTable, log: &Path) -> Scan {
    let mut out = Scan::default();
    for (path, marker) in markers(&ctx.dir) {
        let Some(marker) = marker else {
            if fs::remove_file(&path).is_ok() {
                out.removed.push(path);
            }
            continue;
        };
        if processes.start_time(marker.pid) == Some(marker.start_time) {
            continue; // läuft noch (z. B. zweite Instanz)
        }
        if !marker.panic_report {
            let tail = previous_log_tail(log, &marker);
            let mut report = shared::build(
                ctx,
                shared::KIND_UNCLEAN_EXIT,
                "",
                "previous app process ended without an orderly shutdown",
                "",
                Vec::new(),
                tail,
            );
            report.app_version = marker.version.clone();
            if !marker.build.is_empty() {
                report.build = marker.build.clone();
            }
            report.uptime_s = 0;
            report.crawl_running = marker.crawl_running;
            report.rendering_active = marker.rendering_active;
            if shared::write_report(&ctx.dir, &mut report).is_ok() {
                out.reports.push(report);
            } else {
                // Ohne Bericht bleibt die Marke für den nächsten Start liegen.
                continue;
            }
        }
        if fs::remove_file(&path).is_ok() {
            out.removed.push(path);
        }
    }
    out
}

/// Log des beendeten Laufs: ab seiner Startzeile (Kennung `session`), nach einer Rotation zusätzlich aus
/// `openspider.1.log`; ohne Startzeile ab dem gespeicherten Versatz.
pub fn previous_log_tail(log: &Path, marker: &Marker) -> Vec<String> {
    let after_start = |path: &Path| -> Option<Vec<String>> {
        if marker.session.is_empty() {
            return None;
        }
        let raw = fs::read(path).ok()?;
        let text = String::from_utf8_lossy(&raw);
        let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
        let start = lines.iter().rposition(|l| l.contains(&marker.session))?;
        Some(lines[start..].iter().map(|l| l.to_string()).collect())
    };
    // Höchstens 2 MB je Datei; `build` behält davon die letzten 200 Zeilen.
    let keep_last = |mut lines: Vec<String>| {
        let drop = lines.len().saturating_sub(20_000);
        lines.drain(..drop);
        lines
    };
    if let Some(lines) = after_start(log) {
        return keep_last(lines);
    }
    if let Some(mut lines) = after_start(&log.with_file_name("openspider.1.log")) {
        lines.extend(shared::log_tail(log, 0, usize::MAX));
        return keep_last(lines);
    }
    shared::log_tail(log, marker.log_offset, usize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::time::Instant;

    struct Table(HashMap<u32, u64>);
    impl ProcessTable for Table {
        fn start_time(&self, pid: u32) -> Option<u64> {
            self.0.get(&pid).copied()
        }
    }

    fn ctx(dir: &Path) -> Context {
        Context {
            dir: dir.join("crashes"),
            info: shared::Static::current("app"),
            sanitizer: shared::Sanitizer::new([PathBuf::from("/home/alice")]),
            log: None,
            started: Instant::now(),
            on_written: None,
        }
    }

    fn marker(pid: u32, start: u64) -> Marker {
        Marker {
            pid,
            start_time: start,
            version: "0.9.0".into(),
            build: "old1234".into(),
            log_offset: 0,
            session: String::new(),
            crawl_running: true,
            rendering_active: false,
            panic_report: false,
        }
    }

    // spec: OUT-246
    #[test]
    fn running_process_with_same_start_time_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx(dir.path());
        write_marker(&ctx.dir, &marker(42, 1000)).unwrap();
        let scan = scan(
            &ctx,
            &Table(HashMap::from([(42, 1000)])),
            &dir.path().join("openspider.log"),
        );
        assert!(scan.reports.is_empty() && scan.removed.is_empty());
        assert!(marker_path(&ctx.dir, 42).exists());
    }

    // spec: OUT-246
    #[test]
    fn reused_pid_with_other_start_time_counts_as_crash() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx(dir.path());
        let log = dir.path().join("openspider.log");
        fs::write(
            &log,
            "earlier run\nrun line 1 https://crash-secret.test/x\nrun line 2\n",
        )
        .unwrap();
        let mut m = marker(42, 1000);
        m.log_offset = "earlier run\n".len() as u64;
        write_marker(&ctx.dir, &m).unwrap();
        let scan = scan(&ctx, &Table(HashMap::from([(42, 2000)])), &log);
        assert_eq!(scan.reports.len(), 1);
        let r = &scan.reports[0];
        assert_eq!(r.kind, "unclean_exit");
        assert_eq!(r.app_version, "0.9.0");
        assert_eq!(r.build, "old1234");
        assert!(r.crawl_running);
        assert_eq!(r.log_tail, ["run line 1 <url>", "run line 2"]);
        assert!(!marker_path(&ctx.dir, 42).exists());
        assert_eq!(shared::list(&ctx.dir).len(), 1);
    }

    // spec: OUT-246
    #[test]
    fn dead_process_yields_exactly_one_report_and_second_start_none() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx(dir.path());
        write_marker(&ctx.dir, &marker(7, 5)).unwrap();
        let none = Table(HashMap::new());
        let log = dir.path().join("openspider.log");
        assert_eq!(scan(&ctx, &none, &log).reports.len(), 1);
        assert_eq!(scan(&ctx, &none, &log).reports.len(), 0);
        assert_eq!(shared::list(&ctx.dir).len(), 1);
    }

    // spec: OUT-246
    #[test]
    fn run_with_panic_report_gets_no_extra_unclean_exit() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx(dir.path());
        let mut m = marker(9, 5);
        m.panic_report = true;
        write_marker(&ctx.dir, &m).unwrap();
        let scan = scan(
            &ctx,
            &Table(HashMap::new()),
            &dir.path().join("openspider.log"),
        );
        assert!(scan.reports.is_empty());
        assert_eq!(scan.removed.len(), 1);
    }

    // spec: OUT-246
    #[test]
    fn unreadable_marker_is_removed_without_report() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx(dir.path());
        fs::create_dir_all(&ctx.dir).unwrap();
        fs::write(marker_path(&ctx.dir, 3), "{kaputt").unwrap();
        let scan = scan(
            &ctx,
            &Table(HashMap::new()),
            &dir.path().join("openspider.log"),
        );
        assert!(scan.reports.is_empty());
        assert_eq!(scan.removed.len(), 1);
    }

    // spec: OUT-246
    #[test]
    fn own_process_is_found_in_the_system_table() {
        let me = std::process::id();
        let start = SystemProcesses.start_time(me);
        assert!(start.is_some_and(|s| s > 0));
        assert_eq!(SystemProcesses.start_time(me), start);
    }

    // spec: OUT-246
    #[test]
    fn log_of_the_run_starts_at_its_session_line_even_after_rotation() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("openspider.log");
        let mut m = marker(1, 1);
        m.session = "sess-abc".into();
        fs::write(&log, "old\nApp-Sitzung gestartet session=sess-abc\nrun-1\n").unwrap();
        assert_eq!(
            previous_log_tail(&log, &m),
            ["App-Sitzung gestartet session=sess-abc", "run-1"]
        );
        fs::write(
            dir.path().join("openspider.1.log"),
            "before\nstart sess-abc\nafter-a\n",
        )
        .unwrap();
        fs::write(&log, "after-b\n").unwrap();
        assert_eq!(
            previous_log_tail(&log, &m),
            ["start sess-abc", "after-a", "after-b"]
        );
        m.session.clear();
        m.log_offset = 0;
        assert_eq!(previous_log_tail(&log, &m), ["after-b"]);
    }
}
