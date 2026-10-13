//! Schlanker Starter für den MCP-Server, nur für die Prozess-Tests von P18/P27 (stdout-Reinheit,
//! rmcp-Client über Kindprozess). Ausgeliefert wird `openspider mcp` aus `os-cli`.

use clap::Parser;

#[derive(Parser)]
#[command(
    name = "openspider-mcp",
    version,
    about = "OpenSpider MCP-Server (Teststarter)"
)]
struct Cli {
    #[command(flatten)]
    args: os_mcp::cli::McpArgs,
}

fn main() {
    let cli = Cli::parse();
    std::process::exit(os_mcp::cli::run(cli.args));
}
