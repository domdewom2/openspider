//! `os-cli` – Kommandozeile `openspider`: headless Crawl, Export, `openspider mcp`, Scheduling-Ziel.
//!
//! Ablauf eines Aufrufs (OUT-180 ff.):
//!
//! 1. [`main_entry`] repariert den Windows-Fallstrick `"C:\out\"` (OUT-190), prüft die Namen aller langen
//!    Optionen und lässt dann `clap` lesen. Jeder Fehler hier ist ein Aufruffehler (Exit-Code 2).
//! 2. [`run`] bedient `--help <thema>`, `--list-exports`, `--list-reports`, `--list-crawls` und den
//!    Unterbefehl `mcp` ohne Crawl; für Läufe baut [`plan::build`] den Ablaufplan. **Bis dahin gibt es keinen
//!    Netzzugriff** (OUT-203).
//! 3. [`exec::execute`] crawlt (oder lädt), startet die Crawl-Analyse über den Engine-Hook (CRAWL-141), führt
//!    die Exporte aus und speichert.
//!
//! stdout trägt nur Nutzdaten; Hinweise, Warnungen, Fortschritt und die Schlusszeile gehen nach stderr. Die
//! letzte stderr-Zeile fasst Ergebnis und Exit-Code zusammen (OUT-204).

pub mod args;
pub mod exec;
pub mod exit;
pub mod help;
pub mod listing;
pub mod logging;
pub mod plan;
pub mod registry;
pub mod reporter;
pub mod signals;

use std::ffi::OsString;
use std::io::Write;
use std::sync::Arc;

use clap::{CommandFactory, Parser};

pub use args::{Cli, RunArgs, Sub, VERSION};
pub use exit::{CliError, Exit};

use os_analyze::analysis::CancelToken;
use os_store::paths::app_paths;
use reporter::Reporter;

/// Schreibt einen Text auf stdout; ein geschlossenes stdout (z. B. `| head`) beendet den Lauf nicht.
fn print_stdout(text: &str) {
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(text.as_bytes());
    let _ = out.flush();
}

/// Schlusszeile auf stderr: Ergebnis und Exit-Code (OUT-204). Es folgt nichts mehr danach.
fn finish(summary: &str, exit: Exit) -> i32 {
    let mut err = std::io::stderr().lock();
    let _ = writeln!(err, "openspider: {summary} (Exit-Code {})", exit.code());
    let _ = err.flush();
    exit.code()
}

/// Meldung eines Aufruffehlers samt Schlusszeile.
fn fail(error: &CliError) -> i32 {
    let mut err = std::io::stderr().lock();
    let _ = writeln!(err, "Fehler: {}", error.message);
    drop(err);
    finish(error.headline(), error.exit)
}

/// Verbindet das Protokoll (`tracing`) mit stderr; jede Zeile läuft durch `os_fetch::redact::text`, damit
/// weder Zugangsdaten noch Tokens im Log landen. Stufe über `OPENSPIDER_LOG` (Standard `error`: Warnungen
/// der Bibliotheken meldet der Lauf selbst über seine Ergebnisse, damit nichts doppelt erscheint).
pub(crate) fn init_logging() {
    use tracing_subscriber::fmt::MakeWriter;
    use tracing_subscriber::EnvFilter;

    struct Redacting;
    struct RedactingWriter;

    impl Write for RedactingWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let text = String::from_utf8_lossy(buf);
            let clean = os_fetch::redact::text(&text);
            let mut err = std::io::stderr().lock();
            let _ = err.write_all(clean.as_bytes());
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for Redacting {
        type Writer = RedactingWriter;

        fn make_writer(&'a self) -> Self::Writer {
            RedactingWriter
        }
    }

    let filter =
        EnvFilter::try_from_env("OPENSPIDER_LOG").unwrap_or_else(|_| EnvFilter::new("error"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(Redacting)
        .with_ansi(false)
        .try_init();
}

/// Prüft die langen Optionen vor `clap`: Unbekannte Namen bekommen einen Vorschlag mit Levenshtein ≤ 3
/// (OUT-203, z. B. `--crawl-lst` → „meinten Sie `--crawl-list`?“).
fn check_option_names(args: &[String]) -> Result<(), CliError> {
    if args.first().map(String::as_str) == Some("mcp") {
        return Ok(());
    }
    let known = args::long_option_names();
    for arg in args {
        if arg == "--" {
            break;
        }
        let Some(rest) = arg.strip_prefix("--") else {
            continue;
        };
        let name = format!("--{}", rest.split('=').next().unwrap_or(rest));
        if known.contains(&name) {
            continue;
        }
        let mut near: Vec<(usize, &String)> = known
            .iter()
            .map(|k| (os_export::catalog::bulk_links::levenshtein(&name, k), k))
            .filter(|(d, _)| *d <= 3)
            .collect();
        near.sort();
        return Err(CliError::usage(match near.first() {
            Some((_, best)) => format!("unbekannte Option '{name}' – meinten Sie `{best}`?"),
            None => {
                format!("unbekannte Option '{name}' – `openspider --help` listet alle Optionen")
            }
        }));
    }
    Ok(())
}

/// Parser des Unterbefehls `mcp` (ohne die Sonderbehandlung von `--help` im Hauptprogramm).
#[derive(Debug, Parser)]
#[command(name = "openspider mcp", version = VERSION, about = "Startet den MCP-Server (STDIO) für KI-Agenten")]
struct McpCli {
    #[command(flatten)]
    args: os_mcp::cli::McpArgs,
}

fn run_mcp(rest: &[String]) -> i32 {
    let mut full = vec!["openspider mcp".to_string()];
    full.extend(rest.iter().cloned());
    match McpCli::try_parse_from(full) {
        Ok(cli) => os_mcp::cli::run(cli.args),
        Err(e) => {
            let ok = matches!(
                e.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            );
            let _ = e.print();
            if ok {
                0
            } else {
                Exit::Usage.code()
            }
        }
    }
}

/// Einstieg für `main`: liest die Argumente, führt den Befehl aus und liefert den Exit-Code.
pub fn main_entry<I, T>(argv: I) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let raw: Vec<String> = argv
        .into_iter()
        .map(|a| a.into().to_string_lossy().into_owned())
        .collect();
    let (program, rest) = match raw.split_first() {
        Some((p, r)) => (p.clone(), r.to_vec()),
        None => ("openspider".to_string(), Vec::new()),
    };
    // `openspider mcp …` ist eine reine Durchreichung an os-mcp (eigene Hilfe, eigene Fehlermeldungen).
    if rest.first().map(String::as_str) == Some("mcp") {
        return run_mcp(&rest[1..]);
    }
    let (rest, touched) = args::fix_trailing_backslash_quote(rest);
    for option in &touched {
        eprintln!(
            "Hinweis: Der Wert von {option} endete mit einem Backslash vor dem Anführungszeichen \
             (\"C:\\ordner\\\") – er wurde korrigiert; besser ohne abschließenden Backslash schreiben"
        );
    }
    if let Err(e) = check_option_names(&rest) {
        return fail(&e);
    }
    let mut full = vec![program];
    full.extend(rest);
    match Cli::try_parse_from(full) {
        Ok(cli) => run(cli),
        Err(e) => {
            use clap::error::ErrorKind;
            if matches!(e.kind(), ErrorKind::DisplayVersion | ErrorKind::DisplayHelp) {
                // `--version` (und Hilfe von Unterbefehlen) auf stdout, Exit-Code 0.
                let _ = e.print();
                return 0;
            }
            let _ = e.print();
            let first = e.to_string();
            let headline = first
                .lines()
                .next()
                .unwrap_or("ungültiger Aufruf")
                .trim_start_matches("error: ")
                .to_string();
            finish(&headline, Exit::Usage)
        }
    }
}

/// Führt den Befehl aus und gibt den Exit-Code zurück.
pub fn run(cli: Cli) -> i32 {
    if let Some(Sub::Mcp(args)) = cli.command {
        return os_mcp::cli::run(args);
    }
    let args = cli.run;

    // Hilfe und Auflistungen: nur stdout, kein Crawl, kein Netz.
    if let Some(topic) = &args.help {
        match topic {
            None => {
                print_stdout(&help::general());
                return 0;
            }
            Some(name) => {
                return match help::topic(name, args.json) {
                    Ok(text) => {
                        print_stdout(&text);
                        0
                    }
                    Err(e) => fail(&e),
                };
            }
        }
    }
    if args.list_exports || args.list_reports {
        if args.list_exports {
            print_stdout(&help::list_exports(args.json));
        }
        if args.list_reports {
            print_stdout(&help::list_reports(args.json));
        }
        return 0;
    }

    logging::init(&args);
    let paths = app_paths();

    if args.list_crawls {
        return match listing::collect(&paths.crawls, args.project_name.as_deref()) {
            Ok((rows, skipped)) => {
                print_stdout(&if args.json {
                    listing::json(&rows)
                } else {
                    listing::table(&rows)
                });
                for (path, why) in &skipped {
                    eprintln!("Warnung: {path} nicht lesbar: {why}");
                }
                finish(&format!("{} Crawl(s) aufgelistet", rows.len()), Exit::Ok)
            }
            Err(e) => fail(&e),
        };
    }

    let rep = Reporter::new(args.json_progress);
    rep.phase("validate");
    let plan = match plan::build(&args, paths, &rep) {
        Ok(plan) => plan,
        Err(e) => return fail(&e),
    };

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => return fail(&CliError::internal(format!("Laufzeit nicht startbar: {e}"))),
    };
    let outcome = runtime.block_on(async {
        let signals = match signals::Signals::install(CancelToken::new(), Arc::clone(&rep)) {
            Ok(s) => s,
            Err(e) => {
                return exec::Outcome {
                    exit: Exit::Internal,
                    summary: format!("Signalbehandlung nicht installierbar: {e}"),
                }
            }
        };
        exec::execute(plan, Arc::clone(&rep), signals).await
    });
    // Offene Hintergrundaufgaben (Signalwächter) nicht abwarten.
    runtime.shutdown_background();
    finish(&outcome.summary, outcome.exit)
}

/// Hilfe für Tests: die Befehlsbeschreibung.
pub fn command() -> clap::Command {
    Cli::command()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_definition_is_consistent() {
        Cli::command().debug_assert();
    }

    // spec: OUT-203
    #[test]
    fn unknown_options_get_a_suggestion() {
        let err = check_option_names(&["--crawl-lst".to_string(), "x".to_string()]).unwrap_err();
        assert_eq!(err.exit, Exit::Usage);
        assert!(
            err.message.contains("meinten Sie `--crawl-list`?"),
            "{}",
            err.message
        );
        let far = check_option_names(&["--zzzzzzzzzzzzzzzzzzzz".to_string()]).unwrap_err();
        assert!(far.message.contains("--help"));
        assert!(check_option_names(&["--crawl=http://a.test/".to_string()]).is_ok());
    }

    #[test]
    fn parses_mcp_subcommand() {
        let cli = Cli::try_parse_from(["openspider", "mcp", "--read-only"]).unwrap();
        let Some(Sub::Mcp(args)) = cli.command else {
            panic!("mcp erwartet");
        };
        assert!(args.read_only);
    }
}
