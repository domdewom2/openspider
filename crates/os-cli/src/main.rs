fn main() {
    // Diagnostics are a standalone operation and take precedence over crawl validation.
    use clap::Parser;
    if let Ok(cli) = os_cli::Cli::try_parse() {
        if let Some(dest) = &cli.run.diagnostics {
            os_cli::logging::init(&cli.run);
            let code = match os_cli::logging::diagnostics_bundle_with_config(
                dest,
                cli.run.config.as_deref(),
            ) {
                Ok(path) => {
                    println!("{}", path.display());
                    0
                }
                Err(e) => {
                    eprintln!(
                        "Diagnosepaket fehlgeschlagen: {}",
                        os_cli::logging::diagnostics::redact_text(&e.to_string())
                    );
                    1
                }
            };
            std::process::exit(code);
        }
    }
    std::process::exit(os_cli::main_entry(std::env::args_os()));
}
