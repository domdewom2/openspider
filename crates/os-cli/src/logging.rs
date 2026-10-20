//! CLI log setup and diagnostics (OUT-240 … OUT-244).
// M2-N01 interface request (C7): move these Tauri-free primitives to a common
// crate and remove this temporary source inclusion; keep diag.rs free of app types.
#[doc(hidden)]
#[path = "../../os-app/src/diag.rs"]
pub mod diagnostics;
use std::time::SystemTime;
use tracing_subscriber::{prelude::*, EnvFilter};

pub fn init(args: &crate::args::RunArgs) {
    let paths = os_store::paths::app_paths();
    let file = diagnostics::LogFile::new(
        paths.logs.join(format!(
            "cli-{}-{}.log",
            diagnostics::file_stamp(),
            std::process::id()
        )),
        false,
    );
    let directives = args
        .log_level
        .clone()
        .or_else(|| std::env::var("OPENSPIDER_LOG").ok())
        .unwrap_or_else(|| "info".into());
    let filter =
        EnvFilter::try_new(format!("info,{directives}")).unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = diagnostics::cleanup(&paths.logs, SystemTime::now(), 30, 500 * 1024 * 1024);
    let json = args.log_format.as_deref() == Some("json");
    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(diagnostics::LogLayer {
            file: file.clone(),
            json,
            stderr: true,
        });
    if subscriber.try_init().is_ok() {
        let old = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let message = diagnostics::redact_text(&format!(
                "{info}\n{}",
                std::backtrace::Backtrace::force_capture()
            ));
            let line = if json {
                serde_json::json!({"timestamp":diagnostics::timestamp(),"level":"ERROR","module":"panic","message":message}).to_string()
            } else {
                format!("{} ERROR panic: {message}", diagnostics::timestamp())
            };
            let _ = file.write_line(&line.replace('\n', "\\n"));
            old(info);
        }));
    }
    // The legacy initializer keeps its other callers; an installed subscriber is left intact.
    crate::init_logging();
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
