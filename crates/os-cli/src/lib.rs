//! `os-cli` – Kommandozeile `openspider`: headless Crawl, Export, Scheduling-Ziel.

use clap::{Parser, Subcommand};

/// OpenSpider – Open-Source-SEO-Crawler.
#[derive(Debug, Parser)]
#[command(name = "openspider", version, about)]
pub struct Cli {
    /// Auszuführender Unterbefehl.
    #[command(subcommand)]
    pub command: Command,
}

/// Unterbefehle von `openspider`.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Crawlt eine Website ab der angegebenen URL (derzeit nur ein Stub).
    Crawl {
        /// Start-URL des Crawls.
        url: String,
    },
}

/// Führt den Befehl aus und gibt den Exit-Code zurück.
pub fn run(cli: Cli) -> i32 {
    match cli.command {
        Command::Crawl { url } => {
            eprintln!("openspider crawl: noch nicht implementiert (Start-URL: {url})");
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_crawl_subcommand() {
        let cli = Cli::try_parse_from(["openspider", "crawl", "https://example.com/"]).unwrap();
        let Command::Crawl { url } = cli.command;
        assert_eq!(url, "https://example.com/");
    }
}
