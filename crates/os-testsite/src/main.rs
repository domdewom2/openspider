//! Startet einen Fixture-Server.
//!
//! ```text
//! os-testsite <szenario | synthetic:n=1000000,links=30> [port] [--log-requests] [--ca-cert <datei>]
//! ```
//!
//! `--log-requests` zählt Requests je Pfad; beim Beenden (Ctrl+C) werden Duplikat-Abrufe
//! ausgegeben, während des Laufs liefert `/__testsite/requests` die Zähler als JSON.
//! `--ca-cert <datei>` schreibt die Test-CA (PEM) in die Datei, damit ein Client die
//! HTTPS-Antworten (gleicher Port) prüfen kann.

use os_testsite::{SiteOptions, TestSite};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let mut positional = Vec::new();
    let mut log_requests = false;
    let mut ca_cert: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--log-requests" => log_requests = true,
            "--ca-cert" => ca_cert = args.next(),
            _ => positional.push(arg),
        }
    }
    let Some(scenario) = positional.first().cloned() else {
        eprintln!(
            "Aufruf: os-testsite <szenario | synthetic:n=...,links=...> [port] [--log-requests] [--ca-cert <datei>]"
        );
        std::process::exit(2);
    };
    let port: u16 = positional.get(1).and_then(|p| p.parse().ok()).unwrap_or(0);
    let site = TestSite::start_spec(&scenario, SiteOptions { port, log_requests }).await?;
    println!("{} -> {}", scenario, site.base_url());
    if let Some(path) = ca_cert {
        std::fs::write(path, site.ca_cert_pem())?;
    }
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
