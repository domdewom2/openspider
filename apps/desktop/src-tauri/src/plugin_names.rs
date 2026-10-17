// Namen der internen Command-Plugins. Diese Datei wird vom Build-Skript (`include!`) und von der Bibliothek
// (`mod plugin_names`) gemeinsam benutzt, damit Berechtigungen und Registrierung nie auseinanderlaufen.
//
// Tauri registriert eigene Kern-Plugins unter diesen Namen; ein Plugin gleichen Namens würde das Kern-Plugin ersetzen
// (`PluginStore::register`). Eine Command-Domäne, die so heißt, bekommt das Präfix `os-` (heute nur `app` →
// `os-app`: Aufruf `plugin:os-app|info`, Berechtigung `os-app:default`). Das Frontend (`ipc`-Transport) wendet
// dieselbe Regel an.

/// Namen der Kern-Plugins von Tauri.
pub const TAURI_CORE_PLUGINS: &[&str] = &[
    "app",
    "event",
    "image",
    "menu",
    "path",
    "resources",
    "tray",
    "webview",
    "window",
];

/// Plugin-Name zu einer Command-Domäne.
pub fn plugin_name(domain: &str) -> String {
    if TAURI_CORE_PLUGINS.contains(&domain) {
        format!("os-{domain}")
    } else {
        domain.to_string()
    }
}
