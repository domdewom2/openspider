//! Gemeinsame Hilfen der CLI-Integrationstests: eigener Datenordner (`OPENSPIDER_HOME`), Aufruf des Binaries
//! und Testsite mit Request-Log.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use os_testsite::{fixtures_root, SiteOptions, TestSite};

/// Ergebnis eines Aufrufs.
#[derive(Debug, Clone)]
pub struct Run {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Run {
    pub fn from_output(out: Output) -> Run {
        Run {
            code: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        }
    }

    /// Letzte Zeile auf stderr.
    pub fn last_line(&self) -> &str {
        self.stderr.lines().last().unwrap_or("")
    }

    /// Fehlermeldung mit beiden Strömen für `assert!`.
    pub fn dump(&self) -> String {
        format!(
            "exit {}\n--- stdout ---\n{}\n--- stderr ---\n{}",
            self.code, self.stdout, self.stderr
        )
    }
}

/// Isolierte Umgebung: eigener Datenordner und Arbeitsverzeichnis.
pub struct Env {
    pub home: tempfile::TempDir,
    pub work: tempfile::TempDir,
}

impl Env {
    pub fn new() -> Env {
        Env {
            home: tempfile::tempdir().unwrap(),
            work: tempfile::tempdir().unwrap(),
        }
    }

    pub fn command(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_openspider"));
        cmd.env("OPENSPIDER_HOME", self.home.path())
            // C13: Tests öffnen nie den Schlüsselbund des Betriebssystems.
            .env("OPENSPIDER_SECRET_STORE", "memory")
            .env_remove("OPENSPIDER_LOG")
            .env_remove("RUST_LOG")
            .current_dir(self.work.path())
            .stdin(Stdio::null());
        cmd
    }

    /// Führt das Binary aus (blockierend; in async-Tests über `spawn_blocking` aufrufen, `run_async`).
    pub fn run<S: AsRef<str>>(&self, args: &[S]) -> Run {
        let mut cmd = self.command();
        for a in args {
            cmd.arg(a.as_ref());
        }
        Run::from_output(cmd.output().expect("Binary startbar"))
    }

    /// Wie [`Env::run`], ohne die Laufzeit zu blockieren (die Testsite läuft auf ihr).
    pub async fn run_async<S: AsRef<str> + Send + 'static>(&self, args: Vec<S>) -> Run {
        let mut cmd = self.command();
        for a in &args {
            cmd.arg(a.as_ref());
        }
        tokio::task::spawn_blocking(move || {
            Run::from_output(cmd.output().expect("Binary startbar"))
        })
        .await
        .unwrap()
    }

    /// Crawl-Ordner der Anwendung.
    pub fn crawls_dir(&self) -> PathBuf {
        self.home.path().join("crawls")
    }

    /// Alle Crawl-Dateien.
    pub fn crawl_files(&self) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = std::fs::read_dir(self.crawls_dir())
            .map(|d| {
                d.filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|x| x == "ospider"))
                    .collect()
            })
            .unwrap_or_default();
        files.sort();
        files
    }

    /// Ausgabeordner im Arbeitsverzeichnis.
    pub fn out(&self) -> PathBuf {
        self.work.path().join("out")
    }
}

/// Startet ein Szenario mit Request-Log.
pub async fn site(name: &str) -> TestSite {
    TestSite::start_dir_with(
        fixtures_root().join(name),
        SiteOptions {
            port: 0,
            log_requests: true,
        },
    )
    .await
    .expect("Testsite startbar")
}

/// `--resolve`-Regel, die `*.test` auf die Testsite lenkt.
pub fn resolve(site: &TestSite) -> String {
    format!("*.test=127.0.0.1:{}", site.port())
}

/// Liest eine CSV-Datei (ohne BOM) als Zeilen von Feldern.
pub fn read_csv(path: &Path) -> Vec<Vec<String>> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let body = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(body.as_bytes());
    rdr.records()
        .map(|r| r.unwrap().iter().map(str::to_string).collect())
        .collect()
}

/// Werte einer Spalte (nach Kopfzeilenname) ohne Kopfzeile.
pub fn column(rows: &[Vec<String>], name: &str) -> Vec<String> {
    let idx = rows[0]
        .iter()
        .position(|h| h == name)
        .unwrap_or_else(|| panic!("Spalte {name} fehlt in {:?}", rows[0]));
    rows[1..].iter().map(|r| r[idx].clone()).collect()
}

/// Dateinamen eines Ordners, sortiert.
pub fn file_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|d| {
            d.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// Spalten, deren Werte von Lauf zu Lauf schwanken und für Goldens maskiert werden: Zeiten, und der Hash des
/// Bodys (die HTML-Seiten der Testsite enthalten ihre Basis-URL samt zufälligem Port).
const VOLATILE_COLUMNS: [&str; 3] = ["Response Time", "Crawl Timestamp", "Hash"];

/// Zeilen aus Überblicks-Reports, deren Wert von der Laufzeit abhängt.
const VOLATILE_ITEMS: [&str; 5] = [
    "Crawl Start",
    "Crawl End",
    "Elapsed Time",
    "Average Response Time (ms)",
    "Average URLs/s",
];

/// Maskiert flüchtige Werte, sortiert die Datenzeilen (die Reihenfolge hängt vom Zeitverlauf der Abrufe ab)
/// und gibt die Tabelle als CSV-Text mit `\n` zurück. Basis-URLs → `{BASE}`, Zeitstempel → `<TS>`.
pub fn golden_text(rows: &[Vec<String>], bases: &[&str]) -> String {
    let header = &rows[0];
    let volatile: Vec<usize> = header
        .iter()
        .enumerate()
        .filter(|(_, h)| VOLATILE_COLUMNS.contains(&h.as_str()))
        .map(|(i, _)| i)
        .collect();
    let item_col = header.iter().position(|h| h == "Item");
    let mut body: Vec<Vec<String>> = rows[1..]
        .iter()
        .map(|r| {
            let mut r = r.clone();
            for &i in &volatile {
                if i < r.len() {
                    r[i] = "<VAR>".to_string();
                }
            }
            if let Some(ic) = item_col {
                if r.len() > ic + 1 && VOLATILE_ITEMS.contains(&r[ic].as_str()) {
                    r[ic + 1] = "<VAR>".to_string();
                }
            }
            r
        })
        .collect();
    body.sort();
    let mut wtr = csv::WriterBuilder::new()
        .terminator(csv::Terminator::Any(b'\n'))
        .from_writer(Vec::new());
    wtr.write_record(header).unwrap();
    for r in &body {
        wtr.write_record(r).unwrap();
    }
    let text = String::from_utf8(wtr.into_inner().unwrap()).unwrap();
    os_testsite::golden::normalize(&text, bases)
}

/// Liest das erste Blatt einer XLSX-Datei als Zeilen von Textzellen.
pub fn read_xlsx(path: &Path) -> Vec<Vec<String>> {
    use calamine::{open_workbook, Reader, Xlsx};
    let mut book: Xlsx<_> =
        open_workbook(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let sheet = book.worksheet_range_at(0).unwrap().unwrap();
    sheet
        .rows()
        .map(|r| r.iter().map(|c| c.to_string()).collect())
        .collect()
}

/// Ist der Lauf `Completed`? Der Hinweis auf wenig freien Speicherplatz (Plattenwächter der Engine) hängt vom
/// Testrechner ab und zählt nicht.
pub fn is_completed(record: &os_sched::history::RunRecord) -> bool {
    use os_sched::history::RunResult;
    match record.result {
        RunResult::Completed => true,
        RunResult::CompletedWithWarnings => record
            .warnings
            .iter()
            .all(|w| w.contains("freier Speicherplatz")),
        _ => false,
    }
}
