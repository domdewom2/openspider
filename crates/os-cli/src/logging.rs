//! CLI log setup and diagnostics (OUT-240 … OUT-244).
// M2-N01 interface request (C7): move these Tauri-free primitives to a common
// crate and remove this temporary source inclusion; keep diag.rs free of app types.
/// Absturzberichte (OUT-246, M3-C01): gemeinsamer Teil mit der App, wie `diagnostics` per Quelldatei eingebunden.
#[doc(hidden)]
#[path = "../../os-app/src/crash/shared.rs"]
pub mod crash;
#[doc(hidden)]
#[path = "../../os-app/src/diag.rs"]
pub mod diagnostics;
use std::time::SystemTime;
use tracing_subscriber::{prelude::*, EnvFilter};

pub fn init(args: &crate::args::RunArgs) {
    let paths = os_store::paths::app_paths();
    let log_path = paths.logs.join(format!(
        "cli-{}-{}.log",
        diagnostics::file_stamp(),
        std::process::id()
    ));
    let file = diagnostics::LogFile::new(log_path.clone(), false);
    let directives = if args.quiet {
        "error".into()
    } else {
        args.log_level
            .clone()
            .or_else(|| std::env::var("OPENSPIDER_LOG").ok())
            .unwrap_or_else(|| "info".into())
    };
    let directives = if args.quiet {
        directives
    } else {
        format!("info,{directives}")
    };
    let filter = EnvFilter::try_new(directives).unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = diagnostics::cleanup(&paths.logs, SystemTime::now(), 30, 500 * 1024 * 1024);
    let json = args.log_format.as_deref() == Some("json");
    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(diagnostics::LogLayer {
            file: file.clone(),
            json,
            stderr: true,
        });
    // OUT-246: Zustand für den Absturzbericht (die CLI zeigt keinen Dialog und sendet nie).
    install_crash_context(&paths.logs, &log_path, args);
    if subscriber.try_init().is_ok() {
        let old = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let backtrace = std::backtrace::Backtrace::force_capture().to_string();
            let message = diagnostics::redact_text(&format!("{info}\n{backtrace}"));
            let line = if json {
                serde_json::json!({"timestamp":diagnostics::timestamp(),"level":"ERROR","module":"panic","message":message}).to_string()
            } else {
                format!("{} ERROR panic: {message}", diagnostics::timestamp())
            };
            let _ = file.write_line(&line.replace('\n', "\\n"));
            // OUT-246: Bericht im Ordner `crashes/`, Pfad auf stderr.
            if let Some(path) = crash::on_panic(info, &backtrace) {
                eprintln!("Absturzbericht gespeichert: {}", path.display());
            }
            old(info);
        }));
        #[cfg(debug_assertions)]
        crash_test_trigger(&log_path);
    }
    // The legacy initializer keeps its other callers; an installed subscriber is left intact.
    crate::init_logging();
}

/// Zustand für Absturzberichte der CLI (`process = cli`, Log dieses Laufs, Heimatpfade für die Bereinigung).
fn install_crash_context(
    logs: &std::path::Path,
    log_path: &std::path::Path,
    args: &crate::args::RunArgs,
) {
    // Die CLI hat keine `dirs`-Abhängigkeit (D9); std liefert denselben Heimatordner.
    let mut homes: Vec<std::path::PathBuf> = std::env::home_dir().into_iter().collect();
    if let Some(h) = std::env::var_os("OPENSPIDER_HOME").filter(|h| !h.is_empty()) {
        homes.push(h.into());
    }
    crash::install(std::sync::Arc::new(crash::Context {
        dir: logs.join(crash::DIR),
        info: crash::Static::current("cli"),
        sanitizer: crash::Sanitizer::new(homes),
        log: Some((log_path.to_path_buf(), 0)),
        started: std::time::Instant::now(),
        on_written: None,
    }));
    let crawl = args.crawl.is_some() || !args.crawl_list.is_empty();
    crash::CRAWL_RUNNING.store(crawl, std::sync::atomic::Ordering::Relaxed);
}

/// Testbefehl, nur in Debug-Builds (OUT-246): `OPENSPIDER_CRASH_TEST=panic` löst sofort einen Panic aus;
/// `OPENSPIDER_CRASH_TEST=panic-after:<Text>` wartet in einem eigenen Thread, bis `<Text>` im Log dieses Laufs steht,
/// schreibt eine Logzeile mit E-Mail-Adresse und Heimatpfad und löst dann dort einen Panic aus (der Lauf geht weiter).
#[cfg(debug_assertions)]
fn crash_test_trigger(log_path: &std::path::Path) {
    let Ok(mode) = std::env::var("OPENSPIDER_CRASH_TEST") else {
        return;
    };
    if mode == "panic" {
        panic!("OpenSpider crash test (OPENSPIDER_CRASH_TEST=panic)");
    }
    let Some(needle) = mode.strip_prefix("panic-after:").map(str::to_string) else {
        return;
    };
    let log_path = log_path.to_path_buf();
    let _ = std::thread::Builder::new()
        .name("crash-test".into())
        .spawn(move || {
            let started = std::time::Instant::now();
            while started.elapsed() < std::time::Duration::from_secs(60) {
                let text = std::fs::read_to_string(&log_path).unwrap_or_default();
                if let Some(line) = text.lines().find(|l| l.contains(&needle)) {
                    let home = std::env::var("OPENSPIDER_HOME").unwrap_or_default();
                    tracing::warn!("crash test contact ops-246@crash-secret.test home={home}");
                    // E-Mail und Heimatpfad auch in der Meldung: der Test hängt nicht davon ab, welche Logzeilen
                    // bis zum Panic noch geschrieben werden.
                    panic!(
                        "crash test contact ops-246@crash-secret.test home={home} after: {line}"
                    );
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        });
}

/// Standalone diagnostics do not open any crawl or contact the network.
pub fn diagnostics_bundle(dest: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
    diagnostics_bundle_with_config(dest, None)
}

/// Includes the effective crawl configuration as well as application settings.
pub fn diagnostics_bundle_with_config(
    dest: &std::path::Path,
    config: Option<&std::path::Path>,
) -> std::io::Result<std::path::PathBuf> {
    let paths = os_store::paths::app_paths();
    let mut system = diagnostics::system_info();
    system["paths"] = serde_json::json!({"settings":paths.settings,"logs":paths.logs,"crawls":paths.crawls,"cache":paths.cache});
    let settings = os_model::settings::AppSettings::load(&paths.settings);
    let current = settings.unwrap_or_default();
    system["resourceLimits"] = serde_json::to_value(&current.storage)?;
    system["build"] = serde_json::json!(crate::args::VERSION);
    let default_config = paths.settings.join("default.osconfig");
    let crawl =
        match config.or_else(|| default_config.is_file().then_some(default_config.as_path())) {
            Some(path) => os_model::CrawlConfig::from_toml_str(&std::fs::read_to_string(path)?)
                .map_err(std::io::Error::other)?,
            None => os_model::CrawlConfig::default(),
        };
    let diff = serde_json::json!({
        "settings": diagnostics::config_diff(&serde_json::to_value(current)?, &serde_json::to_value(os_model::settings::AppSettings::default())?),
        "crawl": diagnostics::config_diff(&serde_json::to_value(crawl)?, &serde_json::to_value(os_model::CrawlConfig::default())?)
    });
    let entries = diagnostics::bundle_entries(
        &paths.logs,
        system,
        diff,
        serde_json::json!([]),
        SystemTime::now(),
    )?;
    std::fs::create_dir_all(dest)?;
    let path = dest.join(format!(
        "openspider-diagnostics-{}-{}.zip",
        diagnostics::file_stamp(),
        std::process::id()
    ));
    diagnostics::write_bundle(&path, &entries, || false)?;
    Ok(path)
}

/// Handle standalone diagnostics before crawl validation (OUT-244).
/// M2-N01: retain this early entry when consolidating CLI dispatch.
pub fn run_diagnostics(cli: &crate::Cli) -> Option<i32> {
    let dest = cli.run.diagnostics.as_deref()?;
    init(&cli.run);
    Some(
        match diagnostics_bundle_with_config(dest, cli.run.config.as_deref()) {
            Ok(path) => {
                println!("{}", path.display());
                0
            }
            Err(e) => {
                eprintln!(
                    "Diagnosepaket fehlgeschlagen: {}",
                    diagnostics::redact_text(&e.to_string())
                );
                1
            }
        },
    )
}
