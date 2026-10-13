//! `os-cli` – Kommandozeile `openspider`: headless Crawl, Export, `openspider mcp`, Scheduling-Ziel.
//!
//! Gerüst (P00b): `Cli { command: Option<Sub>, run: RunArgs }`. Die SF-kompatiblen Flags, Validierung,
//! Exit-Codes, Exporte und Signalbehandlung setzt M1-P26 um; nach dem Crawl übergibt P26 an
//! `EngineDeps.on_finished` den Aufruf `os_analyze::analysis::run_after_crawl` (CRAWL-141).

use clap::{CommandFactory, Parser, Subcommand};

/// OpenSpider – Open-Source-SEO-Crawler.
#[derive(Debug, Parser)]
#[command(name = "openspider", version, about)]
pub struct Cli {
    /// Unterbefehl (ohne Unterbefehl gelten die Optionen des Headless-Betriebs).
    #[command(subcommand)]
    pub command: Option<Sub>,

    /// Optionen des Headless-Betriebs.
    #[command(flatten)]
    pub run: RunArgs,
}

/// Unterbefehle von `openspider`.
#[derive(Debug, Subcommand)]
pub enum Sub {
    /// Startet den MCP-Server (STDIO) für KI-Agenten.
    Mcp(os_mcp::cli::McpArgs),
}

/// Optionen des Headless-Betriebs (Screaming-Frog-kompatibel, Umfang siehe M1-P26).
#[derive(Debug, Clone, Default, clap::Args)]
pub struct RunArgs {
    /// Crawlt eine Website ab der angegebenen URL (derzeit nur ein Stub).
    #[arg(long, value_name = "URL")]
    pub crawl: Option<String>,
}

/// Führt den Befehl aus und gibt den Exit-Code zurück.
pub fn run(cli: Cli) -> i32 {
    match cli.command {
        Some(Sub::Mcp(args)) => os_mcp::cli::run(args),
        None => match cli.run.crawl {
            Some(url) => {
                eprintln!("openspider --crawl: noch nicht implementiert (Start-URL: {url})");
                0
            }
            None => {
                let _ = Cli::command().print_help();
                2
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_crawl_flag() {
        let cli = Cli::try_parse_from(["openspider", "--crawl", "https://example.com/"]).unwrap();
        assert!(cli.command.is_none());
        assert_eq!(cli.run.crawl.as_deref(), Some("https://example.com/"));
    }

    #[test]
    fn parses_mcp_subcommand() {
        let cli = Cli::try_parse_from(["openspider", "mcp", "--read-only"]).unwrap();
        let Some(Sub::Mcp(args)) = cli.command else {
            panic!("mcp erwartet");
        };
        assert!(args.read_only);
    }

    #[test]
    fn cli_definition_is_consistent() {
        Cli::command().debug_assert();
    }
}
