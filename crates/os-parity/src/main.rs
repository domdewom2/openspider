//! `os-parity` – Paritätsabgleich und Spec-Abdeckung.

use clap::{Parser, Subcommand};

/// OpenSpider-Paritätswerkzeug.
#[derive(Debug, Parser)]
#[command(name = "os-parity", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Vergleicht zwei Exporte anhand eines Spaltenmappings.
    Diff(os_parity::diff::DiffArgs),
    /// Erzeugt die Exporte einer Fixture-Site und vergleicht sie mit den Referenzen.
    Run(os_parity::run::RunArgs),
    /// Ermittelt den Umsetzungsstand je Spec-ID.
    Coverage(os_parity::coverage::CoverageArgs),
    /// Test-Hilfsbefehl: gibt sich als `openspider` aus (nur für die Tests von `run`).
    #[command(hide = true)]
    FakeOpenspider(FakeArgs),
}

#[derive(Debug, clap::Args)]
struct FakeArgs {
    /// Die Kommandozeile, die `openspider` erhalten hätte (nach `--`).
    #[arg(last = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

fn main() {
    let cli = Cli::parse();
    let result = match &cli.command {
        Command::Diff(args) => os_parity::diff::run(args),
        Command::Run(args) => os_parity::run::run(args),
        Command::Coverage(args) => os_parity::coverage::run(args),
        Command::FakeOpenspider(fake) => std::process::exit(os_parity::fake::run(&fake.args)),
    };
    match result {
        Ok(true) => {}
        Ok(false) => std::process::exit(1),
        Err(e) => {
            eprintln!("os-parity: {e}");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_consistent() {
        Cli::command().debug_assert();
    }
}
