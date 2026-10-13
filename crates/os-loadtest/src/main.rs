//! `os-loadtest run --scenario perf-001 --size small`

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use os_loadtest::runner::{self, ProbeParams};
use os_loadtest::{report, scenario, NoCrawler, Size, SCENARIOS};

#[derive(Parser)]
#[command(
    name = "os-loadtest",
    about = "Lasttest-Runner (PERF-009) für OpenSpider"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Führt ein Szenario aus und schreibt den Bericht.
    Run {
        /// Szenario, z. B. `perf-001` (siehe `list`).
        #[arg(long)]
        scenario: String,
        /// `small` (10.000 URLs, jede CI) oder `large` (1 Mio., vor Meilensteinen).
        #[arg(long, value_enum)]
        size: Size,
        /// Berichtsdatei (Standard: target/loadtest/report.json).
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Listet die Szenarien auf.
    List,
}

#[tokio::main]
async fn main() -> ExitCode {
    match Cli::parse().command {
        Command::List => {
            for s in SCENARIOS {
                println!(
                    "{:<9} {}  {} (small {}, large {} Seiten)",
                    s.name, s.perf_id, s.description, s.pages.0, s.pages.1
                );
            }
            ExitCode::SUCCESS
        }
        Command::Run {
            scenario: name,
            size,
            out,
        } => {
            let Some(def) = scenario::find(&name) else {
                eprintln!("Unbekanntes Szenario `{name}`; `os-loadtest list` zeigt alle.");
                return ExitCode::from(2);
            };
            let out = out.unwrap_or_else(report::default_report_path);
            let result = runner::run(def, size, ProbeParams::for_size(size), &NoCrawler).await;
            let rep = match result {
                Ok(rep) => rep,
                Err(e) => {
                    eprintln!("Lauf fehlgeschlagen: {e}");
                    return ExitCode::from(2);
                }
            };
            if let Err(e) = rep.write(&out) {
                eprintln!(
                    "Bericht konnte nicht geschrieben werden ({}): {e}",
                    out.display()
                );
                return ExitCode::from(2);
            }
            println!(
                "{} ({}): {:.0} Requests/s, passed = {} -> {}",
                rep.scenario,
                rep.size.as_str(),
                rep.metrics["server_requests_per_s"].as_f64().unwrap_or(0.0),
                rep.passed,
                out.canonicalize().unwrap_or(out).display()
            );
            if rep.passed {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
    }
}
