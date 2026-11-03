//! Absturzberichte, gemeinsamer Teil von App und CLI (OUT-246): Berichtsformat, Panic-Hook-Teil, Schreiben mit
//! Größengrenze, Aufbewahrung und Ablage im Ordner `crashes/` des Log-Ordners.
//!
//! Die CLI kompiliert diese Datei über `#[path]` mit (wie `diag.rs`); sie darf deshalb keine Typen aus `os-app`
//! benutzen. Der Elternmodul stellt `diagnostics` bereit (`os-app`: `crate::diag`, CLI: `logging::diagnostics`).
//! Alles, was im Panic-Hook läuft, ist fehlertolerant und panikt nicht: ein Fehler beim Schreiben (Ordner
//! schreibgeschützt, Platte voll) lässt den Bericht still entfallen.

#[path = "sanitize.rs"]
pub mod sanitize;

use super::diagnostics;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant, SystemTime},
};

pub use sanitize::Sanitizer;

/// Ordner der Berichte im Log-Ordner.
pub const DIR: &str = "crashes";
/// Größte Länge eines Berichts in Byte.
pub const MAX_REPORT_BYTES: usize = 64 * 1024;
/// Höchstens so viele Berichte bleiben liegen.
pub const MAX_REPORTS: usize = 20;
/// Höchstens so viele Byte belegen alle Berichte.
pub const MAX_TOTAL_BYTES: u64 = 5 * 1024 * 1024;
/// Berichte, die älter sind, werden beim App-Start gelöscht.
pub const MAX_AGE: Duration = Duration::from_secs(30 * 86_400);
/// Höchstens so viele Logzeilen in `log_tail`.
pub const MAX_LOG_LINES: usize = 200;
/// Höchstens so viele Einträge in `backtrace`.
pub const MAX_FRAMES: usize = 64;
/// Höchstens so viele Zeichen in `message`.
pub const MAX_MESSAGE_CHARS: usize = 500;

/// Arten (`kind`).
pub const KIND_PANIC: &str = "panic";
pub const KIND_FRONTEND: &str = "frontend";
pub const KIND_UNCLEAN_EXIT: &str = "unclean_exit";

/// Ein Absturzbericht, genau die Felder aus OUT-246 in dieser Reihenfolge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub format: u32,
    pub id: String,
    pub kind: String,
    pub time: String,
    pub app_version: String,
    pub build: String,
    pub os: String,
    pub arch: String,
    pub process: String,
    pub uptime_s: u64,
    pub thread: String,
    pub message: String,
    pub location: String,
    pub backtrace: Vec<String>,
    pub crawl_running: bool,
    pub rendering_active: bool,
    pub log_tail: Vec<String>,
    pub sent: Option<String>,
    pub server_ref: Option<String>,
}

impl Report {
    /// Text des Berichts (eingerücktes JSON ohne abschließenden Zeilenumbruch); genau dieser Text liegt in der
    /// Datei, wird angezeigt und als `report` gesendet.
    pub fn to_text(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Liest und prüft einen Bericht (Format 1, keine fremden Felder).
    pub fn parse(text: &str) -> Option<Report> {
        serde_json::from_str::<Report>(text)
            .ok()
            .filter(|r| r.format == 1)
    }
}

/// Feste Angaben des Prozesses, die jeder Bericht trägt.
#[derive(Debug, Clone)]
pub struct Static {
    pub app_version: String,
    pub build: String,
    pub os: String,
    pub arch: String,
    pub process: &'static str,
}

impl Static {
    /// `process`: `app` oder `cli`.
    pub fn current(process: &'static str) -> Static {
        Static {
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            build: build_hash(),
            os: os_name(),
            arch: std::env::consts::ARCH.to_string(),
            process,
        }
    }
}

/// Commit-Kurzhash aus `OPENSPIDER_BUILD` (erstes Wort), sonst `dev`.
pub fn build_hash() -> String {
    option_env!("OPENSPIDER_BUILD")
        .and_then(|b| b.split_whitespace().next())
        .unwrap_or("dev")
        .chars()
        .take(40)
        .collect()
}

/// Name und Version des Betriebssystems.
pub fn os_name() -> String {
    sysinfo::System::long_os_version()
        .or_else(sysinfo::System::name)
        .unwrap_or_else(|| std::env::consts::OS.to_string())
}

/// Bereinigung über die OUT-243-Regeln des Logs (vom Elternmodul bereitgestellt).
pub fn redact_secrets(text: &str) -> String {
    diagnostics::redact_text(text)
}

/// Zustand des Prozesses für den Panic-Hook.
#[derive(Debug)]
pub struct Context {
    /// Ordner der Berichte (`<logs>/crashes`).
    pub dir: PathBuf,
    pub info: Static,
    pub sanitizer: Sanitizer,
    /// Log des Laufs und Byte-Versatz, ab dem es zu diesem Lauf gehört.
    pub log: Option<(PathBuf, u64)>,
    pub started: Instant,
    /// Wird nach einem Panic-Bericht neu geschrieben (Sitzungsmarke der App, sonst `None`).
    pub on_written: Option<fn(&Report)>,
}

static CONTEXT: Mutex<Option<Arc<Context>>> = Mutex::new(None);
/// Läuft gerade ein Crawl?
pub static CRAWL_RUNNING: AtomicBool = AtomicBool::new(false);
/// Rendert gerade ein Crawl mit JavaScript?
pub static RENDERING_ACTIVE: AtomicBool = AtomicBool::new(false);
static PROCESS_START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

/// Setzt den Zustand (die zuletzt gestartete App bzw. CLI gilt).
pub fn install(context: Arc<Context>) {
    let _ = PROCESS_START.set(context.started);
    *CONTEXT.lock().unwrap_or_else(|p| p.into_inner()) = Some(context);
}

/// Aktueller Zustand, ohne im Panic-Hook zu blockieren (die Sperre wird nur kurz gehalten).
pub fn context() -> Option<Arc<Context>> {
    for _ in 0..200 {
        match CONTEXT.try_lock() {
            Ok(guard) => return guard.clone(),
            Err(std::sync::TryLockError::Poisoned(p)) => return p.into_inner().clone(),
            Err(std::sync::TryLockError::WouldBlock) => {
                std::thread::sleep(Duration::from_millis(1))
            }
        }
    }
    None
}

/// Sekunden seit Prozessstart.
pub fn uptime_s() -> u64 {
    PROCESS_START.get().map_or(0, |s| s.elapsed().as_secs())
}

/// Zeitstempel ISO 8601 mit Offset.
pub fn now_iso() -> String {
    diagnostics::timestamp()
}

/// Zufällige UUID v4. Die Zufallsbits stammen aus zwei UUID v7 der gemeinsamen Quelle `os_model::RandomIds`
/// (je 74 Zufallsbits); die CLI hat keine eigene `uuid`-Abhängigkeit (D9).
pub fn new_id() -> String {
    use os_model::IdSource;
    let now = os_model::Timestamp(time::OffsetDateTime::now_utc());
    let a = *os_model::RandomIds.new_crawl_id(now).0.as_bytes();
    let b = *os_model::RandomIds.new_crawl_id(now).0.as_bytes();
    // Zufällig in v7: Byte 7 und Bytes 9..16.
    let mut bytes = [0u8; 16];
    bytes[..7].copy_from_slice(&a[9..16]);
    bytes[7] = a[7];
    bytes[8..15].copy_from_slice(&b[9..16]);
    bytes[15] = b[7];
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// Erste Zeile, höchstens 500 Zeichen.
pub fn first_line(text: &str) -> String {
    text.lines()
        .next()
        .unwrap_or("")
        .chars()
        .take(MAX_MESSAGE_CHARS)
        .collect()
}

/// `<crate-relativer Pfad>:<Zeile>`: der Pfad ab dem Ordner des Crates (Komponente vor dem letzten `src`, `tests`,
/// `benches` oder `examples`).
pub fn crate_relative(file: &str, line: u32) -> String {
    let parts: Vec<&str> = file.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
    let rel = match parts.iter().rposition(|p| *p == "src").or_else(|| {
        parts
            .iter()
            .rposition(|p| matches!(*p, "tests" | "benches" | "examples"))
    }) {
        Some(i) if i > 0 => parts[i - 1..].join("/"),
        _ => parts.last().copied().unwrap_or("").to_string(),
    };
    format!("{rel}:{line}")
}

/// Funktionsnamen aus einem `std::backtrace::Backtrace` (ohne Pfade), ohne die Rahmen der Panic-Behandlung.
pub fn backtrace_frames(text: &str) -> Vec<String> {
    let names: Vec<String> = text
        .lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let (num, rest) = line.split_once(": ")?;
            num.chars()
                .all(|c| c.is_ascii_digit())
                .then(|| rest.trim().to_string())
        })
        .filter(|name| !name.is_empty())
        .collect();
    let machinery = |n: &str| {
        n.starts_with("std::panicking")
            || n.starts_with("core::panicking")
            || n.starts_with("std::backtrace")
            || n.starts_with("std::sys::backtrace")
            || n.starts_with("rust_begin_unwind")
            || n.starts_with("__rustc")
            || (n.contains("crash") && n.ends_with("::on_panic"))
            || (n.contains("logging::init") && n.contains("{{closure}}"))
            || n.starts_with("<alloc::boxed::Box<F,A> as core::ops::function::Fn")
    };
    let skip = names
        .iter()
        .take(32)
        .rposition(|n| machinery(n))
        .map_or(0, |i| i + 1);
    names.into_iter().skip(skip).take(MAX_FRAMES).collect()
}

/// Letzte `max` Zeilen ab `offset` (Zeilen dieses Laufs); fehlende Datei → leer.
pub fn log_tail(path: &Path, offset: u64, max: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let Ok(mut file) = fs::File::open(path) else {
        return lines;
    };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    // Bei rotiertem Log gehört die neue Datei vollständig zu diesem Lauf.
    let start = if offset <= len { offset } else { 0 };
    // Nur das Ende lesen: 200 Zeilen passen in die letzten 2 MB.
    let start = start.max(len.saturating_sub(2 * 1024 * 1024));
    if file.seek(SeekFrom::Start(start)).is_err() {
        return lines;
    }
    let mut raw = Vec::new();
    if file.read_to_end(&mut raw).is_err() {
        return lines;
    }
    let text = String::from_utf8_lossy(&raw);
    lines.extend(
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .map(str::to_string),
    );
    let drop = lines.len().saturating_sub(max);
    lines.drain(..drop);
    lines
}

/// Kürzt den Bericht auf 64 KB: erst `log_tail` von vorn, dann `backtrace` von hinten.
pub fn fit(report: &mut Report) -> String {
    let mut text = report.to_text();
    while text.len() > MAX_REPORT_BYTES && !report.log_tail.is_empty() {
        let excess = text.len() - MAX_REPORT_BYTES;
        // Mehrere Zeilen auf einmal entfernen, wenn viel zu viel da ist.
        let mut removed = 0;
        let mut n = 0;
        while n < report.log_tail.len() && removed < excess {
            removed += report.log_tail[n].len() + 8;
            n += 1;
        }
        report.log_tail.drain(..n.max(1));
        text = report.to_text();
    }
    while text.len() > MAX_REPORT_BYTES && !report.backtrace.is_empty() {
        report.backtrace.pop();
        text = report.to_text();
    }
    if text.len() > MAX_REPORT_BYTES {
        report.message = first_line(&report.message).chars().take(100).collect();
        report.thread = report.thread.chars().take(100).collect();
        report.location = report.location.chars().take(200).collect();
        text = report.to_text();
    }
    text
}

/// Dateiname `crash-<YYYYMMDD-HHMMSS>-<art>.json` (UTC); bei gleicher Sekunde mit Zähler.
fn file_name(stamp: &str, kind: &str, n: u32) -> String {
    if n == 0 {
        format!("crash-{stamp}-{kind}.json")
    } else {
        format!("crash-{stamp}-{kind}-{n}.json")
    }
}

/// Schreibt einen Bericht nach `dir` (Grenze 64 KB). Liefert den Pfad.
pub fn write_report(dir: &Path, report: &mut Report) -> io::Result<PathBuf> {
    let text = fit(report);
    fs::create_dir_all(dir)?;
    let stamp = diagnostics::file_stamp();
    for n in 0..100 {
        let path = dir.join(file_name(&stamp, &report.kind, n));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                let written = file
                    .write_all(text.as_bytes())
                    .and_then(|()| file.sync_all());
                if let Err(e) = written {
                    drop(file);
                    let _ = fs::remove_file(&path);
                    return Err(e);
                }
                return Ok(path);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "kein freier Dateiname",
    ))
}

/// Baut einen Bericht mit den festen Angaben des Kontexts; Texte werden bereinigt, `log_tail` behält die letzten
/// 200 der übergebenen Logzeilen.
#[allow(clippy::too_many_arguments)]
pub fn build(
    ctx: &Context,
    kind: &str,
    thread: &str,
    message: &str,
    location: &str,
    backtrace: Vec<String>,
    log_lines: Vec<String>,
) -> Report {
    // Hostnamen aus allen URLs der Eingaben (auch aus Logzeilen vor den letzten 200) werden überall maskiert.
    let s = &ctx.sanitizer.with_hosts_from(
        std::iter::once(message)
            .chain(backtrace.iter().map(String::as_str))
            .chain(log_lines.iter().map(String::as_str)),
    );
    let skip = log_lines.len().saturating_sub(MAX_LOG_LINES);
    let log_tail: Vec<&String> = log_lines.iter().skip(skip).collect();
    Report {
        format: 1,
        id: new_id(),
        kind: kind.to_string(),
        time: now_iso(),
        app_version: ctx.info.app_version.clone(),
        build: ctx.info.build.clone(),
        os: ctx.info.os.clone(),
        arch: ctx.info.arch.clone(),
        process: ctx.info.process.to_string(),
        uptime_s: ctx.started.elapsed().as_secs(),
        thread: s.clean(thread),
        message: first_line(&s.clean(&first_line(message))),
        location: s.clean(location),
        backtrace: backtrace.iter().map(|f| s.clean(f)).collect(),
        crawl_running: CRAWL_RUNNING.load(Ordering::Relaxed),
        rendering_active: RENDERING_ACTIVE.load(Ordering::Relaxed),
        log_tail: log_tail.iter().map(|l| s.clean(l)).collect(),
        sent: None,
        server_ref: None,
    }
}

/// Text der Panic-Nutzlast.
pub fn panic_message(info: &std::panic::PanicHookInfo<'_>) -> String {
    if let Some(s) = info.payload().downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = info.payload().downcast_ref::<String>() {
        s.clone()
    } else {
        "Box<dyn Any>".to_string()
    }
}

/// Teil des Panic-Hooks: schreibt den Bericht und liefert seinen Pfad. Panikt nie.
pub fn on_panic(info: &std::panic::PanicHookInfo<'_>, backtrace: &str) -> Option<PathBuf> {
    let ctx = context()?;
    let thread = std::thread::current();
    let location = info
        .location()
        .map(|l| crate_relative(l.file(), l.line()))
        .unwrap_or_default();
    let tail = ctx
        .log
        .as_ref()
        .map(|(path, offset)| log_tail(path, *offset, usize::MAX))
        .unwrap_or_default();
    let mut report = build(
        &ctx,
        KIND_PANIC,
        thread.name().unwrap_or(""),
        &panic_message(info),
        &location,
        backtrace_frames(backtrace),
        tail,
    );
    let path = write_report(&ctx.dir, &mut report).ok()?;
    if let Some(f) = ctx.on_written {
        f(&report);
    }
    Some(path)
}

/// Gespeicherter Bericht.
#[derive(Debug, Clone)]
pub struct Stored {
    pub path: PathBuf,
    pub text: String,
    pub report: Report,
}

/// Dateien `crash-*.json`, neueste zuerst (der Name beginnt mit dem Zeitstempel).
pub fn report_files(dir: &Path) -> Vec<(PathBuf, u64)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<(PathBuf, u64)> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.starts_with("crash-") && name.ends_with(".json")
        })
        .map(|e| (e.path(), e.metadata().map(|m| m.len()).unwrap_or(0)))
        .collect();
    files.sort_by_key(|f| std::cmp::Reverse(sort_key(&f.0)));
    files
}

/// Zeitstempel und Zähler aus dem Dateinamen (für die Reihenfolge).
fn sort_key(path: &Path) -> (String, u32) {
    let name = path
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stamp: String = name.chars().skip(6).take(15).collect();
    let counter = name
        .rsplit_once('-')
        .and_then(|(_, n)| n.parse::<u32>().ok())
        .unwrap_or(0);
    (stamp, counter)
}

/// Zeitpunkt aus dem Dateinamen (UTC), sonst Änderungszeit.
fn file_time(path: &Path) -> Option<SystemTime> {
    let (stamp, _) = sort_key(path);
    let format =
        time::format_description::parse_borrowed::<2>("[year][month][day]-[hour][minute][second]")
            .ok()?;
    time::PrimitiveDateTime::parse(&stamp, &format)
        .ok()
        .map(|t| t.assume_utc().into())
        .or_else(|| fs::metadata(path).and_then(|m| m.modified()).ok())
}

/// Alle lesbaren Berichte, neueste zuerst. Unlesbare Dateien werden übersprungen.
pub fn list(dir: &Path) -> Vec<Stored> {
    report_files(dir)
        .into_iter()
        .filter_map(|(path, _)| {
            let text = fs::read_to_string(&path).ok()?;
            let report = Report::parse(&text)?;
            Some(Stored { path, text, report })
        })
        .collect()
}

/// Aufbewahrung beim App-Start: älter als 30 Tage weg, dann höchstens 20 Berichte und 5 MB (älteste zuerst).
pub fn enforce_retention(dir: &Path, now: SystemTime) -> io::Result<usize> {
    let mut removed = 0;
    let mut kept = Vec::new();
    for (path, size) in report_files(dir) {
        let old =
            file_time(&path).is_some_and(|t| now.duration_since(t).unwrap_or_default() > MAX_AGE);
        if old {
            fs::remove_file(&path)?;
            removed += 1;
        } else {
            kept.push((path, size));
        }
    }
    // kept: neueste zuerst
    let mut total: u64 = kept.iter().map(|k| k.1).sum();
    while kept.len() > MAX_REPORTS || total > MAX_TOTAL_BYTES {
        let Some((path, size)) = kept.pop() else {
            break;
        };
        fs::remove_file(&path)?;
        total = total.saturating_sub(size);
        removed += 1;
    }
    Ok(removed)
}

/// Schreibt eine Datei atomar (temporäre Datei, dann Umbenennen).
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    let result = (|| {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(dir: &Path) -> Context {
        Context {
            dir: dir.to_path_buf(),
            info: Static {
                app_version: "1.2.3".into(),
                build: "abc1234".into(),
                os: "TestOS 1".into(),
                arch: "x86_64".into(),
                process: "app",
            },
            sanitizer: Sanitizer::new([PathBuf::from("/home/alice")]),
            log: None,
            started: Instant::now(),
            on_written: None,
        }
    }

    // spec: OUT-246
    #[test]
    fn report_has_exactly_the_specified_fields_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let r = build(
            &ctx(dir.path()),
            KIND_PANIC,
            "main",
            "boom\nzweite",
            "os-app/src/x.rs:3",
            vec![],
            vec![],
        );
        let text = r.to_text();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        let fields = [
            "format",
            "id",
            "kind",
            "time",
            "app_version",
            "build",
            "os",
            "arch",
            "process",
            "uptime_s",
            "thread",
            "message",
            "location",
            "backtrace",
            "crawl_running",
            "rendering_active",
            "log_tail",
            "sent",
            "server_ref",
        ];
        assert_eq!(value.as_object().unwrap().len(), fields.len());
        let positions: Vec<usize> = fields
            .iter()
            .map(|f| {
                text.find(&format!("\n  \"{f}\": "))
                    .unwrap_or_else(|| panic!("{f}"))
            })
            .collect();
        assert!(
            positions.windows(2).all(|w| w[0] < w[1]),
            "Reihenfolge laut OUT-246"
        );
        assert_eq!(value["format"], 1);
        assert_eq!(value["message"], "boom");
        assert!(value["sent"].is_null() && value["server_ref"].is_null());
        // UUID v4: 8-4-4-4-12 Hex, Version 4, Variante 10xx
        let parts: Vec<&str> = r.id.split('-').collect();
        assert_eq!(
            parts.iter().map(|p| p.len()).collect::<Vec<_>>(),
            [8, 4, 4, 4, 12]
        );
        assert!(r.id.chars().all(|c| c == '-' || c.is_ascii_hexdigit()));
        assert!(parts[2].starts_with('4'));
        assert!(matches!(
            parts[3].chars().next(),
            Some('8' | '9' | 'a' | 'b')
        ));
        assert_ne!(new_id(), new_id());
        // ISO mit Offset
        assert!(
            time::OffsetDateTime::parse(&r.time, &time::format_description::well_known::Rfc3339)
                .is_ok(),
            "{}",
            r.time
        );
        assert!(Report::parse(&r.to_text()).is_some());
        let mut extra = value.clone();
        extra["note"] = "x".into();
        assert!(Report::parse(&extra.to_string()).is_none());
    }

    // spec: OUT-246
    #[test]
    fn message_is_first_line_and_at_most_500_chars() {
        let dir = tempfile::tempdir().unwrap();
        let long = "ä".repeat(900);
        let r = build(&ctx(dir.path()), KIND_PANIC, "", &long, "", vec![], vec![]);
        assert_eq!(r.message.chars().count(), 500);
    }

    // spec: OUT-246
    #[test]
    fn location_is_crate_relative() {
        assert_eq!(
            crate_relative("crates/os-app/src/crash/core.rs", 7),
            "os-app/src/crash/core.rs:7"
        );
        assert_eq!(
            crate_relative(
                "/home/a/.cargo/registry/src/index.crates.io-1/tokio-1.53.3/src/rt/mod.rs",
                9
            ),
            "tokio-1.53.3/src/rt/mod.rs:9"
        );
        assert_eq!(
            crate_relative(r"C:\b\crates\os-cli\src\lib.rs", 1),
            "os-cli/src/lib.rs:1"
        );
        assert_eq!(
            crate_relative("/rustc/abc/library/core/src/option.rs", 2),
            "core/src/option.rs:2"
        );
        assert_eq!(
            crate_relative("crates/os-app/tests/crash_panic.rs", 5),
            "os-app/tests/crash_panic.rs:5"
        );
    }

    // spec: OUT-246
    #[test]
    fn backtrace_keeps_function_names_only_skips_panic_machinery_and_caps_at_64() {
        let mut text = String::from(
            "   0: std::backtrace::Backtrace::force_capture\n             at /rustc/x/library/std/src/backtrace.rs:1:2\n   1: os_app::logging::init_for_app::{{closure}}\n   2: std::panicking::rust_panic_with_hook\n   3: core::panicking::panic_fmt\n",
        );
        for n in 4..100 {
            text.push_str(&format!(
                "  {n}: my_crate::frame_{n}\n             at /home/alice/src/lib.rs:{n}:1\n"
            ));
        }
        let frames = backtrace_frames(&text);
        assert_eq!(frames.len(), 64);
        assert_eq!(frames[0], "my_crate::frame_4");
        assert!(frames.iter().all(|f| !f.contains('/')));
    }

    // spec: OUT-246
    #[test]
    fn oversized_report_trims_log_tail_from_the_front_then_backtrace() {
        let dir = tempfile::tempdir().unwrap();
        let tail: Vec<String> = (0..200)
            .map(|n| format!("{n:03} {}", "x".repeat(1000)))
            .collect();
        let frames: Vec<String> = (0..64).map(|n| format!("f{n}")).collect();
        let mut r = build(
            &ctx(dir.path()),
            KIND_PANIC,
            "",
            "m",
            "",
            frames.clone(),
            tail,
        );
        let path = write_report(dir.path(), &mut r).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.len() <= MAX_REPORT_BYTES);
        assert_eq!(
            r.backtrace, frames,
            "backtrace untouched while log_tail suffices"
        );
        assert!(r.log_tail.len() < 200 && !r.log_tail.is_empty());
        assert!(
            r.log_tail.last().unwrap().starts_with("199 "),
            "newest lines kept"
        );
        // Nur noch Backtrace zu groß
        let mut r = build(
            &ctx(dir.path()),
            KIND_PANIC,
            "",
            "m",
            "",
            (0..64)
                .map(|n| format!("f{n}{}", "y".repeat(2000)))
                .collect(),
            vec!["a".into()],
        );
        let text = fit(&mut r);
        assert!(text.len() <= MAX_REPORT_BYTES);
        assert!(r.log_tail.is_empty());
        assert!(r.backtrace.len() < 64 && r.backtrace[0].starts_with("f0"));
    }

    // spec: OUT-246
    #[test]
    fn retention_removes_old_then_keeps_twenty_and_five_megabytes() {
        let dir = tempfile::tempdir().unwrap();
        let now: SystemTime = time::OffsetDateTime::parse(
            "2026-11-03T12:00:00Z",
            &time::format_description::well_known::Rfc3339,
        )
        .unwrap()
        .into();
        let write =
            |name: &str, size: usize| fs::write(dir.path().join(name), vec![b' '; size]).unwrap();
        write("crash-20261001-120000-panic.json", 10); // > 30 Tage
        for n in 0..22 {
            write(
                &format!("crash-202611{:02}-0000{:02}-panic.json", 1 + n / 10, n),
                10,
            );
        }
        write("session-1.json", 10);
        write("dismissed.json", 10);
        assert_eq!(enforce_retention(dir.path(), now).unwrap(), 3);
        let left = report_files(dir.path());
        assert_eq!(left.len(), 20);
        assert!(left
            .iter()
            .all(|(p, _)| !p.ends_with("crash-20261101-000000-panic.json")));
        assert!(
            dir.path().join("session-1.json").exists()
                && dir.path().join("dismissed.json").exists()
        );
        // 5 MB
        for n in 0..3 {
            write(
                &format!("crash-20261103-1000{n:02}-frontend.json"),
                2 * 1024 * 1024,
            );
        }
        enforce_retention(dir.path(), now).unwrap();
        let left = report_files(dir.path());
        assert!(left.iter().map(|f| f.1).sum::<u64>() <= MAX_TOTAL_BYTES);
        assert!(left[0].0.ends_with("crash-20261103-100002-frontend.json"));
        assert!(!dir
            .path()
            .join("crash-20261103-100000-frontend.json")
            .exists());
    }

    // spec: OUT-246
    #[test]
    fn write_failure_is_an_error_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        let blocked = dir.path().join("file");
        fs::write(&blocked, "x").unwrap();
        let mut r = build(&ctx(dir.path()), KIND_PANIC, "", "m", "", vec![], vec![]);
        // Ein Ordner, der eine Datei ist, lässt sich nicht anlegen.
        assert!(write_report(&blocked.join("crashes"), &mut r).is_err());
    }

    // spec: OUT-246
    #[test]
    fn same_second_reports_get_distinct_names_and_newest_first_order() {
        let dir = tempfile::tempdir().unwrap();
        let c = ctx(dir.path());
        let mut ids = Vec::new();
        for _ in 0..3 {
            let mut r = build(&c, KIND_FRONTEND, "", "m", "", vec![], vec![]);
            write_report(dir.path(), &mut r).unwrap();
            ids.push(r.id);
        }
        let listed: Vec<String> = list(dir.path()).into_iter().map(|s| s.report.id).collect();
        ids.reverse();
        // Innerhalb derselben Sekunde ordnet der Zähler.
        assert_eq!(listed.len(), 3);
        assert_eq!(listed, ids);
    }

    // spec: OUT-246
    #[test]
    fn log_tail_reads_only_this_run_and_the_last_200_lines() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("openspider.log");
        fs::write(&log, "old run\n").unwrap();
        let offset = fs::metadata(&log).unwrap().len();
        let mut f = fs::OpenOptions::new().append(true).open(&log).unwrap();
        for n in 0..250 {
            writeln!(f, "line {n}").unwrap();
        }
        let tail = log_tail(&log, offset, MAX_LOG_LINES);
        assert_eq!(tail.len(), 200);
        assert_eq!(tail[0], "line 50");
        assert!(log_tail(&dir.path().join("missing.log"), 0, 200).is_empty());
        // rotiert: Versatz hinter dem Dateiende → ganze Datei
        assert_eq!(
            log_tail(&log, u64::MAX, 3),
            ["line 247", "line 248", "line 249"]
        );
    }
}
