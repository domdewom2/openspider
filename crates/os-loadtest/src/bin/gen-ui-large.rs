//! Deterministische UI-Lastfixture (UI-148, M2-K19).

use clap::Parser;

fn main() {
    match os_loadtest::gen_ui::cli::Args::parse().run() {
        Ok(report) => println!("{}", serde_json::to_string(&report).unwrap()),
        Err(error) => {
            eprintln!("gen-ui-large: {error}");
            std::process::exit(1);
        }
    }
}
