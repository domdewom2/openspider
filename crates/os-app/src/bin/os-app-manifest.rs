//! Schreibt den Command-Katalog und die TypeScript-Typen aus der Registry.
//!
//! ```text
//! os-app-manifest [--out <commands.json>] [--ts-dir <gen-ordner>] [--check]
//! ```
//!
//! Ohne Argumente: Standardpfade `apps/desktop/src/ipc/commands.json` und `apps/desktop/src/ipc/gen` relativ zur
//! Repo-Wurzel (Arbeitsverzeichnis). Mit `--check` wird nichts geschrieben; Abweichung = Exit-Code 1. Die Pakete, die
//! Commands ergänzen, erzeugen beide Ausgaben neu und committen sie (M2.md 3.6, C7).

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut out = PathBuf::from("apps/desktop/src/ipc/commands.json");
    let mut ts_dir = PathBuf::from("apps/desktop/src/ipc/gen");
    let mut check = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--out" => out = args.next().map(PathBuf::from).unwrap_or(out),
            "--ts-dir" => ts_dir = args.next().map(PathBuf::from).unwrap_or(ts_dir),
            "--check" => check = true,
            other => {
                eprintln!("unbekanntes Argument {other:?}; erwartet: --out, --ts-dir, --check");
                return ExitCode::from(2);
            }
        }
    }

    let json = os_app::manifest_json(&os_app::default_registry());
    if check {
        let ok = fs::read_to_string(&out).is_ok_and(|on_disk| on_disk == json);
        if !ok {
            eprintln!(
                "{} weicht vom Register ab (os-app-manifest ohne --check neu erzeugen)",
                out.display()
            );
            return ExitCode::FAILURE;
        }
        return ExitCode::SUCCESS;
    }
    if let Some(parent) = out.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            eprintln!("{}: {e}", parent.display());
            return ExitCode::FAILURE;
        }
    }
    if let Err(e) = fs::write(&out, json) {
        eprintln!("{}: {e}", out.display());
        return ExitCode::FAILURE;
    }
    if let Err(e) = os_app::dto::write_gen(&ts_dir) {
        eprintln!("{}: {e}", ts_dir.display());
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
