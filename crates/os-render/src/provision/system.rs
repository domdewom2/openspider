//! Suche nach einem auf dem Rechner installierten Chrome/Chromium (Rückfall nach K3-01). Kandidaten unverändert aus
//! der bisherigen Suche des Renderers übernommen (M2-R01, K2-10); macOS-Pfade jetzt aus Komponenten gebaut.
use std::path::PathBuf;

pub(super) fn find() -> Option<PathBuf> {
    candidates().into_iter().find(|p| p.is_file())
}

fn candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    #[cfg(target_os = "macos")]
    {
        for root in [
            Some(PathBuf::from("/Applications")),
            dirs::home_dir().map(|h| h.join("Applications")),
        ]
        .into_iter()
        .flatten()
        {
            for (bundle, program) in [
                ("Google Chrome.app", "Google Chrome"),
                ("Chromium.app", "Chromium"),
            ] {
                candidates.push(
                    root.join(bundle)
                        .join("Contents")
                        .join("MacOS")
                        .join(program),
                );
            }
        }
    }
    #[cfg(target_os = "windows")]
    for key in ["PROGRAMFILES", "PROGRAMFILES(X86)", "LOCALAPPDATA"] {
        if let Some(root) = std::env::var_os(key) {
            candidates.push(
                PathBuf::from(&root)
                    .join("Google")
                    .join("Chrome")
                    .join("Application")
                    .join("chrome.exe"),
            );
            candidates.push(
                PathBuf::from(root)
                    .join("Chromium")
                    .join("Application")
                    .join("chrome.exe"),
            );
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            for name in [
                "chromium",
                "chromium-browser",
                "google-chrome",
                "google-chrome-stable",
                "chrome.exe",
            ] {
                candidates.push(dir.join(name));
            }
        }
    }
    candidates
}
