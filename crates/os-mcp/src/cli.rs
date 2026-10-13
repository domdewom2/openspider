//! Kommandozeile `openspider mcp` (und der schlanke Starter `openspider-mcp` für die Prozess-Tests).
//!
//! STUB(M1-P18): P18 setzt `run` um (Server, Freigaben, `--print-config`).

use std::path::PathBuf;

/// Argumente von `openspider mcp`.
#[derive(Debug, Clone, Default, clap::Args)]
pub struct McpArgs {
    /// Ordner mit den Crawl-Dateien (Standard: der Crawl-Ordner der Anwendung).
    #[arg(long, value_name = "ORDNER")]
    pub crawl_dir: Option<PathBuf>,

    /// Freigegebene Werkzeugklassen (read, crawl, write, cost), kommagetrennt.
    #[arg(long, value_delimiter = ',', value_name = "KLASSEN")]
    pub allow: Vec<String>,

    /// Nur lesende Werkzeuge (überstimmt die Einstellungsdatei).
    #[arg(long)]
    pub read_only: bool,

    /// Gibt die Konfiguration für einen MCP-Client aus und beendet sich.
    #[arg(
        long,
        value_name = "CLIENT",
        value_parser = ["claude-desktop", "claude-code", "codex", "cursor"]
    )]
    pub print_config: Option<String>,
}

/// Startet den MCP-Server; liefert den Exit-Code.
// STUB(M1-P18)
pub fn run(_args: McpArgs) -> i32 {
    eprintln!("openspider mcp: noch nicht implementiert (STUB(M1-P18))");
    1
}
