//! Startet einen Fixture-Server: `os-testsite <szenario> [port]`.

use os_testsite::TestSite;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let mut args = std::env::args().skip(1);
    let Some(scenario) = args.next() else {
        eprintln!("Aufruf: os-testsite <szenario> [port]");
        std::process::exit(2);
    };
    let port: u16 = args.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let dir = os_testsite::fixtures_root().join(&scenario);
    let site = TestSite::start_dir(dir, port).await?;
    println!("{} -> {}", scenario, site.base_url());
    tokio::signal::ctrl_c().await?;
    Ok(())
}
