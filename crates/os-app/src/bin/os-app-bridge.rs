//! E2E-Bridge (Feature `e2e-bridge`): WebSocket auf Loopback mit Token, Protokoll in `docs/plan/M2.md` 3.6.
//!
//! ```text
//! os-app-bridge --home <ordner> [--fixture fx-ui-basic] [--stub-views]
//! ```
//!
//! Gibt nach dem Start auf stdout eine Zeile `{"port":<n>,"token":"<hex>"}` aus und läuft, bis das Signal zum Beenden
//! (Strg+C, SIGTERM) kommt. Alle Daten liegen unter `--home`; Geheimnisse liegen immer im Arbeitsspeicher (C13).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use os_app::runtime::bridge;
use os_app::{App, AppConfig};

struct Args {
    home: PathBuf,
    fixture: Option<String>,
    stub_views: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut home = None;
    let mut fixture = None;
    let mut stub_views = false;
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--home" => {
                home = Some(PathBuf::from(
                    it.next().ok_or("--home braucht einen Ordner")?,
                ))
            }
            "--fixture" => fixture = Some(it.next().ok_or("--fixture braucht einen Namen")?),
            "--stub-views" => stub_views = true,
            "-h" | "--help" => {
                return Err(
                    "os-app-bridge --home <ordner> [--fixture <name>] [--stub-views]".into(),
                )
            }
            other => return Err(format!("unbekannte Option {other}")),
        }
    }
    let home = home.ok_or("--home <ordner> fehlt")?;
    Ok(Args {
        home,
        fixture,
        stub_views,
    })
}

/// Kopiert die Crawl-Dateien der Fixture (`tests/fixtures/ui/<name>/*.ospider`) in den Crawl-Ordner. Fehlt die
/// Fixture, gibt es eine Warnung auf stderr und der Start geht ohne Daten weiter.
fn install_fixture(name: &str, crawls_dir: &Path) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/ui");
    let dir = root.join(name);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        eprintln!(
            "os-app-bridge: Fixture {name} nicht gefunden ({})",
            dir.display()
        );
        return;
    };
    let _ = std::fs::create_dir_all(crawls_dir);
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x == "ospider") {
            if let Some(file) = p.file_name() {
                if let Err(err) = std::fs::copy(&p, crawls_dir.join(file)) {
                    eprintln!("os-app-bridge: {} nicht kopierbar: {err}", p.display());
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::from(2);
        }
    };
    if args.stub_views && !cfg!(feature = "stub-views") {
        eprintln!("os-app-bridge: --stub-views verlangt einen Build mit dem Feature stub-views");
        return ExitCode::from(2);
    }
    let mut config = AppConfig::new(&args.home);
    config.stub_views = args.stub_views;
    if let Some(name) = &args.fixture {
        install_fixture(name, &config.paths.crawls);
    }
    let app = match App::new(config) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("os-app-bridge: Start fehlgeschlagen: {e}");
            return ExitCode::from(1);
        }
    };
    let handle = match bridge::start(app).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("os-app-bridge: Port nicht bindbar: {e}");
            return ExitCode::from(1);
        }
    };
    println!("{}", handle.startup_line());
    wait_for_signal().await;
    handle.shutdown().await;
    ExitCode::SUCCESS
}

#[cfg(unix)]
async fn wait_for_signal() {
    use tokio::signal::unix::{signal, SignalKind};
    let Ok(mut term) = signal(SignalKind::terminate()) else {
        let _ = tokio::signal::ctrl_c().await;
        return;
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = term.recv() => {}
    }
}

#[cfg(not(unix))]
async fn wait_for_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
