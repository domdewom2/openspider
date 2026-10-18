use std::path::PathBuf;

use clap::Parser;

use super::{generate, Report, Result, DEFAULT_ROWS, DEFAULT_SEED};

#[derive(Debug, Parser)]
#[command(
    name = "gen-ui-large",
    about = "Deterministische Crawl-Datei für UI-Benchmarks (UI-148)"
)]
pub struct Args {
    /// Neue Zieldatei; vorhandene Dateien werden nicht überschrieben.
    #[arg(long, value_name = "FILE.ospider")]
    pub output: PathBuf,
    /// Interne HTML-Seiten (22 bis 5000000), jeweils mit 20 Hyperlinks.
    #[arg(long, default_value_t = DEFAULT_ROWS, value_parser = clap::value_parser!(u64).range(22..=DEFAULT_ROWS))]
    pub rows: u64,
    /// Fester Seed für Spaltenwerte und Crawl-ID.
    #[arg(long, default_value_t = DEFAULT_SEED)]
    pub seed: u64,
}

impl Args {
    pub fn run(self) -> Result<Report> {
        generate(&self.output, self.rows, self.seed, |phase, n| {
            if phase != "rows" || n % 100_000 == 0 || n == self.rows {
                eprintln!("gen-ui-large: {phase} {n}/{}", self.rows);
            }
        })
    }
}
