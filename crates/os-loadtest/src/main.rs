//! `os-loadtest run --scenario perf-001 --size small`

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use os_loadtest::child::{self, ChildArgs};
use os_loadtest::report::{self, now_rfc3339, Report, SuiteReport};
use os_loadtest::runner::{self, ProbeParams};
use os_loadtest::{scenario, CrawlerUnderTest, EngineCrawler, NoCrawler, Size, SCENARIOS};
use os_store::StorageProfile;

#[derive(Parser)]
#[command(
    name = "os-loadtest",
    about = "Lasttest-Runner (PERF-009) für OpenSpider"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, ValueEnum)]
enum CrawlerChoice {
    /// os-engine im Kindprozess (Standard).
    Engine,
    /// Nur den Testserver messen.
    None,
}

#[derive(Clone, Copy, ValueEnum)]
enum ProfileChoice {
    Balanced,
    MaxSpeed,
    MinMemory,
}

impl From<ProfileChoice> for StorageProfile {
    fn from(p: ProfileChoice) -> Self {
        match p {
            ProfileChoice::Balanced => StorageProfile::Balanced,
            ProfileChoice::MaxSpeed => StorageProfile::MaxSpeed,
            ProfileChoice::MinMemory => StorageProfile::MinMemory,
        }
    }
}

#[derive(Subcommand)]
enum Command {
    /// Führt Szenarien aus und schreibt den Bericht.
    Run {
        /// Szenario, z. B. `perf-001` (siehe `list`); mehrfach angebbar, `all` = alle.
        #[arg(long, required = true)]
        scenario: Vec<String>,
        /// `small` (10.000 URLs, jede CI) oder `large` (1 Mio., vor Meilensteinen).
        #[arg(long, value_enum)]
        size: Size,
        /// Berichtsdatei (Standard: target/loadtest/report.json).
        #[arg(long)]
        out: Option<PathBuf>,
        /// Welcher Crawler gemessen wird.
        #[arg(long, value_enum, default_value = "engine")]
        crawler: CrawlerChoice,
        /// Parallele Abrufe des Crawlers.
        #[arg(long, default_value_t = os_loadtest::engine::DEFAULT_THREADS)]
        threads: u32,
        /// Speicherprofil der Crawl-Dateien (CRAWL-124).
        #[arg(long, value_enum, default_value = "balanced")]
        profile: ProfileChoice,
        /// Seed der Abbruchzeitpunkte (PERF-005).
        #[arg(long, default_value_t = 20_261_014)]
        seed: u64,
        /// Anzahl harter Abbrüche (PERF-005; Standard 3 bei small, 5 bei large).
        #[arg(long)]
        kills: Option<usize>,
        /// Seitenzahl der synthetischen Site statt der Voreinstellung der Größe (z. B. 100000 für die
        /// Profilvergleiche aus CRAWL-124).
        #[arg(long)]
        pages: Option<u64>,
        /// Ordner für die Crawl-Dateien (Standard: temporärer Ordner; 1-Mio.-Crawls brauchen mehrere GB).
        #[arg(long)]
        work_dir: Option<PathBuf>,
    },
    /// Listet die Szenarien auf.
    List,
    /// Sondenmessung für den Durchsatz (Timer, fsync, Verbindungsaufbau, HTTP ohne Engine, Crawl mit Messzählern).
    Diag {
        /// Seiten der Site für `http` und `crawl`.
        #[arg(long, default_value_t = 3_000)]
        pages: u64,
        /// Parallele Abrufe.
        #[arg(long, default_value_t = os_loadtest::engine::DEFAULT_THREADS)]
        threads: u32,
        /// Nur diese Sonden (`timers`, `fsync`, `connect`, `http`, `crawl`); mehrfach angebbar. Standard: alle.
        #[arg(long)]
        only: Vec<String>,
        /// Speicherprofil des Crawls.
        #[arg(long, value_enum, default_value = "balanced")]
        profile: ProfileChoice,
        /// `PRAGMA wal_autocheckpoint` des Crawls in Seiten (Standard: SQLite-Standard).
        #[arg(long)]
        wal_autocheckpoint: Option<u32>,
        /// Ordner für Dateien der Sonden (Standard: temporärer Ordner).
        #[arg(long)]
        work_dir: Option<PathBuf>,
        /// Ergebnisdatei (JSON).
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Kindprozess eines Crawls (intern; vom Runner gestartet, damit der Spitzen-RSS gemessen werden kann).
    #[command(hide = true)]
    CrawlChild(ChildCli),
}

#[derive(Args)]
struct ChildCli {
    #[arg(long)]
    file: PathBuf,
    #[arg(long)]
    start: String,
    #[arg(long)]
    resolve: Vec<String>,
    #[arg(long, default_value_t = os_loadtest::engine::DEFAULT_THREADS)]
    threads: u32,
    #[arg(long, default_value = "balanced")]
    profile: String,
    #[arg(long)]
    slow_sink_ms: Option<u64>,
    #[arg(long)]
    wal_autocheckpoint: Option<u32>,
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
        Command::Diag {
            pages,
            threads,
            only,
            profile,
            wal_autocheckpoint,
            work_dir,
            out,
        } => {
            if let Some(bad) = only
                .iter()
                .find(|o| !os_loadtest::diag::PROBES.contains(&o.as_str()))
            {
                eprintln!(
                    "Unbekannte Sonde `{bad}` (erlaubt: {}).",
                    os_loadtest::diag::PROBES.join(", ")
                );
                return ExitCode::from(2);
            }
            let report = os_loadtest::diag::run(&os_loadtest::diag::DiagOptions {
                pages,
                threads,
                work_dir,
                only,
                profile: profile.into(),
                wal_autocheckpoint,
            })
            .await;
            let text = serde_json::to_string_pretty(&report).unwrap_or_default();
            println!("{text}");
            if let Some(path) = out {
                if let Err(e) = std::fs::write(&path, &text) {
                    eprintln!("{}: {e}", path.display());
                    return ExitCode::from(2);
                }
            }
            ExitCode::SUCCESS
        }
        Command::CrawlChild(c) => {
            let profile = match child::parse_profile(&c.profile) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::from(2);
                }
            };
            match child::run(ChildArgs {
                file: c.file,
                start: c.start,
                resolve: c.resolve,
                threads: c.threads,
                profile,
                slow_sink_ms: c.slow_sink_ms,
                wal_autocheckpoint: c.wal_autocheckpoint,
            })
            .await
            {
                Ok(0) => ExitCode::SUCCESS,
                Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Run {
            scenario: names,
            size,
            out,
            crawler,
            threads,
            profile,
            seed,
            kills,
            pages,
            work_dir,
        } => {
            let mut defs = Vec::new();
            for name in &names {
                if name.eq_ignore_ascii_case("all") {
                    defs.extend(SCENARIOS.iter());
                } else if let Some(def) = scenario::find(name) {
                    defs.push(def);
                } else {
                    eprintln!("Unbekanntes Szenario `{name}`; `os-loadtest list` zeigt alle.");
                    return ExitCode::from(2);
                }
            }
            let engine;
            let under_test: &dyn CrawlerUnderTest = match crawler {
                CrawlerChoice::None => &NoCrawler,
                CrawlerChoice::Engine => {
                    let exe = match std::env::current_exe() {
                        Ok(p) => p,
                        Err(e) => {
                            eprintln!("Programmpfad unbekannt: {e}");
                            return ExitCode::from(2);
                        }
                    };
                    let mut c = EngineCrawler::new(exe);
                    c.threads = threads;
                    c.profile = profile.into();
                    c.seed = seed;
                    c.kills = kills;
                    c.work_dir = work_dir;
                    engine = c;
                    &engine
                }
            };
            let out = out.unwrap_or_else(report::default_report_path);
            let started_at = now_rfc3339();
            let mut reports: Vec<Report> = Vec::new();
            for def in defs {
                let mut def = *def;
                if let Some(n) = pages {
                    def.pages = (n, n);
                }
                match runner::run(&def, size, ProbeParams::for_size(size), under_test).await {
                    Ok(rep) => {
                        print_summary(&rep);
                        reports.push(rep);
                    }
                    Err(e) => {
                        eprintln!("Lauf {} fehlgeschlagen: {e}", def.name);
                        return ExitCode::from(2);
                    }
                }
            }
            let (written, passed) = if names.len() == 1 && !names[0].eq_ignore_ascii_case("all") {
                let rep = &reports[0];
                (rep.write(&out), rep.passed)
            } else {
                let suite = SuiteReport::new(size, started_at, reports);
                (suite.write(&out), suite.passed)
            };
            if let Err(e) = written {
                eprintln!(
                    "Bericht konnte nicht geschrieben werden ({}): {e}",
                    out.display()
                );
                return ExitCode::from(2);
            }
            println!("Bericht: {}", out.canonicalize().unwrap_or(out).display());
            if passed {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
    }
}

fn print_summary(rep: &Report) {
    println!(
        "{} ({}): {:.0} Requests/s, Crawler {}, passed = {}",
        rep.scenario,
        rep.size.as_str(),
        rep.metrics["server_requests_per_s"].as_f64().unwrap_or(0.0),
        rep.metrics
            .get("crawler_available")
            .and_then(|v| v.as_bool())
            .map_or("?", |a| if a { "ja" } else { "nicht angebunden" }),
        rep.passed
    );
    if let Some(rss) = rep
        .metrics
        .get("crawler_peak_rss_bytes")
        .and_then(|v| v.as_u64())
    {
        println!("  Spitzen-RSS des Crawl-Prozesses: {} MB", rss / 1_000_000);
    }
    if let Some(segs) = rep
        .metrics
        .get("crawler_segment_urls_per_s")
        .and_then(|v| v.as_array())
    {
        let list: Vec<String> = segs
            .iter()
            .filter_map(|v| v.as_f64())
            .map(|v| format!("{v:.0}"))
            .collect();
        println!(
            "  PERF-002: {} URLs/s im Mittel, Abschnitte [{}] URLs/s",
            rep.metrics
                .get("crawler_urls_per_s")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            list.join(", ")
        );
    }
    if let Some(v) = rep
        .metrics
        .get("crawler_violations")
        .and_then(|v| v.as_array())
    {
        for violation in v {
            println!("  VERLETZT: {violation}");
        }
    }
    if let Some(err) = rep.metrics.get("crawler_error") {
        println!("  FEHLER: {err}");
    }
}
