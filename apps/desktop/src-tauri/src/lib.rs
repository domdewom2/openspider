//! OpenSpider Desktop: Tauri-Shell um die Rust-Kern-Crates.

/// Version der Anwendung (aus `Cargo.toml`).
fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Tauri-Command: liefert die App-Version an die UI.
#[tauri::command]
fn app_version() -> String {
    version().to_string()
}

/// Startet die Tauri-Anwendung.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_version])
        .run(tauri::generate_context!())
        .expect("Fehler beim Starten von OpenSpider");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_version_matches_package_version() {
        assert_eq!(app_version(), env!("CARGO_PKG_VERSION"));
        assert!(!app_version().is_empty());
    }
}
