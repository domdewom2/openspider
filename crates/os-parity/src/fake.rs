//! Test-Hilfsbefehl `os-parity fake-openspider`: gibt sich für die Tests von `os-parity run` als
//! `openspider` aus (versteckt in der Hilfe, nur für Tests gedacht).
//!
//! Er kopiert alle `*.csv` aus dem Ordner der Umgebungsvariable `FAKE_OPENSPIDER_SRC` nach
//! `--output-folder` und schreibt die erhaltene Kommandozeile als `args.txt` dazu. Mit
//! `FAKE_OPENSPIDER_EXIT=<n>` endet er mit diesem Exit-Code (ohne zu kopieren).

use std::path::PathBuf;

/// Führt den Fake aus und liefert den Exit-Code.
pub fn run(args: &[String]) -> i32 {
    if let Some(code) = std::env::var("FAKE_OPENSPIDER_EXIT")
        .ok()
        .and_then(|c| c.parse::<i32>().ok())
    {
        eprintln!("fake-openspider: erzwungener Fehler");
        return code;
    }
    let out = args
        .iter()
        .position(|a| a == "--output-folder")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);
    let Some(out) = out else {
        eprintln!("fake-openspider: --output-folder fehlt");
        return 2;
    };
    if let Err(e) = copy_fixtures(&out, args) {
        eprintln!("fake-openspider: {e}");
        return 2;
    }
    0
}

fn copy_fixtures(out: &PathBuf, args: &[String]) -> std::io::Result<()> {
    std::fs::create_dir_all(out)?;
    if let Some(src) = std::env::var_os("FAKE_OPENSPIDER_SRC") {
        for entry in std::fs::read_dir(src)?.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "csv") {
                std::fs::copy(&path, out.join(entry.file_name()))?;
            }
        }
    }
    std::fs::write(out.join("args.txt"), args.join("\n"))
}
