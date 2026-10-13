//! Startet einen Fixture-Server.
//!
//! ```text
//! os-testsite <szenario | synthetic:n=1000000,links=30> [port] [--log-requests]
//! ```
//!
//! `--log-requests` zählt Requests je Pfad; beim Beenden (Ctrl+C) werden Duplikat-Abrufe
//! ausgegeben, während des Laufs liefert `/__testsite/requests` die Zähler als JSON.

use os_testsite::{SiteOptions, TestSite};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let mut positional = Vec::new();
    let mut log_requests = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--log-requests" => log_requests = true,
            _ => positional.push(arg),
        }
    }
    let Some(scenario) = positional.first().cloned() else {
        eprintln!(
            "Aufruf: os-testsite <szenario | synthetic:n=...,links=...> [port] [--log-requests]"
        );
        std::process::exit(2);
    };
    let port: u16 = positional.get(1).and_then(|p| p.parse().ok()).unwrap_or(0);
    let site = TestSite::start_spec(&scenario, SiteOptions { port, log_requests }).await?;
    println!("{} -> {}", scenario, site.base_url());
    tokio::signal::ctrl_c().await?;
    if log_requests {
        let dups = site.duplicate_requests();
        println!(
            "{} Requests, {} Pfade mehrfach abgerufen",
            site.total_requests(),
            dups.len()
        );
        for (path, count) in dups.iter().take(20) {
            println!("  {count}x {path}");
        }
    }
    Ok(())
}
